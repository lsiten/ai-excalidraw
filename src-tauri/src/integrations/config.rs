use super::{
    files,
    model::{Context, ToolId},
    NAME,
};
use serde_json::{json, Value};
use std::path::Path;
use toml_edit::{Document, Item, Table};

type Prepared = (Option<Vec<u8>>, Vec<u8>);
pub(super) fn prepare_scoped(
    context: &Context,
    id: ToolId,
    root: Option<&Path>,
) -> Result<Prepared, String> {
    let path = super::project::paths(context, id, root).0;
    files::safe_path(&path)?;
    let old = files::read_optional(&path)?;
    let text = std::str::from_utf8(old.as_deref().unwrap_or_default())
        .map_err(|_| "配置文件不是 UTF-8 编码")?;
    if old.is_some() {
        match (root, id) {
            (None, ToolId::Codex | ToolId::Cursor) => {}
            (Some(_), _) | (None, ToolId::Claude | ToolId::Pi) => {
                serde_json::from_str::<Value>(text.trim_start_matches('\u{feff}'))
                    .map_err(|_| "JSON 配置格式错误")?;
            }
        }
        if root.is_none() && id == ToolId::Cursor && text.trim().is_empty() {
            return Err("JSON 配置格式错误".into());
        }
    }
    let command = context
        .executable
        .to_str()
        .ok_or("程序路径不是 UTF-8 编码")?;
    let bridge = context
        .bridge
        .to_str()
        .ok_or("桥接文件路径不是 UTF-8 编码")?;
    let updated = match (root, id) {
        (None, ToolId::Codex) => update_toml(text, command, bridge)?,
        (Some(_), _) | (None, ToolId::Claude | ToolId::Cursor | ToolId::Pi) => {
            update_json(text, command, bridge)?
        }
    };
    Ok((old, updated.into_bytes()))
}

fn update_toml(text: &str, command: &str, bridge: &str) -> Result<String, String> {
    let mut doc: Document = text.parse().map_err(|_| "TOML 配置格式错误")?;
    let servers = doc
        .entry("mcp_servers")
        .or_insert(Item::Table(Table::new()))
        .as_table_like_mut()
        .ok_or("mcp_servers 必须为 TOML 表")?;
    if !servers.contains_key(NAME) {
        servers.insert(NAME, Item::Table(Table::new()));
    }
    let server = servers
        .get_mut(NAME)
        .and_then(Item::as_table_like_mut)
        .ok_or("AI Excalidraw MCP 配置必须为 TOML 表")?;
    let expected_args = toml_edit::Array::from_iter(["--mcp-stdio"]);
    let current = server.get("command").and_then(Item::as_str) == Some(command)
        && server
            .get("args")
            .and_then(Item::as_array)
            .is_some_and(|args| {
                args.len() == 1
                    && args.get(0).and_then(toml_edit::Value::as_str) == Some("--mcp-stdio")
            })
        && server
            .get("env")
            .and_then(Item::as_table_like)
            .and_then(|env| env.get("AI_EXCALIDRAW_BRIDGE_FILE"))
            .and_then(Item::as_str)
            == Some(bridge)
        && ![
            "url",
            "headers",
            "http_headers",
            "bearer_token_env_var",
            "env_http_headers",
            "type",
        ]
        .iter()
        .any(|key| server.contains_key(key));
    if current {
        return Ok(text.into());
    }
    server.insert("command", toml_edit::value(command));
    server.insert("args", toml_edit::value(expected_args));
    for key in [
        "url",
        "headers",
        "http_headers",
        "bearer_token_env_var",
        "env_http_headers",
        "type",
    ] {
        server.remove(key);
    }
    if !server.contains_key("env") {
        server.insert("env", Item::Table(Table::new()));
    }
    let env = server
        .get_mut("env")
        .and_then(Item::as_table_like_mut)
        .ok_or("MCP env 必须为 TOML 表")?;
    env.insert("AI_EXCALIDRAW_BRIDGE_FILE", toml_edit::value(bridge));
    Ok(doc.to_string())
}

