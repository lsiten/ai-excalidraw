use super::{payload, wire::Event, ChatError, Parser, ToolStatus};

impl Parser {
    pub(super) fn codex(&mut self, event: Event) -> Result<(), ChatError> {
        match event.kind.as_str() {
            "thread.started" => self.status("Codex 会话已启动"),
            "turn.started" => self.status("Codex 正在执行"),
            "item.started" | "item.updated" | "item.completed" => {
                let item = event.item.ok_or(ChatError::Protocol)?;
                let completed = matches!(event.kind.as_str(), "item.completed");
                let status = if completed {
                    if matches!(item.status.as_str(), "failed" | "declined" | "cancelled")
                        || item.exit_code.is_some_and(|c| c != 0)
                        || !item.error.is_null()
                    {
                        ToolStatus::Failed
                    } else {
                        ToolStatus::Completed
                    }
                } else {
                    ToolStatus::Running
                };
                match item.kind.as_str() {
                    "agent_message" => {
                        self.snapshot(&item.id, &item.text, false)?;
                        if completed && !item.text.is_empty() {
                            self.status("Codex 已生成回复")?;
                        }
                        Ok(())
                    }
                    "reasoning" => {
                        self.snapshot(&format!("thinking:{}", item.id), &item.text, true)
                    }
                    "command_execution" | "mcp_tool_call" | "file_change" | "web_search"
                    | "todo_list" => {
                        let name = match item.kind.as_str() {
                            "todo_list" => "update_plan".into(),
                            _ if !item.tool.is_empty() => format!("{}:{}", item.server, item.tool),
                            _ => self
                                .tool_names
                                .get(&item.id)
                                .cloned()
                                .unwrap_or_else(|| item.kind.clone()),
                        };
                        let input = match item.kind.as_str() {
                            "file_change" => payload(&item.changes),
                            "todo_list" => Some(
                                serde_json::to_string(&item.items)
                                    .map_err(|_| ChatError::Protocol)?,
                            ),
                            "web_search" => Some(item.query),
                            _ if !item.command.is_empty() => Some(item.command),
                            _ => payload(&item.arguments),
                        };
                        let output = if !item.error.is_null() {
                            payload(&item.error)
                        } else if !item.aggregated_output.is_empty() {
                            Some(item.aggregated_output)
                        } else {
                            payload(&item.result)
                        };
                        self.tool(&item.id, &name, status, input, output)
                    }
                    // Error items are non-fatal in Codex exec's JSONL protocol.
                    "error" => self.status("Codex 报告了可恢复的问题，继续执行"),
                    _ => Ok(()),
                }
            }
            "item.agent_message.delta" | "item/agentMessage/delta" => {
                if let Some(text) = event.delta.as_str() {
                    let id = if event.item_id.is_empty() {
                        "codex:message"
                    } else {
                        &event.item_id
                    };
                    let snapshot = self.text_by_id.get(id).cloned().unwrap_or_default() + text;
                    self.snapshot(id, &snapshot, false)?;
                }
                Ok(())
            }
            "turn.completed" => {
                self.complete("", None)?;
                let message = event.usage.map_or_else(
                    || "Codex 执行完成".into(),
                    |usage| {
                        format!(
                            "Codex 执行完成 · 输入 {} / 输出 {} tokens",
                            usage.input_tokens, usage.output_tokens
                        )
                    },
                );
                self.status(&message)
            }
            "turn.failed" | "error" => {
                let error = payload(&event.error)
                    .or_else(|| payload(&event.message))
                    .unwrap_or_else(|| "Codex error".into());
                self.complete("", Some(error))
            }
            // Unknown event types never become assistant prose or tool completion.
            _ => Ok(()),
        }
    }
}
