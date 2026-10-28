#![expect(
    clippy::unwrap_used,
    reason = "Tests fail immediately on fixture errors"
)]
use super::{cli, error::ChatError, state::LocalChatState};
use crate::integrations::ToolId;
use std::path::Path;

#[test]
fn argv_retains_native_tools_and_mcp_without_permission_bypasses() {
    // Given each supported native agent.
    for tool in ToolId::ALL {
        // When constructing argv.
        let command = cli::command(tool, Path::new("/stub/tool"), Path::new("/project"));
        let args: Vec<_> = command
            .as_std()
            .get_args()
            .map(|s| s.to_string_lossy().into_owned())
            .collect();
        // Then JSON transports are selected and global native resources are retained.
        assert!(!args.iter().any(|s| s.contains("dangerously")
            || s == "--force"
            || s == "--yolo"
            || s == "--approve-mcps"
            || s == "--no-skills"
            || s == "--no-mcp"
            || s == "--strict-mcp-config"
            || s == "--tools"));
        let pair = |a: &str, b: &str| args.windows(2).any(|p| p == [a, b]);
        match tool {
            ToolId::Codex => {
                assert!(args.iter().any(|s| s == "--json"));
                assert!(pair("--sandbox", "read-only"));
            }
            ToolId::Claude => {
                assert!(pair("--output-format", "stream-json"));
                assert!(args.iter().any(|s| s == "--include-partial-messages"));
            }
            ToolId::Cursor => {
                assert!(pair("--output-format", "stream-json"));
                assert!(args.iter().any(|s| s == "--stream-partial-output"));
            }
            ToolId::Pi => assert!(pair("--mode", "json")),
        }
    }
}

#[test]
fn actual_cli_parser_failures_still_report_incompatible_arguments() {
    // Given stderr from an actual CLI argument parser, rather than an agent event.
    for diagnostic in [
        "error: unexpected argument '--ephemeral' found",
        "error: unknown option '--json'",
        "unexpected argument 'temperature'",
    ] {
        // When reporting a child that failed before producing a terminal event.
        let error = super::error::cli_failure(
            ToolId::Codex,
            diagnostic,
            super::error::FailureOrigin::Command,
        );
        // Then version guidance remains available without irrelevant login instructions.
        assert!(error.to_string().contains("非交互参数"));
        assert!(!error.to_string().contains("codex login"));
    }
}

#[test]
fn stop_before_start_is_remembered_without_spawning() {
    // Given an early Stop IPC.
    let state = LocalChatState::default();
    state.cancel("early-stop").unwrap();
    // When its Start IPC is polled later.
    let result = state.register("early-stop".into(), "prompt");
    // Then the request rejects before resolving or spawning any CLI.
    assert!(matches!(result, Err(ChatError::Cancelled)));
}

#[test]
fn active_ids_and_capacity_are_released_by_raii() {
    // Given four active requests, including a cancelled one not yet finished.
    let state = LocalChatState::default();
    let mut leases = Vec::new();
    for i in 0..4 {
        leases.push(state.register(format!("id-{i}"), "prompt").unwrap());
    }
    state.cancel("id-0").unwrap();
    assert!(matches!(
        state.register("id-0".into(), "prompt"),
        Err(ChatError::Duplicate)
    ));
    assert!(matches!(
        state.register("fifth".into(), "prompt"),
        Err(ChatError::Busy)
    ));
    // When the finished request's lease drops.
    drop(leases.remove(0));
    // Then its slot and ID are reusable.
    assert!(state.register("id-0".into(), "prompt").is_ok());
}

#[test]
fn shutdown_prevents_pending_starts() {
    // Given the app has started shutdown.
    let state = LocalChatState::default();
    state.cancel_all();
    // When a pending async start is polled.
    let result = state.register("late".into(), "prompt");
    // Then it cannot spawn a CLI after the shutdown hook ran.
    assert!(matches!(result, Err(ChatError::Cancelled)));
}

#[test]
fn canvas_context_accepts_long_paths_and_rejects_cross_project_ids() {
    // Given a supported long canvas ID and another project's canvas.
    let long = format!("project:{}", "a".repeat(2000));
    // When validating the IPC context before discovery/spawn.
    let accepted = super::state::validate_canvas(Some("project"), Some(&long));
    let rejected =
        super::state::validate_canvas(Some("project"), Some("different:canvas.excalidraw"));
    let oversized = super::state::validate_canvas(None, Some(&"x".repeat(2101)));
    // Then registered-project binding allows valid paths and rejects mismatches/oversized IDs.
    assert!(accepted.is_ok());
    assert!(matches!(rejected, Err(ChatError::CanvasContext)));
    assert!(matches!(oversized, Err(ChatError::CanvasContext)));
}
