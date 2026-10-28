#![expect(clippy::unwrap_used, reason = "Fixtures must decode")]
use super::{
    tests::{chunks, fixture},
    *,
};

#[test]
fn codex_tool_updates_preserve_inputs_outputs_errors_and_deduplicate() {
    // Given exec JSONL tool lifecycles, including a repeated update and failure.
    let (mut parser, events) = fixture(ToolId::Codex);
    // When the native tool loop runs.
    for line in [
        r#"{"type":"thread.started"}"#,
        r#"{"type":"thread.started"}"#,
        r#"{"type":"turn.started"}"#,
        r#"{"type":"item.started","item":{"id":"cmd","type":"command_execution","command":"printf hi","status":"in_progress"}}"#,
        r#"{"type":"item.updated","item":{"id":"cmd","type":"command_execution","aggregated_output":"h"}}"#,
        r#"{"type":"item.updated","item":{"id":"cmd","type":"command_execution","aggregated_output":"h"}}"#,
        r#"{"type":"item.completed","item":{"id":"cmd","type":"command_execution","aggregated_output":"hi","exit_code":0}}"#,
        r#"{"type":"item.started","item":{"id":"mcp","type":"mcp_tool_call","server":"canvas","tool":"read","arguments":{"canvasId":"x"}}}"#,
        r#"{"type":"item.completed","item":{"id":"mcp","type":"mcp_tool_call","status":"failed","error":{"message":"offline"}}}"#,
        r#"{"type":"turn.completed"}"#,
    ] {
        parser.line(line.as_bytes()).unwrap();
    }
    parser.finish().unwrap();
    // Then repeated progress is absent; completed/failed calls retain their input.
    let events = events.lock().unwrap();
    assert_eq!(
        events
            .iter()
            .filter(|e| matches!(e, LocalChatEvent::Status { .. }))
            .count(),
        3
    );
    let commands: Vec<_> = events
        .iter()
        .filter(|e| matches!(e, LocalChatEvent::Tool { id, .. } if id == "cmd"))
        .collect();
    assert_eq!(commands.len(), 3);
    assert!(
        matches!(commands[1], LocalChatEvent::Tool { status: ToolStatus::Running, input: Some(input), output: Some(output), .. } if input == "printf hi" && output == "h")
    );
    assert!(
        matches!(commands[2], LocalChatEvent::Tool { status: ToolStatus::Completed, input: Some(input), output: Some(output), .. } if input == "printf hi" && output == "hi")
    );
    assert!(events.iter().any(|e| matches!(e, LocalChatEvent::Tool { id, name, status: ToolStatus::Failed, input: Some(_), output: Some(output) } if id == "mcp" && name == "canvas:read" && output.contains("offline"))));
}

#[test]
fn codex_file_changes_and_plans_are_contentful_tools_not_answer_text() {
    // Given native file and todo-list items containing structured data.
    let (mut parser, events) = fixture(ToolId::Codex);
    // When execution completes with no assistant prose.
    for line in [
        r#"{"type":"item.completed","item":{"id":"file","type":"file_change","status":"completed","changes":[{"path":"a.rs","kind":"update","diff":"-a\n+b"}]}}"#,
        r#"{"type":"item.updated","item":{"id":"plan","type":"todo_list","items":[{"text":"Read source","completed":true},{"text":"Run tests","completed":false}]}}"#,
        r#"{"type":"item.completed","item":{"id":"plan","type":"todo_list","items":[{"text":"Read source","completed":true},{"text":"Run tests","completed":true}]}}"#,
        r#"{"type":"turn.completed"}"#,
    ] {
        parser.line(line.as_bytes()).unwrap();
    }
    parser.finish().unwrap();
    // Then diffs and plan states are visible only in process activity.
    assert_eq!(chunks(&events), "");
    let events = events.lock().unwrap();
    assert!(events.iter().any(|e| matches!(e, LocalChatEvent::Tool { name, input: Some(input), .. } if name == "file_change" && input.contains("a.rs") && input.contains("diff"))));
    assert!(events.iter().any(|e| matches!(e, LocalChatEvent::Tool { name, status: ToolStatus::Running, input: Some(input), .. } if name == "update_plan" && input.contains("Run tests"))));
}

