mod activity;
mod anthropic;
mod codex;
mod cursor;
#[cfg(test)]
mod fixture_tests;
#[cfg(test)]
mod lifecycle_tests;
mod partial;
mod pi;
#[cfg(test)]
mod read_tests;
#[cfg(test)]
mod tests;
mod wire;

use super::{
    error::{cli_failure, io, ChatError, FailureOrigin},
    events::{LocalChatEvent, ToolStatus},
    process::{kill_registered, RESPONSE_LIMIT},
};
use crate::integrations::ToolId;
use std::{collections::HashMap, sync::Mutex};
use tokio::io::{AsyncBufReadExt, AsyncRead, BufReader};
use wire::Event;

pub(super) type Sink = Box<dyn FnMut(LocalChatEvent) -> Result<(), ChatError> + Send>;

pub(super) struct Parser {
    tool: ToolId,
    emit: Sink,
    pub terminal: bool,
    failure: Option<String>,
    text_by_id: HashMap<String, String>,
    tool_names: HashMap<String, String>,
    latest_text: String,
    latest_text_id: Option<String>,
    emitted_text: bool,
    activities: HashMap<String, LocalChatEvent>,
    transcript: String,
    last_status: String,
    message_id: String,
    blocks: HashMap<usize, wire::Block>,
    sequence: usize,
    cursor_partial: bool,
}

impl Parser {
    pub(super) fn new(tool: ToolId, emit: Sink) -> Self {
        Self {
            tool,
            emit,
            terminal: false,
            failure: None,
            text_by_id: HashMap::new(),
            tool_names: HashMap::new(),
            latest_text: String::new(),
            latest_text_id: None,
            emitted_text: false,
            activities: HashMap::new(),
            transcript: String::new(),
            last_status: String::new(),
            message_id: String::new(),
            blocks: HashMap::new(),
            sequence: 0,
            cursor_partial: false,
        }
    }

    pub(super) fn line(&mut self, bytes: &[u8]) -> Result<(), ChatError> {
        if self.terminal || bytes.iter().all(u8::is_ascii_whitespace) {
            return Ok(());
        }
        let event: Event = serde_json::from_slice(bytes).map_err(|_| ChatError::Protocol)?;
        match self.tool {
            ToolId::Codex => self.codex(event),
            ToolId::Claude | ToolId::Cursor => self.anthropic(event),
            ToolId::Pi => self.pi(event),
        }
    }

    fn send(&mut self, event: LocalChatEvent) -> Result<(), ChatError> {
        (self.emit)(event)
    }

    fn status(&mut self, message: &str) -> Result<(), ChatError> {
        if self.last_status == message {
            return Ok(());
        }
        self.last_status = message.into();
        self.send(LocalChatEvent::Status {
            message: message.into(),
        })
    }

    fn text(&mut self, text: &str) -> Result<(), ChatError> {
        if text.is_empty() {
            return Ok(());
        }
        if text.len() > RESPONSE_LIMIT {
            return Err(ChatError::OutputLimit);
        }
        self.emitted_text = true;
        self.latest_text.push_str(text);
        self.transcript.push_str(text);
        self.send(LocalChatEvent::Chunk { text: text.into() })
    }

    fn snapshot(&mut self, id: &str, text: &str, thinking: bool) -> Result<(), ChatError> {
        let old = self.text_by_id.entry(id.to_owned()).or_default();
        // Append-only consumers cannot retract a rewritten snapshot. Keep delivered
        // bytes and accept only a matching cumulative suffix (as Multica does).
        let Some(delta) = text.strip_prefix(old.as_str()).map(str::to_owned) else {
            return Ok(());
        };
        *old = text.into();
        if delta.is_empty() {
            return Ok(());
        }
        if !thinking && self.latest_text_id.as_deref() != Some(id) {
            self.latest_text.clear();
            self.latest_text_id = Some(id.to_owned());
        }
        if thinking {
            self.send(LocalChatEvent::Thinking { text: delta })
        } else {
            self.text(&delta)
        }
    }

    fn complete(&mut self, final_text: &str, failure: Option<String>) -> Result<(), ChatError> {
        self.terminal = true;
        self.failure = failure;
        self.settle_tools()?;
        if self.failure.is_none() && !final_text.is_empty() {
            let candidate = if final_text.starts_with(&self.transcript) {
                &self.transcript
            } else {
                &self.latest_text
            };
            let suffix = final_text
                .strip_prefix(candidate.as_str())
                .unwrap_or(final_text)
                .to_owned();
            if !suffix.is_empty() {
                self.text(&suffix)?;
            }
        }
        Ok(())
    }

    pub(super) fn finish(self) -> Result<(), ChatError> {
        if let Some(error) = self.failure {
            return Err(cli_failure(self.tool, &error, FailureOrigin::Agent));
        }
        if !self.terminal {
            return Err(ChatError::Protocol);
        }
        if !self.emitted_text {
            return self.empty_reply();
        }
        Ok(())
    }

    fn empty_reply(&self) -> Result<(), ChatError> {
        // Successful MCP-only edits need not include assistant prose.
        if self.tool_names.is_empty() {
            Err(cli_failure(
                self.tool,
                "empty response",
                FailureOrigin::Agent,
            ))
        } else {
            Ok(())
        }
    }
}

fn bounded(mut text: String) -> String {
    if text.len() > 32 * 1024 {
        let mut end = 32 * 1024;
        while !text.is_char_boundary(end) {
            end -= 1;
        }
        text.truncate(end);
        text.push_str("\n[已截断]");
    }
    text
}

fn payload(value: &serde_json::Value) -> Option<String> {
    match value {
        serde_json::Value::Null => None,
        serde_json::Value::String(text) => Some(text.clone()),
        value => Some(value.to_string()),
    }
}

pub(super) async fn read(
    reader: impl AsyncRead + Unpin,
    mut parser: Parser,
    process: &Mutex<Option<u32>>,
) -> Result<Parser, ChatError> {
    let mut reader = BufReader::new(reader);
    let mut line = Vec::new();
    let mut total = 0usize;
    loop {
        let available = reader.fill_buf().await.map_err(io("读取本地工具事件"))?;
        if available.is_empty() {
            if !line.is_empty() {
                parser.line(&line)?;
            }
            return Ok(parser);
        }
        let length = available
            .iter()
            .position(|c| *c == b'\n')
            .map_or(available.len(), |i| i + 1);
        total = total.saturating_add(length);
        if total > RESPONSE_LIMIT {
            return Err(ChatError::OutputLimit);
        }
        line.extend_from_slice(&available[..length]);
        let complete = available[length - 1] == b'\n';
        reader.consume(length);
        if complete {
            parser.line(&line)?;
            line.clear();
            if parser.terminal {
                kill_registered(process);
                return Ok(parser);
            }
        }
    }
}
