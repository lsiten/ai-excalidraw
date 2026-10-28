#![expect(clippy::unwrap_used, reason = "Fixtures must decode")]
use super::{
    tests::{chunks, fixture},
    *,
};

#[test]
fn cursor_real_capture_does_not_echo_prompt_or_duplicate_final_result() {
    // Given the unmodified Cursor 2026.07.20 capture from Multica.
    let (mut parser, events) = fixture(ToolId::Cursor);
    // When the full native stream is replayed.
    for line in include_str!("fixtures/cursor-agent-2026.07.20.jsonl").lines() {
        parser.line(line.as_bytes()).unwrap();
    }
    parser.finish().unwrap();
    // Then only the two assistant messages enter the answer, once each.
    assert_eq!(chunks(&events), "I'll read `notes.txt`, run `wc -l`, then write the line count to `count.txt`.`notes.txt` has 3 lines (`alpha`, `beta`, `gamma`). Wrote `3` to `count.txt`.");
    let events = events.lock().unwrap();
    let tools: Vec<_> = events
        .iter()
        .filter(|e| matches!(e, LocalChatEvent::Tool { .. }))
        .collect();
    assert_eq!(tools.len(), 6);
    assert!(tools
        .iter()
        .all(|e| matches!(e, LocalChatEvent::Tool { input: Some(_), .. })));
}

#[test]
fn claude_partial_blocks_stream_before_snapshots_without_repetition() {
    // Given an SDK-shaped partial stream with separately completed assistant blocks.
    let (mut parser, events) = fixture(ToolId::Claude);
    let lines: Vec<_> = include_str!("fixtures/claude-partials.jsonl")
        .lines()
        .collect();
    // When deltas arrive, before the terminal result or completed snapshots.
    for line in &lines[..9] {
        parser.line(line.as_bytes()).unwrap();
    }
    assert!(!parser.terminal);
    assert_eq!(chunks(&events), "读取画布");
    for line in &lines[9..] {
        parser.line(line.as_bytes()).unwrap();
    }
    parser.finish().unwrap();
    // Then snapshots do not duplicate either text channel or reset tool payloads.
    assert_eq!(chunks(&events), "读取画布已读取");
    let events = events.lock().unwrap();
    assert_eq!(
        events
            .iter()
            .filter(|e| matches!(e, LocalChatEvent::Thinking { .. }))
            .count(),
        1
    );
    assert!(events.iter().any(|e| matches!(e, LocalChatEvent::Tool { status: ToolStatus::Running, input: Some(input), .. } if input == "{\"canvasId\":")));
    assert!(events.iter().any(|e| matches!(e, LocalChatEvent::Tool { status: ToolStatus::Completed, input: Some(input), output: Some(_), .. } if input.contains("canvas-1"))));
}

#[test]
fn pi_updates_stream_and_end_snapshots_do_not_repeat() {
    // Given Pi's installed AgentEvent/AssistantMessageEvent wire shapes.
    let (mut parser, events) = fixture(ToolId::Pi);
    // When tool output and message_end/turn_end/agent_end replay the same content.
    for line in include_str!("fixtures/pi-tools.jsonl").lines() {
        parser.line(line.as_bytes()).unwrap();
    }
    parser.finish().unwrap();
    // Then tool progress stays running, has its input, and all prose appears once.
    assert_eq!(chunks(&events), "checkingfinal answer");
    let events = events.lock().unwrap();
    assert_eq!(
        events
            .iter()
            .filter(|e| matches!(e, LocalChatEvent::Thinking { .. }))
            .count(),
        1
    );
    assert!(events.iter().any(|e| matches!(e, LocalChatEvent::Tool { status: ToolStatus::Running, output: Some(output), input: Some(_), .. } if output.contains("\"h\""))));
    assert!(matches!(events.last(), Some(LocalChatEvent::Chunk { .. })));
    let terminal_tool = events
        .iter()
        .rfind(|e| matches!(e, LocalChatEvent::Tool { .. }));
    assert!(matches!(
        terminal_tool,
        Some(LocalChatEvent::Tool {
            status: ToolStatus::Completed,
            input: Some(_),
            output: Some(_),
            ..
        })
    ));
}

#[test]
fn claude_real_context_exhaustion_is_failure_even_with_success_subtype() {
    // Given Multica's unmodified 2.1.220 exhausted-context capture.
    let (mut parser, _) = fixture(ToolId::Claude);
    // When the CLI reports subtype=success together with is_error=true.
    for line in include_str!("fixtures/claude-context-exhausted.jsonl").lines() {
        parser.line(line.as_bytes()).unwrap();
    }
    // Then a zero process exit cannot turn provider failure into success.
    assert!(parser.finish().is_err());
}

#[test]
fn cursor_partial_flushes_are_not_new_text() {
    // Given the three assistant envelope types documented for --stream-partial-output.
    let (mut parser, events) = fixture(ToolId::Cursor);
    // When new deltas are followed by pre-tool and final buffered flushes.
    for line in [
        r#"{"type":"assistant","timestamp_ms":1,"message":{"content":[{"type":"text","text":"foo"}]}}"#,
        r#"{"type":"assistant","timestamp_ms":2,"message":{"content":[{"type":"text","text":"bar"}]}}"#,
        r#"{"type":"assistant","timestamp_ms":3,"model_call_id":"m","message":{"content":[{"type":"text","text":"foobar"}]}}"#,
        r#"{"type":"assistant","message":{"content":[{"type":"text","text":"foobar"}]}}"#,
        r#"{"type":"result","subtype":"success","result":"foobar"}"#,
    ] {
        parser.line(line.as_bytes()).unwrap();
    }
    parser.finish().unwrap();
    // Then the answer contains only actual streamed text.
    assert_eq!(chunks(&events), "foobar");
}
