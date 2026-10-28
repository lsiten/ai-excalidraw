use super::super::{
    error::{io, ChatError},
    events::LocalChatEvent,
    stream::Sink,
};
use super::{
    transport::send,
    watchdog::{Limits, Watchdog},
    Store,
};
use serde_json::{json, Value};
use std::path::Path;
use tokio::io::{AsyncBufReadExt, AsyncRead, AsyncReadExt, AsyncWrite, BufReader};
pub(super) struct Progress {
    pub(super) limits: Limits,
    pub(super) submitted: bool,
    pub(super) thread: Option<String>,
    pub(super) turn: Option<String>,
}

#[expect(
    clippy::too_many_lines,
    reason = "Ordered handshake and response correlation state machine"
)]
pub(super) async fn session(
    reader: impl AsyncRead + Unpin,
    writer: &mut (impl AsyncWrite + Unpin),
    prompt: &str,
    sink: &mut Sink,
    store: &mut Store,
    cwd: &Path,
    progress: &mut Progress,
) -> Result<(), ChatError> {
    let mut reader = BufReader::new(reader.take(4 * 1024 * 1024));
    let mut phase = 1u64;
    let mut watchdog = Watchdog::new(&progress.limits);
    let mut messages = super::messages::Messages::default();
    let resumed = store.thread.is_some();
    sink(LocalChatEvent::Phase {
        phase: "initializing".into(),
    })?;
    send(writer,json!({"id":1,"method":"initialize","params":{"clientInfo":{"name":"ai_excalidraw","version":env!("CARGO_PKG_VERSION")}}})).await?;
    loop {
        let mut line = String::new();
        let expiry = watchdog.expiry();
        let count = tokio::time::timeout_at(expiry, reader.read_line(&mut line))
            .await
            .map_err(|_| ChatError::Timeout)?
            .map_err(io("read app-server RPC"))?;
        if count == 0 {
            return Err(ChatError::Protocol);
        }
        let message: Value = serde_json::from_str(&line).map_err(|_| ChatError::Protocol)?;
        if message.get("method").is_none() && message["id"].as_u64() == Some(phase) {
            if message.get("error").is_some() {
                let context = match phase {
                    1 => "Codex initialization failed",
                    2 => {
                        if resumed {
                            "Codex thread resume failed"
                        } else {
                            "Codex thread start failed"
                        }
                    }
                    _ => "Codex turn start failed",
                };
                return Err(super::diagnostics::failure(context, &message["error"]));
            }
            match phase {
                1 => {
                    send(writer, json!({"method":"initialized","params":{}})).await?;
                    let mut params =
                        json!({"cwd":cwd,"sandbox":"read-only","approvalPolicy":"never"});
                    let method = if let Some(thread) = &store.thread {
                        params["threadId"] = json!(thread);
                        params["excludeTurns"] = json!(true);
                        "thread/resume"
                    } else {
                        "thread/start"
                    };
                    // No retry or exec fallback from this point, even on lost responses.
                    progress.submitted = true;
                    sink(LocalChatEvent::Phase {
                        phase: method.into(),
                    })?;
                    send(writer, json!({"id":2,"method":method,"params":params})).await?;
                    phase = 2;
                    watchdog.startup(&progress.limits);
                }
                2 => {
                    let thread = message["result"]["thread"]["id"]
                        .as_str()
                        .ok_or(ChatError::Protocol)?;
                    if resumed && store.thread.as_deref() != Some(thread) {
                        return Err(ChatError::Protocol);
                    }
                    store.save(thread)?;
                    progress.thread = Some(thread.into());
                    sink(LocalChatEvent::Session {
                        conversation_id: store.conversation.clone(),
                        session_id: thread.into(),
                        backend: "codex-app-server".into(),
                        resumed,
                    })?;
                    send(writer,json!({"id":3,"method":"turn/start","params":{"threadId":thread,"approvalPolicy":"never","sandboxPolicy":{"type":"readOnly"},"input":[{"type":"text","text":prompt}]}})).await?;
                    phase = 3;
                    watchdog.startup(&progress.limits);
                }
                3 => {
                    progress.turn = Some(
                        message["result"]["turn"]["id"]
                            .as_str()
                            .ok_or(ChatError::Protocol)?
                            .into(),
                    );
                    phase = 4;
                    watchdog.startup(&progress.limits);
                }
                _ => return Err(ChatError::Protocol),
            }
            continue;
        }
        let params = &message["params"];
        let method = message["method"].as_str().unwrap_or("");
        if let Some(id) = message.get("id") {
            sink(LocalChatEvent::Input {
                request_id: id.as_str().map_or_else(|| id.to_string(), str::to_owned),
                kind: method.into(),
                message: "Native interactive input is unsupported".into(),
                supported: false,
            })?;
            send(
                writer,
                json!({"id":id,"error":{"code":-32601,"message":"Interactive input unsupported"}}),
            )
            .await?;
            return Err(ChatError::Cli(
                "Native interactive input unsupported".into(),
            ));
        }
        if phase < 3 {
            if method == "warning" || method == "error" {
                if method == "error" && params["willRetry"] != true {
                    return Err(super::diagnostics::failure("Codex startup failed", params));
                }
                sink(super::diagnostics::status(params, method == "error"))?;
            }
            continue;
        }
        if params["threadId"]
            .as_str()
            .is_some_and(|id| Some(id) != progress.thread.as_deref())
        {
            continue;
        }
        if params["turnId"]
            .as_str()
            .is_some_and(|id| progress.turn.as_deref().is_some_and(|turn| turn != id))
        {
            continue;
        }
        if super::notifications::handle(
            method,
            params,
            sink,
            progress,
            &mut watchdog,
            &mut messages,
        )? {
            return Ok(());
        }
    }
}
