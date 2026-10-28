use super::{
    payload,
    wire::{Event, Message},
    ChatError, Parser, ToolStatus,
};

impl Parser {
    pub(super) fn pi(&mut self, event: Event) -> Result<(), ChatError> {
        match event.kind.as_str() {
            "agent_start" => self.status("Pi 会话已启动"),
            "turn_start" => {
                self.failure = None;
                self.sequence = self.sequence.saturating_add(1);
                self.latest_text.clear();
                self.blocks.clear();
                Ok(())
            }
            "message_update" => {
                self.failure = None;
                if let Some(delta) = event.assistant_delta {
                    let id = format!("pi:{}:{}", self.sequence, delta.content_index);
                    match delta.kind.as_str() {
                        "text_delta" | "thinking_delta" => {
                            let thinking = delta.kind == "thinking_delta";
                            let id = if thinking {
                                format!("thinking:{id}")
                            } else {
                                id
                            };
                            let text = self.text_by_id.get(&id).cloned().unwrap_or_default()
                                + &delta.fragment;
                            self.snapshot(&id, &text, thinking)?;
                        }
                        "text_end" => self.snapshot(&id, &delta.content, false)?,
                        "thinking_end" => {
                            self.snapshot(&format!("thinking:{id}"), &delta.content, true)?;
                        }
                        "toolcall_start" | "toolcall_delta" | "toolcall_end" => {
                            let call = delta
                                .tool_call
                                .as_ref()
                                .or_else(|| delta.partial.content.get(delta.content_index));
                            if let Some(call) = call {
                                self.tool(
                                    &call.id,
                                    &call.name,
                                    ToolStatus::Running,
                                    payload(&call.input),
                                    None,
                                )?;
                            }
                        }
                        _ => {}
                    }
                }
                Ok(())
            }
            "tool_execution_start" | "tool_execution_update" => {
                self.failure = None;
                self.tool(
                    &event.tool_call_id,
                    &event.pi_tool_name,
                    ToolStatus::Running,
                    payload(&event.args),
                    payload(&event.partial_result),
                )
            }
            "tool_execution_end" => self.tool(
                &event.tool_call_id,
                &event.pi_tool_name,
                if event.pi_is_error {
                    ToolStatus::Failed
                } else {
                    ToolStatus::Completed
                },
                None,
                payload(&event.result),
            ),
            "message_end" | "turn_end" => {
                let message: Message =
                    serde_json::from_value(event.message).map_err(|_| ChatError::Protocol)?;
                self.pi_message(&message)
            }
            "agent_end" => {
                if event.will_retry {
                    return self.status("工具正在自动重试");
                }
                if let Some(message) = event.messages.iter().rev().find(|m| m.role == "assistant") {
                    self.pi_message(message)?;
                }
                let failure = self.failure.take();
                self.complete("", failure)
            }
            "auto_retry_start" => self.status("工具正在自动重试"),
            "auto_retry_end" => {
                self.failure = if event.success {
                    None
                } else {
                    Some(event.final_error)
                };
                Ok(())
            }
            "error" => {
                self.failure = payload(&event.message).or_else(|| payload(&event.error));
                self.status("Pi 报告错误")
            }
            _ => Ok(()),
        }
    }

    fn pi_message(&mut self, message: &Message) -> Result<(), ChatError> {
        if message.role != "assistant" {
            return Ok(());
        }
        if matches!(message.stop_reason.as_str(), "error" | "aborted") {
            self.failure = Some(message.failure.clone());
            return Ok(());
        }
        self.failure = None;
        for (index, block) in message.content.iter().enumerate() {
            let id = format!("pi:{}:{index}", self.sequence);
            match block.kind.as_str() {
                "text" => self.snapshot(&id, &block.text, false)?,
                "thinking" => self.snapshot(&format!("thinking:{id}"), &block.thinking, true)?,
                _ => {}
            }
        }
        Ok(())
    }
}
