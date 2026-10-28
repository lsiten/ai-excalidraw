use super::super::{error::ChatError, events::LocalChatEvent, stream::Sink};
use serde_json::Value;
use std::collections::{HashMap, HashSet};

#[derive(Default)]
pub(super) struct Messages {
    streamed: HashSet<String>,
    phases: HashMap<String, String>,
    last_phase: Option<String>,
}
impl Messages {
    pub(super) fn invalidate_phase(&mut self) {
        self.last_phase = None;
    }
    fn phase(
        &mut self,
        id: &str,
        reported: Option<&str>,
        sink: &mut Sink,
    ) -> Result<(), ChatError> {
        if let Some(phase) = reported.filter(|s| !s.is_empty()) {
            self.phases.insert(id.into(), phase.into());
        }
        if let Some(phase) = self.phases.get(id) {
            if self.last_phase.as_ref() != Some(phase) {
                sink(LocalChatEvent::Phase {
                    phase: phase.clone(),
                })?;
                self.last_phase = Some(phase.clone());
            }
        }
        Ok(())
    }
    pub(super) fn delta(&mut self, params: &Value, sink: &mut Sink) -> Result<(), ChatError> {
        let id = params["itemId"].as_str().unwrap_or("");
        self.phase(id, params["phase"].as_str(), sink)?;
        self.streamed.insert(id.into());
        sink(LocalChatEvent::Chunk {
            text: params["delta"].as_str().ok_or(ChatError::Protocol)?.into(),
        })
    }
    pub(super) fn item(
        &mut self,
        item: &Value,
        completed: bool,
        sink: &mut Sink,
    ) -> Result<(), ChatError> {
        if item["type"] != "agentMessage" {
            return Ok(());
        }
        let id = item["id"].as_str().ok_or(ChatError::Protocol)?;
        self.phase(id, item["phase"].as_str(), sink)?;
        if completed && !self.streamed.contains(id) && !self.streamed.contains("") {
            sink(LocalChatEvent::Chunk {
                text: item["text"].as_str().ok_or(ChatError::Protocol)?.into(),
            })?;
        }
        Ok(())
    }
}
pub(super) fn usage(params: &Value, sink: &mut Sink) -> Result<(), ChatError> {
    let usage = &params["tokenUsage"]["total"];
    if let (
        Some(input_tokens),
        Some(output_tokens),
        Some(cached_input_tokens),
        Some(total_tokens),
    ) = (
        usage["inputTokens"].as_u64(),
        usage["outputTokens"].as_u64(),
        usage["cachedInputTokens"].as_u64(),
        usage["totalTokens"].as_u64(),
    ) {
        sink(LocalChatEvent::Usage {
            input_tokens,
            output_tokens,
            cached_input_tokens,
            total_tokens,
        })?;
    }
    Ok(())
}
