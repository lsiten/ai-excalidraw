use crate::bridge::{self, BridgeState, CanvasCommand};
use axum::{
    extract::State,
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    Json,
};
use serde::Deserialize;
use serde_json::{json, Value};
use std::sync::{Arc, OnceLock};

#[derive(Deserialize)]
struct Catalog {
    tools: Vec<Value>,
    instructions: String,
}
static CATALOG: OnceLock<Catalog> = OnceLock::new();
fn catalog() -> Result<&'static Catalog, serde_json::Error> {
    if let Some(value) = CATALOG.get() {
        return Ok(value);
    }
    let parsed = serde_json::from_str(include_str!("mcp-tools.json"))?;
    Ok(CATALOG.get_or_init(|| parsed))
}

#[derive(Deserialize)]
struct Request {
    jsonrpc: String,
    #[serde(default)]
    id: Option<Value>,
    method: String,
    #[serde(default)]
    params: Value,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ToolCall {
    name: String,
    #[serde(default = "empty_arguments")]
    arguments: Value,
}
fn empty_arguments() -> Value {
    json!({})
}
fn error(id: &Value, code: i32, message: &str) -> Value {
    json!({"jsonrpc":"2.0","id":id,"error":{"code":code,"message":message}})
}
fn response(id: &Value, result: &Value) -> Value {
    json!({"jsonrpc":"2.0","id":id,"result":result})
}
const PROTOCOLS: [&str; 4] = ["2024-11-05", "2025-03-26", "2025-06-18", "2025-11-25"];
fn metadata(request: &Request, catalog: &Catalog) -> Option<Value> {
    let id = request.id.clone().unwrap_or(Value::Null);
    Some(match request.method.as_str() {
        "initialize" => {
            let requested = request.params["protocolVersion"].as_str().unwrap_or("");
            let protocol = if PROTOCOLS.contains(&requested) {
                requested
            } else {
                "2025-11-25"
            };
            response(
                &id,
                &json!({"protocolVersion":protocol,"capabilities":{"tools":{}},
                "serverInfo":{"name":"ai-excalidraw-desktop","version":env!("CARGO_PKG_VERSION")},
                "instructions":catalog.instructions}),
            )
        }
        "ping" => response(&id, &json!({})),
        "tools/list" => response(&id, &json!({"tools":catalog.tools})),
        "tools/call" => return None,
        _ => error(&id, -32601, "Method not found"),
    })
}

pub async fn handle(
    State(state): State<Arc<BridgeState>>,
    headers: HeaderMap,
    body: String,
) -> Response {
    if !state.is_authorized(&headers) {
        return StatusCode::UNAUTHORIZED.into_response();
    }
    if let Some(protocol) = headers.get("mcp-protocol-version") {
        if !protocol.to_str().is_ok_and(|v| PROTOCOLS.contains(&v)) {
            return (
                StatusCode::BAD_REQUEST,
                Json(error(
                    &Value::Null,
                    -32600,
                    "Unsupported MCP protocol version",
                )),
            )
                .into_response();
        }
    }
    let raw: Value = match serde_json::from_str(&body) {
        Ok(raw) => raw,
        Err(_) => return Json(error(&Value::Null, -32700, "Parse error")).into_response(),
    };
    let id = raw.get("id").cloned().unwrap_or(Value::Null);
    let request: Request = match serde_json::from_value(raw) {
        Ok(request) => request,
        Err(_) => return Json(error(&id, -32600, "Invalid JSON-RPC request")).into_response(),
    };
    if request.jsonrpc != "2.0"
        || !matches!(request.id, None | Some(Value::String(_) | Value::Number(_)))
    {
        return Json(error(&Value::Null, -32600, "Invalid JSON-RPC request")).into_response();
    }
    if request.id.is_none() {
        return StatusCode::ACCEPTED.into_response();
    }
    let Ok(catalog) = catalog() else {
        return Json(error(&id, -32603, "MCP tool catalog is unavailable")).into_response();
    };
    if let Some(value) = metadata(&request, catalog) {
        return Json(value).into_response();
    }
    let call: ToolCall = match serde_json::from_value(request.params) {
        Ok(call) => call,
        Err(_) => {
            return Json(error(
                &id,
                -32602,
                "Expected tool name and object arguments",
            ))
            .into_response()
        }
    };
    if !call.arguments.is_object() || !catalog.tools.iter().any(|tool| tool["name"] == call.name) {
        return Json(error(&id, -32602, "Unknown tool or invalid arguments")).into_response();
    }
    let (status, value) = bridge::execute(
        &state,
        CanvasCommand {
            tool: call.name,
            arguments: call.arguments,
        },
    )
    .await;
    let text = if status.is_success() {
        value.to_string()
    } else {
        format!("{value}. After a timeout, save failure or conflict, inspect get_workspace and the target before retrying; never blindly repeat a mutation.")
    };
    Json(response(
        &id,
        &json!({"content":[{"type":"text","text":text}],"isError":!status.is_success()}),
    ))
    .into_response()
}

#[cfg(test)]
mod tests {
    #![expect(
        clippy::unwrap_used,
        reason = "Test setup and assertions fail on unexpected errors"
    )]
    use super::*;
    #[test]
    fn negotiates_supported_protocol_and_advertises_real_tools() {
        // Given a real catalog, when initialized by an older client, then retain its supported protocol.
        let request: Request = serde_json::from_value(json!({"jsonrpc":"2.0","id":7,"method":"initialize","params":{"protocolVersion":"2025-03-26"}})).unwrap();
        let result = metadata(&request, catalog().unwrap()).unwrap();
        assert_eq!(result["result"]["protocolVersion"], "2025-03-26");
        assert_eq!(result["id"], 7);
        assert!(result["result"]["capabilities"]["tools"].is_object());
    }
    #[test]
    fn returns_method_not_found_instead_of_claiming_success() {
        // Given an unsupported request, when routed, then the JSON-RPC response is an error.
        let request: Request = serde_json::from_value(
            json!({"jsonrpc":"2.0","id":"unknown","method":"resources/list"}),
        )
        .unwrap();
        assert_eq!(
            metadata(&request, catalog().unwrap()).unwrap()["error"]["code"],
            -32601
        );
    }
    #[test]
    fn tools_catalog_contains_canvas_targeting_schema() {
        // Given the exported definitions, when listed, then native clients receive explicit canvas targeting.
        let tool = catalog()
            .unwrap()
            .tools
            .iter()
            .find(|tool| tool["name"] == "add_elements")
            .unwrap();
        assert!(tool["inputSchema"]["required"]
            .as_array()
            .unwrap()
            .contains(&json!("canvasId")));
    }
}
