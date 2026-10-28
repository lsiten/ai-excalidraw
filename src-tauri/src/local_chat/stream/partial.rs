use super::{payload, wire::PartialEvent, ChatError, Parser, ToolStatus};

impl Parser {
    pub(super) fn partial(&mut self, event: PartialEvent) -> Result<(), ChatError> {
        let id = format!("{}:{}", self.message_id, event.index);
        match event.kind.as_str() {
            "message_start" => {
                self.sequence = self.sequence.saturating_add(1);
                self.message_id = if event.message.id.is_empty() {
                    format!("claude:{}", self.sequence)
                } else {
                    event.message.id
                };
                self.blocks.clear();
                self.latest_text.clear();
                self.latest_text_id = None;
                Ok(())
            }
            "content_block_start" => {
                let block = event.content_block;
                match block.kind.as_str() {
                    "text" => self.snapshot(&id, &block.text, false)?,
                    "thinking" => {
                        self.snapshot(&format!("thinking:{id}"), &block.thinking, true)?;
                    }
                    "tool_use" | "server_tool_use" => self.tool(
                        &block.id,
                        &block.name,
                        ToolStatus::Running,
                        payload(&block.input),
                        None,
                    )?,
                    _ => {}
                }
                self.blocks.insert(event.index, block);
                Ok(())
            }
            "content_block_delta" => {
                let Some(block) = self.blocks.get_mut(&event.index) else {
                    return Ok(());
                };
                match event.delta.kind.as_str() {
                    "text_delta" => {
                        block.text.push_str(&event.delta.text);
                        let text = block.text.clone();
                        self.snapshot(&id, &text, false)
                    }
                    "thinking_delta" => {
                        block.thinking.push_str(&event.delta.thinking);
                        let text = block.thinking.clone();
                        self.snapshot(&format!("thinking:{id}"), &text, true)
                    }
                    "input_json_delta" => {
                        block.text.push_str(&event.delta.partial_json);
                        let (call, name, input) =
                            (block.id.clone(), block.name.clone(), block.text.clone());
                        self.tool(&call, &name, ToolStatus::Running, Some(input), None)
                    }
                    _ => Ok(()),
                }
            }
            // Finishing argument generation does not finish the native tool call.
            _ => Ok(()),
        }
    }
}
