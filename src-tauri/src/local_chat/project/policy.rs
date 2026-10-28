use super::{enabled, McpConfig, ProjectResources};
use crate::{
    integrations::{Context, ToolId},
    local_chat::error::{io, ChatError},
};
use std::path::Path;
use tokio::process::Command;

impl ProjectResources {
    pub(in crate::local_chat) fn validate_globals(
        &self,
        context: &Context,
        tool: ToolId,
    ) -> Result<(), ChatError> {
        let Some(mcp) = &self.mcp else {
            return Ok(());
        };
        match tool {
            ToolId::Codex => {
                let mut paths = vec![context.codex.join("config.toml")];
                paths.extend(self.root.ancestors().map(|p| p.join(".codex/config.toml")));
                for path in paths {
                    if let Some(text) = optional_text(&path)? {
                        let doc: toml_edit::Document =
                            text.parse().map_err(|_| ChatError::ProjectConfig)?;
                        check_codex(doc.as_table(), mcp)?;
                    }
                }
            }
            ToolId::Cursor => {
                if mcp.mcp_servers.values().all(enabled) {
                    return Ok(());
                }
                let global = std::env::var_os("CURSOR_CONFIG_DIR")
                    .map_or_else(|| context.home.join(".cursor"), std::path::PathBuf::from);
                for path in [global.join("mcp.json"), self.root.join(".cursor/mcp.json")] {
                    if let Some(text) = optional_text(&path)? {
                        let native: McpConfig =
                            serde_json::from_str(&text).map_err(|_| ChatError::ProjectConfig)?;
                        for (id, config) in &mcp.mcp_servers {
                            if !enabled(config) && native.mcp_servers.get(id).is_some_and(enabled) {
                                return Err(ChatError::Cli(format!("项目已禁用 MCP 服务器 {id}，但 Cursor 原生配置仍启用了同名服务器。CLI 插件无法覆盖此禁用策略，请先在 Cursor 原生配置中禁用同名服务器。")));
                            }
                        }
                    }
                }
            }
            ToolId::Pi => {
                let global = std::env::var_os("PI_CODING_AGENT_DIR")
                    .map_or_else(|| context.home.join(".pi/agent"), std::path::PathBuf::from);
                for path in [global.join("mcp.json"), self.root.join(".pi/mcp.json")] {
                    if let Some(text) = optional_text(&path)? {
                        let native: McpConfig =
                            serde_json::from_str(&text).map_err(|_| ChatError::ProjectConfig)?;
                        check_pi(&native, mcp)?;
                    }
                }
            }
            ToolId::Claude => {}
        }
        Ok(())
    }

    pub(super) fn permissions(&self, tool: ToolId, command: &mut Command) {
        let Some(mcp) = &self.mcp else {
            return;
        };
        let denied: Vec<_> = mcp
            .mcp_servers
            .iter()
            .filter(|(_, value)| !enabled(value))
            .map(|(id, _)| {
                let id = match tool {
                    ToolId::Pi => pi_name(id),
                    ToolId::Codex | ToolId::Claude | ToolId::Cursor => id.clone(),
                };
                format!("mcp__{id}__*")
            })
            .collect();
        if !denied.is_empty() {
            match tool {
                ToolId::Claude => {
                    command.arg("--disallowedTools").arg(denied.join(","));
                }
                ToolId::Pi => {
                    command.arg("--exclude-tools").arg(denied.join(","));
                }
                ToolId::Codex | ToolId::Cursor => {}
            }
        }
        if tool == ToolId::Claude && mcp.mcp_servers.get("ai-excalidraw").is_some_and(enabled) {
            let tools = [
                "get_canvas",
                "add_elements",
                "update_elements",
                "delete_elements",
                "fit_canvas",
                "list_files",
                "get_workspace",
            ];
            command.arg("--allowedTools").arg(
                tools
                    .map(|name| format!("mcp__ai-excalidraw__{name}"))
                    .join(","),
            );
        }
    }
}

fn check_codex(table: &dyn toml_edit::TableLike, project: &McpConfig) -> Result<(), ChatError> {
    for (name, item) in table.iter() {
        if let Some(child) = item.as_table_like() {
            if name == "mcp_servers" {
                for (id, source) in child.iter() {
                    if let Some(target) = project.mcp_servers.get(id).filter(|v| enabled(v)) {
                        if let Some(source) = source.as_table_like() {
                            let keys: &[&str] = if target.get("command").is_some() {
                                &[
                                    "url",
                                    "http_headers",
                                    "bearer_token_env_var",
                                    "env_http_headers",
                                ]
                            } else {
                                &["command", "args", "env"]
                            };
                            if keys.iter().any(|key| source.contains_key(key)) {
                                return Err(ChatError::Cli(format!("项目 MCP 服务器 {id} 与 Codex 原生配置的同名服务器传输方式冲突。无法安全覆盖合并字段，请先统一两处配置或重命名服务器。")));
                            }
                        }
                    }
                }
            } else {
                check_codex(child, project)?;
            }
        }
    }
    Ok(())
}

fn optional_text(path: &Path) -> Result<Option<String>, ChatError> {
    match std::fs::metadata(path) {
        Ok(metadata) if metadata.len() > 1024 * 1024 => Err(ChatError::ProjectConfig),
        Ok(_) => std::fs::read_to_string(path)
            .map(Some)
            .map_err(io("读取原生 MCP 配置")),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(io("检查原生 MCP 配置")(error)),
    }
}

fn pi_name(name: &str) -> String {
    name.replace('-', "_")
}

fn check_pi(native: &McpConfig, project: &McpConfig) -> Result<(), ChatError> {
    for (id, target) in &project.mcp_servers {
        for (source_id, source) in &native.mcp_servers {
            let native_enabled =
                source.get("enabled").and_then(serde_json::Value::as_bool) != Some(false);
            if pi_name(id) == pi_name(source_id)
                && (enabled(target) || native_enabled)
                && (enabled(target) != native_enabled || pi_settings(source) != pi_settings(target))
            {
                return Err(ChatError::Cli(format!("项目 MCP 服务器 {id} 与 Pi 原生配置的服务器 {source_id} 配置不同（横线和下划线视为同名）。Pi 文件配置会优先于项目注册，无法安全覆盖；请统一两处配置，或移除、重命名原生同名配置。")));
            }
        }
    }
    Ok(())
}

fn pi_settings(value: &serde_json::Value) -> serde_json::Value {
    let mut settings = value.clone();
    if let Some(object) = settings.as_object_mut() {
        object.remove("enabled");
        object.remove("disabled");
        let inferred_type = match object.get("type").and_then(serde_json::Value::as_str) {
            Some("stdio") => object.contains_key("command"),
            Some("http" | "streamable-http") => object.contains_key("url"),
            _ => false,
        };
        if inferred_type {
            object.remove("type");
        }
        for key in ["args", "env", "headers"] {
            let default = match object.get(key) {
                Some(serde_json::Value::Array(values)) => key == "args" && values.is_empty(),
                Some(serde_json::Value::Object(values)) => key != "args" && values.is_empty(),
                _ => false,
            };
            if default {
                object.remove(key);
            }
        }
    }
    settings
}

#[cfg(test)]
#[path = "policy_tests.rs"]
mod tests;
