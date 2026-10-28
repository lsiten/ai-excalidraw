use super::{
    payload,
    wire::{Event, Message},
    ChatError, LocalChatEvent, Parser, ToolStatus,
};

impl Parser {
    fn tool_results(&mut self, value: serde_json::Value) -> Result<(), ChatError> {
        let message: Message = serde_json::from_value(value).map_err(|_| ChatError::Protocol)?;
        for block in message.content {
            if block.kind == "tool_result" {
                self.tool(
                    &block.tool_use_id,
                    "",
                    if block.is_error {
                        ToolStatus::Failed
                    } else {
                        ToolStatus::Completed
                    },
                    None,
                    payload(&block.content),
                )?;
            }
        }
        Ok(())
    }

    pub(super) fn anthropic(&mut self, event: Event) -> Result<(), ChatError> {
        match event.kind.as_str() {
            "stream_event" => match event.partial {
                Some(partial) => self.partial(partial),
                None => Err(ChatError::Protocol),
            },
            "system" => match event.subtype.as_str() {
                "init" => self.status(&format!("{} 会话已启动", self.tool.name())),
                "error" => {
                    self.failure = payload(&event.error).or_else(|| payload(&event.message));
                    self.status("本地工具报告错误")
                }
                "status" if event.status.as_str() == Some("compacting") => {
                    self.status("正在压缩上下文")
                }
                _ => Ok(()),
            },
            "assistant" => {
                // With Cursor partial output, only timestamped events without
                // model_call_id carry new text. Both buffered flushes repeat it.
                if self.tool == crate::integrations::ToolId::Cursor {
                    if event.timestamp_ms.is_some() && event.model_call_id.is_none() {
                        self.cursor_partial = true;
                    } else if self.cursor_partial {
                        return Ok(());
                    }
                }
                self.assistant(event.message)
            }
            // A user event may echo the prompt; only tool_result blocks are output.
            "user" => self.tool_results(event.message),
            "thinking" => match event.subtype.as_str() {
                "delta" if !event.text.is_empty() => {
                    self.send(LocalChatEvent::Thinking { text: event.text })
                }
                _ => Ok(()),
            },
            "tool_call" => self.cursor_call(event),
            "tool_use" => self.tool(
                &event.tool_id,
                &event.tool_name,
                ToolStatus::Running,
                payload(&event.parameters),
                None,
            ),
            "tool_result" => self.tool(
                &event.tool_id,
                "",
                if event.is_error {
                    ToolStatus::Failed
                } else {
                    ToolStatus::Completed
                },
                None,
                Some(event.output),
            ),
            "result" => {
                let failure = if event.is_error
                    || event.terminal_reason == "prompt_too_long"
                    || (!event.subtype.is_empty() && event.subtype != "success")
                {
                    Some(event.errors.join(" ") + &payload(&event.result).unwrap_or_default())
                } else {
                    self.failure.take()
                };
                self.complete(event.result.as_str().unwrap_or_default(), failure)
            }
            "error" => {
                self.failure = payload(&event.error).or_else(|| payload(&event.message));
                Ok(())
            }
            _ => Ok(()),
        }
    }
    fn assistant(&mut self, value: serde_json::Value) -> Result<(), ChatError> {
        let message: Message = serde_json::from_value(value).map_err(|_| ChatError::Protocol)?;
        let has_tools = message
            .content
            .iter()
            .any(|b| matches!(b.kind.as_str(), "tool_use" | "server_tool_use"));
        let mut combined = String::new();
        for (index, block) in message.content.into_iter().enumerate() {
            // Claude emits one completed block per assistant envelope, so its
            // content index can be zero even when the SSE block index is not.
            let index = if message.id == self.message_id {
                self.blocks
                    .iter()
                    .filter(|(_, partial)| {
                        partial.kind == block.kind
                            && match block.kind.as_str() {
                                "text" => block.text.starts_with(&partial.text),
                                "thinking" => block.thinking.starts_with(&partial.thinking),
                                _ => partial.id == block.id,
                            }
                    })
                    .map(|(index, _)| *index)
                    .max()
                    .unwrap_or(index)
            } else {
                index
            };
            match block.kind.as_str() {
                "text" | "output_text" => {
                    combined.push_str(&block.text);
                    let id = if message.id.is_empty() {
                        // Cursor assistant events without IDs are deltas.
                        self.text(&block.text)?;
                        continue;
                    } else {
                        format!("{}:{index}", message.id)
                    };
                    self.snapshot(&id, &block.text, false)?;
                }
                "thinking" => {
                    let text = if block.thinking.is_empty() {
                        block.text
                    } else {
                        block.thinking
                    };
                    self.snapshot(&format!("thinking:{}:{index}", message.id), &text, true)?;
                }
                "tool_use" | "server_tool_use" => self.tool(
                    &block.id,
                    &block.name,
                    ToolStatus::Running,
                    payload(&block.input),
                    None,
                )?,
                "tool_result" => {
                    let status = if block.is_error {
                        ToolStatus::Failed
                    } else {
                        ToolStatus::Completed
                    };
                    self.tool(
                        &block.tool_use_id,
                        "",
                        status,
                        None,
                        payload(&block.content),
                    )?;
                }
                _ => {}
            }
        }
        if !combined.is_empty() && !message.id.is_empty() && !has_tools {
            self.latest_text = combined;
        }
        Ok(())
    }
}
