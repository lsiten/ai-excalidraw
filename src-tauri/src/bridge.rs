use axum::{
    extract::State,
    http::{HeaderMap, StatusCode},
    routing::post,
    Json, Router,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{
    collections::HashMap,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    time::Duration,
};
use tauri::{AppHandle, Emitter, Manager};
use tokio::sync::{oneshot, Mutex};
use uuid::Uuid;

type Reply = Result<Value, String>;

pub struct BridgeState {
    app: AppHandle,
    token: String,
    port: u16,
    ready: AtomicBool,
    pending: Mutex<HashMap<Uuid, oneshot::Sender<Reply>>>,
    serial: Mutex<()>,
}

#[derive(Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct CanvasCommand {
    pub tool: String,
    #[serde(default)]
    pub arguments: Value,
}

#[derive(Clone, Serialize)]
struct CanvasRequest {
    id: Uuid,
    command: CanvasCommand,
}

pub fn authorized(headers: &HeaderMap, token: &str) -> bool {
    !headers.contains_key("origin")
        && headers.get("authorization").and_then(|h| h.to_str().ok())
            == Some(format!("Bearer {token}").as_str())
}

async fn command(
    State(state): State<Arc<BridgeState>>,
    headers: HeaderMap,
    Json(command): Json<CanvasCommand>,
) -> (StatusCode, Json<Value>) {
    if !authorized(&headers, &state.token) {
        return (
            StatusCode::UNAUTHORIZED,
            Json(json!({"error": "Unauthorized"})),
        );
    }
    let (status, value) = execute(&state, command).await;
    (status, Json(value))
}

pub async fn execute(state: &BridgeState, command: CanvasCommand) -> (StatusCode, Value) {
    if !state.ready.load(Ordering::Acquire) {
        return (
            StatusCode::SERVICE_UNAVAILABLE,
            json!({"error": "Canvas is loading. Retry when the editor is visible."}),
        );
    }
    // Serialize mutations so consecutive tool calls observe the preceding scene.
    let Ok(_permit) = state.serial.try_lock() else {
        return (
            StatusCode::CONFLICT,
            json!({"error": "Canvas is busy; retry after the previous operation"}),
        );
    };
    let id = Uuid::new_v4();
    let timeout = match command.tool.as_str() {
        "request_user_input" | "remember_project_fact" => Duration::from_secs(300),
        "write_skill" => Duration::from_secs(60),
        _ => Duration::from_secs(15),
    };
    let (sender, receiver) = oneshot::channel();
    state.pending.lock().await.insert(id, sender);
    if let Err(error) = state
        .app
        .emit_to("main", "canvas-command", CanvasRequest { id, command })
    {
        state.pending.lock().await.remove(&id);
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            json!({"error": error.to_string()}),
        );
    }
    let response = tokio::time::timeout(timeout, receiver).await;
    state.pending.lock().await.remove(&id);
    match response {
        Ok(Ok(Ok(value))) => (StatusCode::OK, value),
        Ok(Ok(Err(error))) => (StatusCode::UNPROCESSABLE_ENTITY, json!({"error": error})),
        Ok(Err(error)) => (
            StatusCode::SERVICE_UNAVAILABLE,
            json!({"error": error.to_string()}),
        ),
        Err(_) => (
            StatusCode::GATEWAY_TIMEOUT,
            json!({"error": "Canvas did not respond. Open the app and retry; inspect the scene before retrying a mutation."}),
        ),
    }
}

impl BridgeState {
    pub fn is_authorized(&self, headers: &HeaderMap) -> bool {
        authorized(headers, &self.token)
    }
}

#[tauri::command]
pub async fn complete_canvas_command(
    state: tauri::State<'_, Arc<BridgeState>>,
    id: Uuid,
    result: Option<Value>,
    error: Option<String>,
) -> Result<(), String> {
    if let Some(sender) = state.pending.lock().await.remove(&id) {
        let reply = match error {
            Some(error) => Err(error),
            None => Ok(result.unwrap_or(Value::Null)),
        };
        sender
            .send(reply)
            .map_err(|_| "Tool caller disconnected".to_owned())?;
    }
    Ok(())
}

