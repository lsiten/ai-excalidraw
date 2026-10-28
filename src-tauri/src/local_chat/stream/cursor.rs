use super::{payload, wire::Event, ChatError, Parser, ToolStatus};

impl Parser {
    pub(super) fn cursor_call(&mut self, event: Event) -> Result<(), ChatError> {
        let status = match event.subtype.as_str() {
            "started" | "updated" => ToolStatus::Running,
            "completed" => ToolStatus::Completed,
            "error" | "failed" => ToolStatus::Failed,
            _ => return Ok(()),
        };
        let call_id = if event.call_id.is_empty() {
            event
                .tool_call
                .get("toolCallId")
                .and_then(serde_json::Value::as_str)
                .unwrap_or_default()
                .to_owned()
        } else {
            event.call_id
        };
        for (key, value) in event.tool_call {
            if key.ends_with("ToolCall") {
                let output = value.get("result");
                let status = if output.is_some_and(cursor_failed) {
                    ToolStatus::Failed
                } else {
                    status.clone()
                };
                self.tool(
                    &call_id,
                    key.trim_end_matches("ToolCall"),
                    status,
                    value.get("args").and_then(payload),
                    output.and_then(payload),
                )?;
            }
        }
        Ok(())
    }
}

fn cursor_failed(result: &serde_json::Value) -> bool {
    result.get("error").is_some_and(|v| !v.is_null())
        || result
            .get("success")
            .and_then(|s| s.get("exitCode"))
            .and_then(serde_json::Value::as_i64)
            .is_some_and(|code| code != 0)
        || result.get("isError").and_then(serde_json::Value::as_bool) == Some(true)
}
