use axum::{
    extract::{Query, State},
    http::{HeaderMap, StatusCode},
    response::Html,
    routing::{get, post},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{
    collections::HashMap,
    fmt::Write as _,
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
    browser_token: String,
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

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct BrowserQuery {
    token: String,
    project_id: String,
    path: String,
}

pub fn authorized(headers: &HeaderMap, token: &str) -> bool {
    !headers.contains_key("origin")
        && headers.get("authorization").and_then(|h| h.to_str().ok())
            == Some(format!("Bearer {token}").as_str())
}

fn query_escape(value: &str) -> String {
    let mut output = String::with_capacity(value.len());
    for byte in value.bytes() {
        if matches!(byte, b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'~') {
            output.push(byte as char);
        } else {
            let _ = write!(output, "%{byte:02X}");
        }
    }
    output
}

pub fn browser_url(state: &BridgeState, project_id: &str, path: &str) -> String {
    format!(
        "http://127.0.0.1:{}/browser/canvas?token={}&projectId={}&path={}",
        state.port,
        query_escape(&state.browser_token),
        query_escape(project_id),
        query_escape(path),
    )
}

fn script_json(value: &str) -> String {
    match serde_json::to_string(value) {
        Ok(json) => json
            .replace('<', "\\u003c")
            .replace('>', "\\u003e")
            .replace('&', "\\u0026"),
        Err(_) => "\"\"".into(),
    }
}

fn browser_document(title: &str, content: &str) -> String {
    let title = html_escape(title);
    let content = script_json(content);
    format!(
        r#"<!doctype html>
<html lang="zh-CN">
<head>
<meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1">
<title>{title} · AI Excalidraw</title>
<style>
:root {{ color-scheme: light dark; font-family: -apple-system,BlinkMacSystemFont,"Segoe UI",sans-serif; background:#f7f7f5; color:#252525; }}
@media (prefers-color-scheme:dark) {{ :root {{ background:#1f1f1f; color:#f2f2f2; }} .bar {{ background:#2b2b2b; border-color:#444; }} .stage {{ background:#171717; }} }}
* {{ box-sizing:border-box; }} body {{ margin:0; height:100vh; display:flex; flex-direction:column; }}
.bar {{ min-height:48px; display:flex; align-items:center; gap:12px; padding:0 16px; background:#fff; border-bottom:1px solid #dededb; }}
.title {{ flex:1; min-width:0; font-size:14px; font-weight:600; overflow:hidden; text-overflow:ellipsis; white-space:nowrap; }}
.meta {{ font-size:12px; opacity:.68; }} .button {{ min-height:32px; border:1px solid #cfcfca; border-radius:6px; background:transparent; color:inherit; padding:0 10px; cursor:pointer; }}
.button:focus-visible {{ outline:2px solid #5b8def; outline-offset:2px; }} .stage {{ flex:1; min-height:0; overflow:auto; background:#ecece8; }}
svg {{ width:100%; height:100%; min-width:640px; min-height:420px; display:block; }} .empty {{ height:100%; display:grid; place-items:center; opacity:.65; font-size:14px; }}
</style>
</head>
<body>
<header class="bar"><div class="title" id="title"></div><div class="meta">浏览器画布预览 · 只读</div><button class="button" id="reload">刷新</button></header>
<main class="stage" id="stage"><div class="empty">正在加载画布…</div></main>
<script>
const title = {title_json};
const raw = {content};
document.getElementById('title').textContent = title;
document.getElementById('reload').onclick = () => location.reload();
const scene = JSON.parse(raw);
const elements = Array.isArray(scene.elements) ? scene.elements.filter((element) => !element.isDeleted) : [];
const visible = elements.filter((element) => Number.isFinite(element.x) && Number.isFinite(element.y));
const bounds = visible.reduce((box, element) => {{
  const width = Number(element.width) || 0, height = Number(element.height) || 0;
  box.minX = Math.min(box.minX, element.x); box.minY = Math.min(box.minY, element.y);
  box.maxX = Math.max(box.maxX, element.x + width); box.maxY = Math.max(box.maxY, element.y + height);
  return box;
}}, {{ minX:Infinity, minY:Infinity, maxX:-Infinity, maxY:-Infinity }});
if (!visible.length || !Number.isFinite(bounds.minX)) {{
  document.getElementById('stage').innerHTML = '<div class="empty">这张画布还是空的</div>';
}} else {{
  const padding = 80, width = Math.max(640, bounds.maxX - bounds.minX + padding * 2), height = Math.max(420, bounds.maxY - bounds.minY + padding * 2);
  const svg = document.createElementNS('http://www.w3.org/2000/svg', 'svg');
  svg.setAttribute('viewBox', `${{bounds.minX-padding}} ${{bounds.minY-padding}} ${{width}} ${{height}}`);
  svg.setAttribute('role', 'img'); svg.setAttribute('aria-label', title);
  const defs = document.createElementNS(svg.namespaceURI, 'defs');
  const marker = document.createElementNS(svg.namespaceURI, 'marker'); marker.id = 'arrow'; marker.setAttribute('markerWidth','8'); marker.setAttribute('markerHeight','8'); marker.setAttribute('refX','7'); marker.setAttribute('refY','4'); marker.setAttribute('orient','auto'); marker.setAttribute('markerUnits','strokeWidth');
  const head = document.createElementNS(svg.namespaceURI, 'path'); head.setAttribute('d','M0,0 L8,4 L0,8 z'); head.setAttribute('fill','context-stroke'); marker.appendChild(head); defs.appendChild(marker); svg.appendChild(defs);
  const shape = (tag, attrs) => {{ const node = document.createElementNS(svg.namespaceURI, tag); for (const [key,value] of Object.entries(attrs)) node.setAttribute(key, String(value)); svg.appendChild(node); return node; }};
  const stroke = (element) => element.strokeColor || '#1e1e1e'; const fill = (element) => element.backgroundColor && element.backgroundColor !== 'transparent' ? element.backgroundColor : 'none';
  for (const element of elements) {{
    const common = {{ stroke:stroke(element), 'stroke-width':element.strokeWidth || 1.5, opacity:(element.opacity ?? 100) / 100, fill:fill(element) }};
    if (element.type === 'rectangle') shape('rect', {{ ...common, x:element.x, y:element.y, width:element.width, height:element.height, rx:element.roundness ? 8 : 0 }});
    else if (element.type === 'ellipse') shape('ellipse', {{ ...common, cx:element.x + element.width/2, cy:element.y + element.height/2, rx:element.width/2, ry:element.height/2 }});
    else if (element.type === 'diamond') shape('polygon', {{ ...common, points:`${{element.x + element.width/2}},${{element.y}} ${{element.x + element.width}},${{element.y + element.height/2}} ${{element.x + element.width/2}},${{element.y + element.height}} ${{element.x}},${{element.y + element.height/2}}` }});
    else if (element.type === 'text') {{ const node = shape('text', {{ ...common, fill:stroke(element), stroke:'none', x:element.x, y:element.y + (element.fontSize || 20), 'font-size':element.fontSize || 20, 'font-family':'-apple-system,BlinkMacSystemFont,"Segoe UI",sans-serif', 'white-space':'pre' }}); node.textContent = element.text || ''; }}
    else if (element.type === 'line' || element.type === 'arrow') {{ const points = Array.isArray(element.points) ? element.points : [[0,0],[element.width || 1, element.height || 1]]; const pointsText = points.map((point) => `${{element.x + point[0]}},${{element.y + point[1]}}`).join(' '); shape('polyline', {{ ...common, fill:'none', points:pointsText, 'marker-end':element.type === 'arrow' ? 'url(#arrow)' : '' }}); }}
    else if (element.type === 'image') {{ shape('rect', {{ ...common, fill:'none', 'stroke-dasharray':'5 5', x:element.x, y:element.y, width:element.width, height:element.height }}); const node = shape('text', {{ x:element.x+8, y:element.y+24, fill:stroke(element), 'font-size':14, stroke:'none' }}); node.textContent = '图片'; }}
  }}
  document.getElementById('stage').replaceChildren(svg);
}}
</script>
</body></html>"#,
        title_json = script_json(&title),
        content = content,
    )
}

fn html_escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

async fn browser_canvas(
    State(state): State<Arc<BridgeState>>,
    Query(query): Query<BrowserQuery>,
) -> Result<Html<String>, StatusCode> {
    if query.token != state.browser_token {
        return Err(StatusCode::UNAUTHORIZED);
    }
    let data = state
        .app
        .path()
        .app_data_dir()
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let value = crate::workspace::dispatch(
        &data,
        crate::workspace::Request::ReadCanvasFile {
            project_id: query.project_id,
            path: query.path,
        },
    )
    .map_err(|_| StatusCode::NOT_FOUND)?;
    let content = value
        .get("content")
        .and_then(Value::as_str)
        .ok_or(StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(Html(browser_document("AI Excalidraw 画布", content)))
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
        browser_token: Uuid::new_v4().to_string(),
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
                    .route("/browser/canvas", get(browser_canvas))
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

    #[test]
    fn escapes_browser_url_query_values() {
        assert_eq!(
            query_escape("slides/中文 1.excalidraw"),
            "slides%2F%E4%B8%AD%E6%96%87%201.excalidraw"
        );
    }

    #[test]
    fn browser_document_does_not_allow_scene_text_to_escape_script() {
        let html = browser_document("title", r"</script><script>alert('x')</script>");
        assert!(!html.contains("</script><script>alert"));
        assert!(html.contains("\\u003c/script\\u003e"));
    }
}
