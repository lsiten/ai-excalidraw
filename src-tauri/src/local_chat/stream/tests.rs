#![expect(
    clippy::unwrap_used,
    reason = "Tests fail immediately on fixture errors"
)]
use super::*;
use std::sync::{Arc, Mutex};

pub(super) fn fixture(tool: ToolId) -> (Parser, Arc<Mutex<Vec<LocalChatEvent>>>) {
    let events = Arc::new(Mutex::new(Vec::new()));
    let sink = Arc::clone(&events);
    (
        Parser::new(
            tool,
            Box::new(move |event| {
                sink.lock().unwrap().push(event);
                Ok(())
            }),
        ),
        events,
    )
}
pub(super) fn chunks(events: &Mutex<Vec<LocalChatEvent>>) -> String {
    events
        .lock()
        .unwrap()
        .iter()
        .filter_map(|event| match event {
            LocalChatEvent::Chunk { text } => Some(text.as_str()),
            LocalChatEvent::Thinking { .. }
            | LocalChatEvent::Tool { .. }
            | LocalChatEvent::Session { .. }
            | LocalChatEvent::Phase { .. }
            | LocalChatEvent::Plan { .. }
            | LocalChatEvent::Usage { .. }
            | LocalChatEvent::Input { .. }
            | LocalChatEvent::Status { .. } => None,
        })
        .collect()
}

#[test]
fn cursor_multiple_deltas_and_final_snapshot_are_emitted_once() {
    // Given two assistant deltas and a matching final result.
    let (mut parser, events) = fixture(ToolId::Cursor);
    // When the native stream is decoded.
    for line in [
        r#"{"type":"assistant","message":{"content":[{"type":"text","text":"foo"}]}}"#,
        r#"{"type":"assistant","message":{"content":[{"type":"text","text":"bar"}]}}"#,
        r#"{"type":"result","subtype":"success","result":"foobar"}"#,
    ] {
        parser.line(line.as_bytes()).unwrap();
    }
    parser.finish().unwrap();
    // Then the aggregate assistant text is not duplicated by its final snapshot.
    assert_eq!(chunks(&events), "foobar");
}

#[test]
fn codex_item_snapshots_deduplicate_and_keep_thinking_separate() {
    // Given growing snapshots for one item and a reasoning item.
    let (mut parser, events) = fixture(ToolId::Codex);
    // When updated and completed snapshots arrive.
    for line in [
        r#"{"type":"item.completed","item":{"id":"r","type":"reasoning","text":"consider"}}"#,
        r#"{"type":"item.updated","item":{"id":"a","type":"agent_message","text":"你好"}}"#,
        r#"{"type":"item.completed","item":{"id":"a","type":"agent_message","text":"你好 🌈"}}"#,
        r#"{"type":"turn.completed"}"#,
    ] {
        parser.line(line.as_bytes()).unwrap();
    }
    parser.finish().unwrap();
    // Then only assistant text enters canvas parsing.
    assert_eq!(chunks(&events), "你好 🌈");
    assert!(events.lock().unwrap().contains(&LocalChatEvent::Thinking {
        text: "consider".into()
    }));
}

#[test]
fn claude_final_after_tool_does_not_reuse_narration_candidate() {
    // Given narration and a native tool call followed by a result-only answer.
    let (mut parser, events) = fixture(ToolId::Claude);
    // When the tool turn completes.
    for line in [
        r#"{"type":"assistant","message":{"id":"m","content":[{"type":"text","text":"checking"},{"type":"tool_use","id":"call","name":"read","input":{"path":"a"}}]}}"#,
        r#"{"type":"user","message":{"content":[{"type":"tool_result","tool_use_id":"call","content":"data"}]}}"#,
        r#"{"type":"result","subtype":"success","result":"answer"}"#,
    ] {
        parser.line(line.as_bytes()).unwrap();
    }
    parser.finish().unwrap();
    // Then the answer is delivered and tools retain their call identity/name.
    assert_eq!(chunks(&events), "checkinganswer");
    assert!(events.lock().unwrap().iter().any(|e| matches!(e, LocalChatEvent::Tool { id, name, status: ToolStatus::Completed, .. } if id == "call" && name == "read")));
}

#[test]
fn cursor_unknown_tool_progress_never_completes_a_call() {
    // Given a new subtype not understood by this adapter.
    let (mut parser, events) = fixture(ToolId::Cursor);
    // When the future progress event is decoded.
    parser.line(br#"{"type":"tool_call","subtype":"progress","call_id":"call","tool_call":{"readToolCall":{"args":{}}}}"#).unwrap();
    // Then it produces no false tool boundary or assistant text.
    assert!(events.lock().unwrap().is_empty());
}

#[test]
fn pi_retry_recovers_before_agent_end_and_deltas_do_not_duplicate() {
    // Given an error turn followed by successful retry deltas and snapshots.
    let (mut parser, events) = fixture(ToolId::Pi);
    // When the native retry loop ends successfully.
    for line in [
        r#"{"type":"turn_end","message":{"role":"assistant","stopReason":"error","errorMessage":"rate limit"}}"#,
        r#"{"type":"agent_end","messages":[{"role":"assistant","stopReason":"error","errorMessage":"rate limit"}],"willRetry":true}"#,
        r#"{"type":"auto_retry_start"}"#,
        r#"{"type":"turn_start"}"#,
        r#"{"type":"message_update","assistantMessageEvent":{"type":"text_delta","delta":"foo"}}"#,
        r#"{"type":"message_update","assistantMessageEvent":{"type":"text_delta","delta":"bar"}}"#,
        r#"{"type":"turn_end","message":{"role":"assistant","stopReason":"stop","content":[{"type":"text","text":"foobar"}]}}"#,
        r#"{"type":"agent_end","messages":[{"role":"assistant","stopReason":"stop","content":[{"type":"text","text":"foobar"}]}]}"#,
    ] {
        parser.line(line.as_bytes()).unwrap();
    }
    parser.finish().unwrap();
    // Then only the successful retry is terminal, with one assistant response.
    assert_eq!(chunks(&events), "foobar");
}

#[test]
fn pi_zero_exit_agent_end_error_still_fails() {
    // Given a provider error represented only by the final assistant state.
    let (mut parser, _) = fixture(ToolId::Pi);
    // When the terminal event arrives.
    parser.line(br#"{"type":"agent_end","messages":[{"role":"assistant","stopReason":"error","errorMessage":"401 secret-token"}]}"#).unwrap();
    // Then the command fails with actionable diagnostics without leaking protocol contents.
    let error = parser.finish().unwrap_err().to_string();
    assert!(error.contains("/login"));
    assert!(!error.contains("secret-token"));
}

#[test]
fn zero_exit_without_terminal_is_not_success() {
    // Given valid but incomplete text from any native tool.
    for tool in ToolId::ALL {
        let (parser, _) = fixture(tool);
        // When stdout closes without its terminal event.
        let result = parser.finish();
        // Then exit status alone cannot certify completion.
        assert!(matches!(result, Err(ChatError::Protocol)));
    }
}

#[test]
fn optional_tool_fields_are_omitted_from_wire_contract() {
    // Given a tool activity with no payloads.
    let event = LocalChatEvent::Tool {
        id: "call".into(),
        name: "read".into(),
        status: ToolStatus::Running,
        input: None,
        output: None,
    };
    // When serializing for the frontend schema.
    let value = serde_json::to_value(event).unwrap();
    // Then missing payloads are absent, not null.
    assert_eq!(
        value,
        serde_json::json!({"type":"tool","id":"call","name":"read","status":"running"})
    );
}
