#![expect(
    clippy::unwrap_used,
    reason = "Tests fail immediately on fixture errors"
)]
use super::{
    cli::{self, Invocation},
    error::ChatError,
    process,
    state::LocalChatState,
};
use crate::integrations::ToolId;
use std::{path::Path, process::Stdio, time::Duration};
use tokio::{
    io::{AsyncBufReadExt, AsyncReadExt, BufReader},
    net::TcpListener,
    process::Command,
};

fn stub(script: &str) -> Command {
    let mut command = Command::new("/bin/bash");
    command
        .args(["-c", script])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    command
}

#[tokio::test]
async fn concurrent_pipe_draining_avoids_stdin_stdout_deadlock() {
    // Given a child that fills stdout before reading a larger-than-pipe prompt.
    let state = LocalChatState::default();
    let mut lease = state.register("pipes".into(), "prompt").unwrap();
    let mut command = stub("head -c 131072 /dev/zero; cat");
    let prompt = "画".repeat(65536);
    // When input and output exceed OS pipe capacity simultaneously.
    let captured = process::capture(&mut command, &prompt, &mut lease.control)
        .await
        .unwrap();
    // Then the child completes and all prompt bytes arrive intact.
    assert!(captured.status.success());
    assert_eq!(&captured.stdout[131_072..], prompt.as_bytes());
}

#[tokio::test]
async fn output_limit_rejects_flooding_child() {
    // Given a child writing indefinitely.
    let state = LocalChatState::default();
    let mut lease = state.register("flood".into(), "prompt").unwrap();
    let mut command = stub("exec cat /dev/zero");
    // When it exceeds the response limit.
    let result = process::capture(&mut command, "prompt", &mut lease.control).await;
    // Then the child is terminated rather than buffering indefinitely.
    assert!(matches!(result, Err(ChatError::OutputLimit)));
}

#[tokio::test]
async fn timeout_covers_blocked_stdin() {
    // Given a child that never reads stdin.
    let state = LocalChatState::default();
    let mut lease = state.register("timeout".into(), "prompt").unwrap();
    lease.control.duration = Duration::ZERO;
    let mut command = stub("exec sleep 60");
    // When a large prompt cannot drain before the deadline.
    let result = process::capture(&mut command, &"x".repeat(131_072), &mut lease.control).await;
    // Then the entire operation times out, including the stdin write.
    assert!(matches!(result, Err(ChatError::Timeout)));
}

#[tokio::test]
async fn bounded_reader_distinguishes_exact_limit_from_overflow() {
    // Given output exactly at the boundary and one byte beyond it.
    // When reading both through the production bounded reader.
    let exact = process::read_bounded(&b"1234"[..], 4).await.unwrap();
    let overflow = process::read_bounded(&b"12345"[..], 4).await;
    // Then exact output survives and oversized output rejects.
    assert_eq!(exact, b"1234");
    assert!(matches!(overflow, Err(ChatError::OutputLimit)));
}

#[tokio::test]
async fn terminal_event_stops_lingering_cli_and_cleans_runtime() {
    // Given a valid result followed by a CLI that never exits.
    let state = LocalChatState::default();
    let mut lease = state.register("terminal".into(), "prompt").unwrap();
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().to_path_buf();
    let command = stub(
        r#"cat >/dev/null; printf '%s\n' '{"type":"result","subtype":"success","result":"完成"}'; sleep 60"#,
    );
    let invocation = Invocation {
        command,
        directory,
        tool: ToolId::Claude,
        project_root: None,
    };
    let events = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
    let sink = std::sync::Arc::clone(&events);
    // When running through the native stream supervisor.
    tokio::time::timeout(
        Duration::from_secs(5),
        process::execute(
            invocation,
            process::ChatInput {
                prompt: "prompt",
                events: Box::new(move |event| {
                    sink.lock().unwrap().push(event);
                    Ok(())
                }),
            },
            &mut lease.control,
        ),
    )
    .await
    .unwrap()
    .unwrap();
    // Then protocol completion wins over a lingering worker, with one final chunk and RAII cleanup.
    assert_eq!(
        events.lock().unwrap().as_slice(),
        &[super::LocalChatEvent::Chunk {
            text: "完成".into()
        }]
    );
    assert!(!path.exists());
}

#[tokio::test]
async fn spawn_failure_cleans_temp_directory() {
    // Given a CLI removed between discovery and spawn.
    let state = LocalChatState::default();
    let mut lease = state.register("spawn-error".into(), "prompt").unwrap();
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().to_path_buf();
    let command = cli::command(ToolId::Pi, &path.join("missing"), Path::new(&path));
    // When spawning fails.
    let result = process::execute(
        Invocation {
            command,
            directory,
            tool: ToolId::Pi,
            project_root: None,
        },
        process::ChatInput {
            prompt: "prompt",
            events: Box::new(|_| Ok(())),
        },
        &mut lease.control,
    )
    .await;
    // Then the disposable runtime is cleaned on the error path too.
    assert!(matches!(result, Err(ChatError::Io { .. })));
    assert!(!path.exists());
}

#[derive(Clone, Copy)]
enum Stop {
    Cancel,
    Shutdown,
    Abort,
}

async fn descendant_cleanup(stop: Stop) {
    let state = LocalChatState::default();
    let mut lease = state.register("tree".into(), "prompt").unwrap();
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    // The descendant inherits fd 3. EOF proves it died too, without polling PIDs.
    let mut command = stub(&format!(
        "exec 3<>/dev/tcp/127.0.0.1/{port}; sleep 60 & printf 'ready\\n' >&3; wait"
    ));
    let task =
        tokio::spawn(
            async move { process::capture(&mut command, "prompt", &mut lease.control).await },
        );
    let (socket, _) = tokio::time::timeout(Duration::from_secs(5), listener.accept())
        .await
        .unwrap()
        .unwrap();
    let mut socket = BufReader::new(socket);
    let mut ready = String::new();
    tokio::time::timeout(Duration::from_secs(5), socket.read_line(&mut ready))
        .await
        .unwrap()
        .unwrap();
    match stop {
        Stop::Cancel => state.cancel("tree").unwrap(),
        Stop::Shutdown => state.cancel_all(),
        Stop::Abort => task.abort(),
    }
    let result = task.await;
    match stop {
        Stop::Cancel | Stop::Shutdown => {
            assert!(matches!(result.unwrap(), Err(ChatError::Cancelled)));
        }
        Stop::Abort => assert!(result.unwrap_err().is_cancelled()),
    }
    let mut remainder = Vec::new();
    tokio::time::timeout(Duration::from_secs(5), socket.read_to_end(&mut remainder))
        .await
        .unwrap()
        .unwrap();
}

#[tokio::test]
async fn cancellation_kills_descendants_holding_open_handles() {
    // Given a shell and child with inherited handles; when cancelled; then both die.
    descendant_cleanup(Stop::Cancel).await;
}

#[tokio::test]
async fn shutdown_kills_descendants_before_async_cleanup() {
    // Given an active CLI tree; when the app exits; then both processes die.
    descendant_cleanup(Stop::Shutdown).await;
}

#[tokio::test]
async fn aborting_future_kills_descendants_on_drop() {
    // Given an active CLI tree; when its future is dropped; then both processes die.
    descendant_cleanup(Stop::Abort).await;
}