fn update_json(text: &str, command: &str, bridge: &str) -> Result<String, String> {
    let mut doc: Value = if text.is_empty() {
        json!({})
    } else {
        serde_json::from_str(&jsonc(text)?).map_err(|_| "JSON 配置格式错误")?
    };
    let original = doc.clone();
    let root = doc.as_object_mut().ok_or("配置必须为 JSON 对象")?;
    let servers = root
        .entry("mcpServers")
        .or_insert_with(|| json!({}))
        .as_object_mut()
        .ok_or("mcpServers 必须为 JSON 对象")?;
    let server = servers
        .entry(NAME)
        .or_insert_with(|| json!({}))
        .as_object_mut()
        .ok_or("AI Excalidraw MCP 配置必须为 JSON 对象")?;
    server.insert("command".into(), json!(command));
    server.insert("args".into(), json!(["--mcp-stdio"]));
    server.insert("type".into(), json!("stdio"));
    for key in [
        "url",
        "headers",
        "http_headers",
        "oauth",
        "auth",
        "bearerToken",
        "bearer_token_env_var",
    ] {
        server.remove(key);
    }
    let env = server
        .entry("env")
        .or_insert_with(|| json!({}))
        .as_object_mut()
        .ok_or("MCP env 必须为 JSON 对象")?;
    env.insert("AI_EXCALIDRAW_BRIDGE_FILE".into(), json!(bridge));
    if doc == original {
        return Ok(text.into());
    }
    serde_json::to_string_pretty(&doc)
        .map(|s| s + "\n")
        .map_err(|e| e.to_string())
}

// Strip JSONC trivia outside strings; serde_json still validates all values and structure.
fn jsonc(text: &str) -> Result<String, String> {
    let mut bytes = text.trim_start_matches('\u{feff}').as_bytes().to_vec();
    let mut index = 0;
    let mut quoted = false;
    while index < bytes.len() {
        match bytes[index] {
            b'\\' if quoted => {
                index += 2;
                continue;
            }
            b'"' => quoted = !quoted,
            b'/' if !quoted && bytes.get(index + 1) == Some(&b'/') => {
                while index < bytes.len() && bytes[index] != b'\n' {
                    bytes[index] = b' ';
                    index += 1;
                }
                continue;
            }
            b'/' if !quoted && bytes.get(index + 1) == Some(&b'*') => {
                bytes[index] = b' ';
                bytes[index + 1] = b' ';
                index += 2;
                while index + 1 < bytes.len() && !(bytes[index] == b'*' && bytes[index + 1] == b'/')
                {
                    bytes[index] = b' ';
                    index += 1;
                }
                if index + 1 >= bytes.len() {
                    return Err("JSONC 注释未闭合".into());
                }
                bytes[index] = b' ';
                bytes[index + 1] = b' ';
                index += 2;
                continue;
            }
            _ => {}
        }
        index += 1;
    }
    index = 0;
    quoted = false;
    while index < bytes.len() {
        match bytes[index] {
            b'\\' if quoted => {
                index += 2;
                continue;
            }
            b'"' => quoted = !quoted,
            b',' if !quoted => {
                let mut next = index + 1;
                while bytes.get(next).is_some_and(u8::is_ascii_whitespace) {
                    next += 1;
                }
                if matches!(bytes.get(next), Some(b'}' | b']')) {
                    bytes[index] = b' ';
                }
            }
            _ => {}
        }
        index += 1;
    }
    String::from_utf8(bytes).map_err(|_| "JSONC 编码无效".into())
}

pub(super) fn has_entry_scoped(
    context: &Context,
    id: ToolId,
    root: Option<&Path>,
) -> Result<bool, String> {
    let path = super::project::paths(context, id, root).0;
    files::safe_path(&path)?;
    let Some(bytes) = files::read_optional(&path)? else {
        return Ok(false);
    };
    let text = std::str::from_utf8(&bytes).map_err(|_| "配置文件不是 UTF-8 编码")?;
    match id {
        ToolId::Codex if root.is_none() => {
            let doc: Document = text.parse().map_err(|_| "TOML 配置格式错误")?;
            Ok(doc
                .get("mcp_servers")
                .and_then(Item::as_table_like)
                .is_some_and(|table| table.contains_key(NAME)))
        }
        ToolId::Codex | ToolId::Claude | ToolId::Cursor | ToolId::Pi => {
            let doc: Value =
                serde_json::from_str(&jsonc(text)?).map_err(|_| "JSON 配置格式错误")?;
            Ok(doc
                .get("mcpServers")
                .and_then(Value::as_object)
                .is_some_and(|servers| servers.contains_key(NAME)))
        }
    }
}

pub(super) fn native_toml(context: &Context) -> Result<String, String> {
    update_toml(
        "",
        context
            .executable
            .to_str()
            .ok_or("程序路径不是 UTF-8 编码")?,
        context
            .bridge
            .to_str()
            .ok_or("桥接文件路径不是 UTF-8 编码")?,
    )
}