#[test]
fn terminal_failure_closes_pending_tools_without_making_error_text_an_answer() {
    // Given a still-running call when the provider rejects the turn.
    let (mut parser, events) = fixture(ToolId::Codex);
    // When the terminal failure arrives.
    for line in [
        r#"{"type":"item.started","item":{"id":"c","type":"command_execution","command":"sleep 1"}}"#,
        r#"{"type":"turn.failed","error":{"message":"interrupted"}}"#,
    ] {
        parser.line(line.as_bytes()).unwrap();
    }
    // Then the task fails, its tool stops spinning, and no error enters assistant text.
    assert!(parser.finish().is_err());
    assert_eq!(chunks(&events), "");
    assert!(matches!(
        events.lock().unwrap().last(),
        Some(LocalChatEvent::Tool {
            status: ToolStatus::Failed,
            input: Some(_),
            ..
        })
    ));
}

#[test]
fn codex_nonfatal_error_item_does_not_poison_completed_answer() {
    // Given a non-fatal error item followed by a successful answer and turn.
    let (mut parser, events) = fixture(ToolId::Codex);
    // When the entire turn is consumed, as defined by Codex exec's JSONL protocol.
    for line in [
        r#"{"type":"item.completed","item":{"id":"notice","type":"error","message":"unrecognized request argument supplied: temperature"}}"#,
        r#"{"type":"item.completed","item":{"id":"reply","type":"agent_message","text":"answer"}}"#,
        r#"{"type":"turn.completed"}"#,
    ] {
        parser.line(line.as_bytes()).unwrap();
    }
    // Then the notice stays in process activity without rejecting the completed answer.
    assert!(parser.finish().is_ok());
    assert_eq!(chunks(&events), "answer");
    assert!(events
        .lock()
        .unwrap()
        .iter()
        .any(|e| matches!(e, LocalChatEvent::Status { .. })));
}

#[test]
fn codex_no_tool_turn_reports_reply_and_completion_with_usage() {
    // Given a real Codex JSONL turn with no tools or reasoning summary.
    let (mut parser, events) = fixture(ToolId::Codex);
    // When its session, reply and completion events arrive.
    for line in [
        r#"{"type":"thread.started","thread_id":"qa"}"#,
        r#"{"type":"turn.started"}"#,
        r#"{"type":"item.completed","item":{"id":"reply","type":"agent_message","text":"OK"}}"#,
        r#"{"type":"turn.completed","usage":{"input_tokens":120,"cached_input_tokens":40,"output_tokens":18}}"#,
    ] {
        parser.line(line.as_bytes()).unwrap();
    }
    parser.finish().unwrap();
    // Then milestones and actual usage survive independently of final answer text.
    assert_eq!(chunks(&events), "OK");
    let events = events.lock().unwrap();
    assert_eq!(
        events
            .iter()
            .filter(|e| matches!(e, LocalChatEvent::Status { .. }))
            .count(),
        4
    );
    assert!(
        matches!(events.last(), Some(LocalChatEvent::Status { message }) if message.contains("120") && message.contains("18"))
    );
}

#[test]
fn codex_provider_failure_is_not_reported_as_an_unsupported_cli_flag() {
    // Given a fatal provider response whose wording resembles an argument-parser error.
    for event in [
        serde_json::json!({"type": "turn.failed", "error": {"message": "unexpected argument 'temperature'"}}),
        serde_json::json!({"type": "error", "message": "unexpected argument 'temperature'"}),
    ] {
        let (mut parser, _) = fixture(ToolId::Codex);
        // When an actual terminal failure is parsed.
        parser.line(event.to_string().as_bytes()).unwrap();
        // Then it remains a failure, without blaming CLI flags or asking for login.
        let error = parser.finish().unwrap_err().to_string();
        assert!(!error.contains("非交互参数"), "{error}");
        assert!(!error.contains("codex login"), "{error}");
    }
}

#[test]
fn cursor_nested_id_and_nonzero_shell_exit_are_preserved_as_failure() {
    // Given a current Cursor shell result without the redundant outer call_id.
    let (mut parser, events) = fixture(ToolId::Cursor);
    // When a nonzero exit arrives inside the success envelope.
    for line in [
        r#"{"type":"tool_call","subtype":"started","tool_call":{"toolCallId":"c","shellToolCall":{"args":{"command":"false"}}}}"#,
        r#"{"type":"tool_call","subtype":"completed","tool_call":{"toolCallId":"c","shellToolCall":{"result":{"success":{"exitCode":1,"stderr":"denied"}}}}}"#,
    ] {
        parser.line(line.as_bytes()).unwrap();
    }
    // Then transport success is not mistaken for command success.
    assert!(
        matches!(events.lock().unwrap().last(), Some(LocalChatEvent::Tool { id, status: ToolStatus::Failed, input: Some(_), output: Some(_), .. }) if id == "c")
    );
}
