#![expect(clippy::unwrap_used, reason = "Test fixtures fail immediately")]
use super::super::{events::LocalChatEvent, stream::Sink};
use super::{
    messages::Messages,
    notifications,
    session::Progress,
    watchdog::{Limits, Watchdog},
};
use serde_json::{json, Value};
use std::{
    sync::{Arc, Mutex},
    time::Duration,
};
use tokio::time::Instant;
struct Harness {
    progress: Progress,
    watchdog: Watchdog,
    messages: Messages,
    events: Arc<Mutex<Vec<LocalChatEvent>>>,
    sink: Sink,
}
impl Harness {
    fn new() -> Self {
        let limits = Limits::default();
        let watchdog = Watchdog::new(&limits);
        let events = Arc::new(Mutex::new(Vec::new()));
        let output = events.clone();
        Self {
            progress: Progress {
                limits,
                submitted: true,
                thread: Some("thread".into()),
                turn: Some("turn".into()),
            },
            watchdog,
            messages: Messages::default(),
            events,
            sink: Box::new(move |e| {
                output.lock().unwrap().push(e);
                Ok(())
            }),
        }
    }
    fn notify(&mut self, method: &str, params: Value) {
        notifications::handle(
            method,
            &params,
            &mut self.sink,
            &mut self.progress,
            &mut self.watchdog,
            &mut self.messages,
        )
        .unwrap();
    }
    fn tool(&mut self, id: &str, server: &str, tool: &str, started: bool) {
        self.notify(if started {"item/started"} else {"item/completed"},json!({"item":{"type":"mcpToolCall","id":id,"server":server,"tool":tool,"arguments":{},"status":"completed"}}));
    }
}
#[test]
fn human_calls_suspend_generic_watchdogs_without_extending_total_window() {
    let start = Instant::now();
    let mut h = Harness::new();
    h.tool("other", "external", "request_user_input", true);
    assert!(!h.watchdog.waiting_for_human());
    assert!(h.watchdog.expiry() < start + Duration::from_secs(121));
    h.tool("input", "ai-excalidraw", "request_user_input", true);
    let deadline = h.watchdog.expiry();
    assert!(deadline >= start + Duration::from_secs(299));
    assert!(deadline < start + Duration::from_secs(301));
    h.tool("memory", "ai-excalidraw", "remember_project_fact", true);
    h.tool("input", "ai-excalidraw", "request_user_input", true);
    assert_eq!(h.watchdog.expiry(), deadline);
    h.tool("input", "ai-excalidraw", "request_user_input", false);
    assert!(h.watchdog.waiting_for_human());
    h.tool("memory", "ai-excalidraw", "remember_project_fact", false);
    assert!(!h.watchdog.waiting_for_human());
    assert!(h.watchdog.expiry() < Instant::now() + Duration::from_secs(121));
    let events = h.events.lock().unwrap();
    let phases: Vec<_> = events
        .iter()
        .filter_map(|e| {
            if let LocalChatEvent::Phase { phase } = e {
                Some(phase.as_str())
            } else {
                None
            }
        })
        .collect();
    assert_eq!(phases, ["waiting_input", "running"]);
    assert!(events.iter().any(
        |e| matches!(e,LocalChatEvent::Tool{name,..} if name=="ai-excalidraw:request_user_input")
    ));
    println!("human_wait_budget=300 generic_tool_budget=120 duplicates_extend_deadline=false resumed_running=true");
}
#[test]
fn tool_names_payload_bounds_and_public_summaries_are_normalized() {
    let mut h = Harness::new();
    for (kind, name) in [
        ("commandExecution", "command_execution"),
        ("fileChange", "file_change"),
        ("webSearch", "web_search"),
    ] {
        h.notify(
            "item/started",
            json!({"item":{"type":kind,"id":kind,"data":"界".repeat(30000)}}),
        );
        assert!(h.events.lock().unwrap().iter().any(|e|matches!(e,LocalChatEvent::Tool{name:n,input:Some(payload),..} if n==name && payload.len()<=65536 && payload.ends_with("[truncated]"))));
    }
    h.notify("item/reasoning/textDelta", json!({"delta":"private"}));
    assert!(!h
        .events
        .lock()
        .unwrap()
        .iter()
        .any(|e| matches!(e, LocalChatEvent::Thinking { .. })));
    h.notify("item/reasoning/summaryTextDelta", json!({"delta":"public"}));
    assert_eq!(
        h.events
            .lock()
            .unwrap()
            .iter()
            .filter(|e| matches!(e, LocalChatEvent::Thinking { .. }))
            .count(),
        1
    );
}
#[test]
fn missing_usage_is_unknown_but_reported_zero_is_preserved() {
    let mut h = Harness::new();
    for key in [
        "inputTokens",
        "outputTokens",
        "cachedInputTokens",
        "totalTokens",
    ] {
        let mut total =
            json!({"inputTokens":0,"outputTokens":0,"cachedInputTokens":0,"totalTokens":0});
        total.as_object_mut().unwrap().remove(key);
        h.notify(
            "thread/tokenUsage/updated",
            json!({"tokenUsage":{"total":total}}),
        );
    }
    h.notify("thread/tokenUsage/updated", json!({}));
    assert!(h.events.lock().unwrap().is_empty());
    h.notify("thread/tokenUsage/updated",json!({"tokenUsage":{"total":{"inputTokens":0,"outputTokens":0,"cachedInputTokens":0,"totalTokens":0}}}));
    assert_eq!(
        *h.events.lock().unwrap(),
        vec![LocalChatEvent::Usage {
            input_tokens: 0,
            output_tokens: 0,
            cached_input_tokens: 0,
            total_tokens: 0
        }]
    );
}
#[test]
fn message_phase_precedes_deltas_and_survives_interleaved_items() {
    let mut h = Harness::new();
    h.notify(
        "item/started",
        json!({"item":{"type":"agentMessage","id":"a","phase":"commentary"}}),
    );
    h.notify(
        "item/agentMessage/delta",
        json!({"itemId":"a","delta":"progress"}),
    );
    h.notify(
        "item/started",
        json!({"item":{"type":"agentMessage","id":"b","phase":"final_answer"}}),
    );
    h.notify(
        "item/agentMessage/delta",
        json!({"itemId":"b","delta":"answer"}),
    );
    h.notify(
        "item/agentMessage/delta",
        json!({"itemId":"a","delta":"more"}),
    );
    assert_eq!(
        *h.events.lock().unwrap(),
        vec![
            LocalChatEvent::Phase {
                phase: "commentary".into()
            },
            LocalChatEvent::Chunk {
                text: "progress".into()
            },
            LocalChatEvent::Phase {
                phase: "final_answer".into()
            },
            LocalChatEvent::Chunk {
                text: "answer".into()
            },
            LocalChatEvent::Phase {
                phase: "commentary".into()
            },
            LocalChatEvent::Chunk {
                text: "more".into()
            }
        ]
    );
}

#[test]
fn message_phase_is_restored_after_human_wait_before_continuation() {
    let mut h = Harness::new();
    h.notify(
        "item/started",
        json!({"item":{"type":"agentMessage","id":"a","phase":"final_answer"}}),
    );
    h.notify(
        "item/agentMessage/delta",
        json!({"itemId":"a","delta":"before"}),
    );
    h.tool("input", "ai-excalidraw", "request_user_input", true);
    h.tool("input", "ai-excalidraw", "request_user_input", false);
    h.notify(
        "item/agentMessage/delta",
        json!({"itemId":"a","delta":"after"}),
    );
    let events = h.events.lock().unwrap();
    assert!(events.ends_with(&[
        LocalChatEvent::Phase {
            phase: "final_answer".into()
        },
        LocalChatEvent::Chunk {
            text: "after".into()
        }
    ]));
}
