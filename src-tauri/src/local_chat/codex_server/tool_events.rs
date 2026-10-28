use super::super::{
    error::ChatError,
    events::{LocalChatEvent, ToolStatus},
    stream::Sink,
};
use super::{session::Progress, watchdog::Watchdog};
use serde_json::Value;

const PAYLOAD_LIMIT: usize = 64 * 1024;
fn payload(item: &Value) -> String {
    let mut value = item.to_string();
    if value.len() > PAYLOAD_LIMIT {
        let marker = "\n[truncated]";
        let mut end = PAYLOAD_LIMIT - marker.len();
        while !value.is_char_boundary(end) {
            end -= 1;
        }
        value.truncate(end);
        value.push_str(marker);
    }
    value
}
fn text<'a>(item: &'a Value, key: &str) -> Result<&'a str, ChatError> {
    item[key]
        .as_str()
        .filter(|s| !s.is_empty())
        .ok_or(ChatError::Protocol)
}
fn identity(item: &Value) -> Result<Option<(String, bool)>, ChatError> {
    let result = match text(item, "type")? {
        "mcpToolCall" => {
            let server = text(item, "server")?;
            let tool = text(item, "tool")?;
            (
                format!("{server}:{tool}"),
                server == "ai-excalidraw"
                    && matches!(tool, "request_user_input" | "remember_project_fact"),
            )
        }
        "commandExecution" => ("command_execution".into(), false),
        "fileChange" => ("file_change".into(), false),
        "webSearch" => ("web_search".into(), false),
        "dynamicToolCall" => ("dynamic_tool_call".into(), false),
        _ => return Ok(None),
    };
    Ok(Some(result))
}
pub(super) fn emit(
    item: &Value,
    running: bool,
    sink: &mut Sink,
    progress: &Progress,
    watchdog: &mut Watchdog,
) -> Result<(), ChatError> {
    let Some((name, human)) = identity(item)? else {
        return Ok(());
    };
    let id = text(item, "id")?;
    let was_waiting = watchdog.waiting_for_human();
    if running {
        if human {
            watchdog.human_started(id.into());
        } else {
            watchdog.tool_started(id.into(), &progress.limits);
        }
    } else {
        watchdog.tool_completed(id, &progress.limits);
    }
    sink(LocalChatEvent::Tool {
        name,
        id: id.into(),
        status: if running {
            ToolStatus::Running
        } else if item["status"] == "failed" {
            ToolStatus::Failed
        } else {
            ToolStatus::Completed
        },
        input: running.then(|| payload(item)),
        output: (!running).then(|| payload(item)),
    })?;
    let waiting = watchdog.waiting_for_human();
    if waiting != was_waiting {
        sink(LocalChatEvent::Phase {
            phase: if waiting { "waiting_input" } else { "running" }.into(),
        })?;
    }
    Ok(())
}