fn publish_connection(state: &BridgeState) -> Result<(), Box<dyn std::error::Error>> {
    let directory = state.app.path().app_data_dir()?;
    std::fs::create_dir_all(&directory)?;
    let path = directory.join("bridge.json");
    let info = serde_json::to_vec(
        &json!({"port": state.port, "token": state.token, "pid": std::process::id(), "version": env!("CARGO_PKG_VERSION"), "mcpPath": "/mcp"}),
    )?;
    #[cfg(unix)]
    {
        use std::{io::Write, os::unix::fs::OpenOptionsExt};
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .mode(0o600)
            .open(&path)?;
        file.write_all(&info)?;
    }
    #[cfg(not(unix))]
    std::fs::write(&path, &info)?;
    state.ready.store(true, Ordering::Release);
    Ok(())
}

#[tauri::command]
#[expect(
    clippy::needless_pass_by_value,
    reason = "Tauri extracts command state arguments by value"
)]
pub fn canvas_bridge_ready(state: tauri::State<'_, Arc<BridgeState>>) -> Result<(), String> {
    publish_connection(&state).map_err(|error| error.to_string())
}

pub fn start(app: &AppHandle) -> Result<(), Box<dyn std::error::Error>> {
    let listener = std::net::TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, 0))?;
    listener.set_nonblocking(true)?;
    let port = listener.local_addr()?.port();
    let state = Arc::new(BridgeState {
        app: app.clone(),
        token: Uuid::new_v4().to_string(),
        port,
        ready: AtomicBool::new(false),
        pending: Mutex::new(HashMap::new()),
        serial: Mutex::new(()),
    });
    app.manage(Arc::clone(&state));
    tauri::async_runtime::spawn(async move {
        let result = async {
            let listener = tokio::net::TcpListener::from_std(listener)?;
            axum::serve(
                listener,
                Router::new()
                    .route("/command", post(command))
                    .route("/mcp", post(crate::mcp::handle))
                    .with_state(state),
            )
            .await
        }
        .await;
        if let Err(error) = result {
            eprintln!("Local canvas bridge stopped: {error}");
        }
    });
    Ok(())
}

pub fn cleanup(app: &AppHandle) {
    let result = (|| -> Result<(), Box<dyn std::error::Error>> {
        let path = app.path().app_data_dir()?.join("bridge.json");
        if !path.exists() {
            return Ok(());
        }
        let value: Value = serde_json::from_slice(&std::fs::read(&path)?)?;
        if value["pid"].as_u64() == Some(u64::from(std::process::id())) {
            std::fs::remove_file(path)?;
        }
        Ok(())
    })();
    if let Err(error) = result {
        eprintln!("Cannot remove local MCP discovery: {error}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::HeaderValue;

    #[test]
    fn rejects_requests_without_the_session_token() {
        // Given an unauthenticated local caller, when authorization is checked, then access is denied.
        assert!(!authorized(&HeaderMap::new(), "test-token"));
    }

    #[test]
    fn rejects_browser_origins_even_with_a_token() {
        // Given a browser request with a token, when authorization is checked, then access is denied.
        let mut headers = HeaderMap::new();
        headers.insert(
            "authorization",
            HeaderValue::from_static("Bearer test-token"),
        );
        headers.insert("origin", HeaderValue::from_static("https://example.com"));
        assert!(!authorized(&headers, "test-token"));
    }

    #[test]
    fn accepts_the_current_session_token() {
        // Given an MCP caller with the current token, when authorization is checked, then access is granted.
        let mut headers = HeaderMap::new();
        headers.insert(
            "authorization",
            HeaderValue::from_static("Bearer test-token"),
        );
        assert!(authorized(&headers, "test-token"));
    }
}
