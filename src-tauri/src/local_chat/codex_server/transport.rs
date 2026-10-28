use super::super::{
    cli::Invocation,
    error::{io, ChatError},
    process::{Process, ProcessControl},
    state::cancelled,
    stream::Sink,
};
use super::{
    session::{session, Progress},
    watchdog::Limits,
    Store,
};
use serde_json::{json, Value};
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWrite, AsyncWriteExt};
pub(super) async fn send(
    writer: &mut (impl AsyncWrite + Unpin),
    value: Value,
) -> Result<(), ChatError> {
    let mut bytes = serde_json::to_vec(&value).map_err(|_| ChatError::Protocol)?;
    bytes.push(b'\n');
    tokio::time::timeout(Duration::from_secs(5), writer.write_all(&bytes))
        .await
        .map_err(|_| ChatError::Timeout)?
        .map_err(io("write app-server RPC"))
}
pub(in crate::local_chat) async fn execute(
    mut invocation: Invocation,
    prompt: &str,
    sink: &mut Sink,
    control: &mut ProcessControl,
    store: &mut Store,
) -> Result<(), ChatError> {
    // Native cancellation must get an opportunity to send turn/interrupt before killing the tree.
    let mut native_control = ProcessControl {
        cancel: control.cancel.clone(),
        process: std::sync::Arc::default(),
        duration: control.duration.min(super::super::process::TIME_LIMIT),
    };
    let mut process = Process::spawn(&mut invocation.command, &native_control)?;
    let mut stdin = process.child.stdin.take().ok_or(ChatError::Channel)?;
    let stdout = process.child.stdout.take().ok_or(ChatError::Channel)?;
    let stderr = process.child.stderr.take().ok_or(ChatError::Channel)?;
    let mut progress = Progress {
        limits: Limits::default(),
        submitted: false,
        thread: None,
        turn: None,
    };
    let cwd = invocation
        .project_root
        .as_deref()
        .unwrap_or(invocation.directory.path());
    let operation = async {
        let protocol = session(stdout, &mut stdin, prompt, sink, store, cwd, &mut progress);
        let diagnostics = async {
            let mut reader = stderr.take(65537);
            let mut bytes = Vec::new();
            reader
                .read_to_end(&mut bytes)
                .await
                .map_err(io("read app-server diagnostics"))?;
            if bytes.len() > 65536 {
                return Err(ChatError::OutputLimit);
            }
            std::future::pending::<Result<(), ChatError>>().await
        };
        tokio::pin!(protocol, diagnostics);
        tokio::select! { result=&mut protocol => result, result=&mut diagnostics => result }
    };
    let result = tokio::select! {
        biased;
        ()=cancelled(&mut native_control.cancel)=>Err(ChatError::Cancelled),
        result=tokio::time::timeout(native_control.duration,operation)=>result.unwrap_or(Err(ChatError::Timeout)),
    };
    if result.is_err() {
        if let (Some(thread), Some(turn)) = (&progress.thread, &progress.turn) {
            match tokio::time::timeout(Duration::from_secs(2),send(&mut stdin,json!({"id":99,"method":"turn/interrupt","params":{"threadId":thread,"turnId":turn}}))).await {
                Ok(Ok(()))=>{}, Ok(Err(error))=>eprintln!("Codex interrupt failed: {error}"), Err(error)=>eprintln!("Codex interrupt timeout: {error}"),
            }
        }
    }
    drop(stdin);
    match tokio::time::timeout(Duration::from_secs(2), process.child.wait()).await {
        Ok(Ok(_)) => {}
        Ok(Err(error)) => {
            eprintln!("Cannot reap Codex app-server: {error}");
            process.stop().await;
        }
        Err(_) => process.stop().await,
    }
    result
}
