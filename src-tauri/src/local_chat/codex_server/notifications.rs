use super::super::{
    error::ChatError,
    events::{LocalChatEvent, PlanStep},
    stream::Sink,
};
use super::messages::{usage, Messages};
use super::{session::Progress, watchdog::Watchdog};
use serde_json::Value;
pub(super) fn handle(
    method: &str,
    params: &Value,
    sink: &mut Sink,
    progress: &mut Progress,
    watchdog: &mut Watchdog,
    messages: &mut Messages,
) -> Result<bool, ChatError> {
    match method {
        "turn/started" => {
            messages.invalidate_phase();
            progress.turn = Some(
                params["turn"]["id"]
                    .as_str()
                    .ok_or(ChatError::Protocol)?
                    .into(),
            );
            sink(LocalChatEvent::Phase {
                phase: "running".into(),
            })?;
        }
        "item/agentMessage/delta" => {
            messages.delta(params, sink)?;
            watchdog.semantic(&progress.limits);
        }
        "item/reasoning/summaryTextDelta" => {
            sink(LocalChatEvent::Thinking {
                text: text(params, "delta")?,
            })?;
            watchdog.semantic(&progress.limits);
        }
        "turn/plan/updated" => {
            let steps = params["plan"]
                .as_array()
                .ok_or(ChatError::Protocol)?
                .iter()
                .map(|step| {
                    Ok(PlanStep {
                        step: text(step, "step")?,
                        status: text(step, "status")?,
                    })
                })
                .collect::<Result<Vec<_>, ChatError>>()?;
            sink(LocalChatEvent::Plan {
                explanation: params["explanation"].as_str().map(str::to_owned),
                steps,
            })?;
            watchdog.semantic(&progress.limits);
        }
        "thread/tokenUsage/updated" => {
            usage(params, sink)?;
        }
        "item/started" | "item/completed" => {
            let item = &params["item"];
            messages.item(item, method == "item/completed", sink)?;
            let was_waiting = watchdog.waiting_for_human();
            super::tool_events::emit(item, method == "item/started", sink, progress, watchdog)?;
            if was_waiting != watchdog.waiting_for_human() {
                messages.invalidate_phase();
            }
            watchdog.semantic(&progress.limits);
        }
        "turn/completed" => {
            if progress
                .turn
                .as_deref()
                .is_some_and(|id| params["turn"]["id"].as_str() != Some(id))
            {
                return Ok(false);
            }
            sink(LocalChatEvent::Phase {
                phase: params["turn"]["status"].as_str().unwrap_or("failed").into(),
            })?;
            return if params["turn"]["status"] == "completed" {
                Ok(true)
            } else {
                Err(super::diagnostics::failed_turn(&params["turn"]))
            };
        }
        "error" if params["willRetry"] != true => {
            return Err(super::diagnostics::failure("Codex request failed", params))
        }
        "warning" | "error" => sink(super::diagnostics::status(params, method == "error"))?,
        _ => {}
    }
    Ok(false)
}
fn text(value: &Value, key: &str) -> Result<String, ChatError> {
    value[key]
        .as_str()
        .map(str::to_owned)
        .ok_or(ChatError::Protocol)
}
