use super::super::{error::ChatError, events::LocalChatEvent};
use serde_json::Value;
const PUBLIC_LIMIT: usize = 1024;

fn public_message(value: &Value) -> Option<String> {
    let message = value["message"]
        .as_str()
        .or_else(|| value["error"]["message"].as_str())?
        .trim();
    if message.is_empty() {
        return None;
    }
    // JSON encoded inside a message can be a provider envelope with credentials.
    if message.starts_with('{') || message.starts_with('[') {
        return None;
    }
    let lower = message.to_ascii_lowercase();
    if [
        "authorization",
        "bearer ",
        "api_key",
        "api-key",
        "access_token",
        "refresh_token",
        "\"headers\"",
        "password",
    ]
    .iter()
    .any(|key| lower.contains(key))
    {
        return None;
    }
    let mut message = message.to_owned();
    if message.len() > PUBLIC_LIMIT {
        let mut end = PUBLIC_LIMIT - 3;
        while !message.is_char_boundary(end) {
            end -= 1;
        }
        message.truncate(end);
        message.push_str("...");
    }
    Some(message)
}
pub(super) fn status(value: &Value, retrying: bool) -> LocalChatEvent {
    let prefix = if retrying {
        "Codex is retrying the current request"
    } else {
        "Codex reported a warning"
    };
    LocalChatEvent::Status {
        message: public_message(value).map_or_else(
            || format!("{prefix}."),
            |message| format!("{prefix}: {message}"),
        ),
    }
}
pub(super) fn failure(context: &str, value: &Value) -> ChatError {
    ChatError::Cli(public_message(value).map_or_else(
        || format!("{context}. No public error message was provided."),
        |message| format!("{context}: {message}"),
    ))
}
pub(super) fn failed_turn(turn: &Value) -> ChatError {
    let context = match turn["status"].as_str() {
        Some("interrupted") => "Codex turn was interrupted",
        Some("failed") => "Codex turn failed",
        _ => "Codex turn ended unsuccessfully",
    };
    failure(context, turn)
}
