mod policy;

use super::error::{io, ChatError};
use crate::integrations::ToolId;
use serde::Deserialize;
use serde_json::Value;
use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
};
use tokio::process::Command;

#[derive(Default, Deserialize)]
#[serde(rename_all = "camelCase")]
struct McpConfig {
    mcp_servers: BTreeMap<String, Value>,
}

pub(super) struct ProjectResources {
    root: PathBuf,
    skills: Option<PathBuf>,
    mcp: Option<McpConfig>,
}

impl ProjectResources {
    pub(super) fn load(root: &Path) -> Result<Self, ChatError> {
        let agents = root.join(".agents");
        let skills = agents.join("skills");
        let config = agents.join("mcp.json");
        let mcp = match std::fs::read(&config) {
            Ok(bytes) => {
                if bytes.len() > 1024 * 1024 {
                    return Err(ChatError::ProjectConfig);
                }
                let config: McpConfig =
                    serde_json::from_slice(&bytes).map_err(|_| ChatError::ProjectConfig)?;
                if config.mcp_servers.values().any(|s| !s.is_object()) {
                    return Err(ChatError::ProjectConfig);
                }
                Some(config)
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => None,
            Err(error) => return Err(io("读取项目 .agents/mcp.json")(error)),
        };
        Ok(Self {
            root: root.to_owned(),
            skills: skills.is_dir().then_some(skills),
            mcp,
        })
    }

    pub(super) fn apply(
        &self,
        tool: ToolId,
        command: &mut Command,
        runtime: &Path,
    ) -> Result<(), ChatError> {
        self.permissions(tool, command);
        match tool {
            // Codex discovers .agents/skills natively from the real project cwd.
            ToolId::Codex => self.codex(command)?,
            ToolId::Claude => {
                if let Some(mcp) = &self.mcp {
                    let path = runtime.join("project-mcp.json");
                    std::fs::write(&path, Self::enabled_config(mcp).to_string())
                        .map_err(io("准备 Claude 项目 MCP"))?;
                    command.arg("--mcp-config").arg(path);
                }
                self.plugin(command, runtime, ".claude-plugin")?;
            }
            ToolId::Cursor => self.plugin(command, runtime, ".cursor-plugin")?,
            ToolId::Pi => {
                if let Some(skills) = &self.skills {
                    command.arg("--skill").arg(skills);
                }
                if let Some(mcp) = &self.mcp {
                    // Same native registerMcpServer adapter as Multica pi.go (Pi >=0.99).
                    let encoded = Self::enabled_config(mcp)["mcpServers"].to_string();
                    let script = format!("const servers = {encoded};\nexport default function(pi) {{\nif (typeof pi.registerMcpServer !== 'function') throw new Error('Project MCP requires Pi >=0.99');\nfor (const [name, config] of Object.entries(servers)) pi.registerMcpServer(name, config);\n}}\n");
                    let path = runtime.join("project-mcp.mjs");
                    std::fs::write(&path, script).map_err(io("准备 Pi 项目 MCP 适配器"))?;
                    command.arg("--extension").arg(path);
                }
            }
        }
        Ok(())
    }

    fn codex(&self, command: &mut Command) -> Result<(), ChatError> {
        if let Some(mcp) = &self.mcp {
            for (name, value) in &mcp.mcp_servers {
                let mut table = value.as_object().cloned().ok_or(ChatError::ProjectConfig)?;
                if !enabled(value) {
                    let key = toml_edit::Value::from(name.as_str());
                    command.args(["-c", &format!("mcp_servers.{key}.enabled=false")]);
                    continue;
                }
                table.remove("disabled");
                table.insert("enabled".into(), Value::Bool(true));
                table.remove("type");
                if let Some(headers) = table.remove("headers") {
                    table.insert("http_headers".into(), headers);
                }
                let value = toml_value(&Value::Object(table))?;
                let key = toml_edit::Value::from(name.as_str());
                command.args(["-c", &format!("mcp_servers.{key}={value}")]);
            }
        }
        Ok(())
    }

    fn plugin(
        &self,
        command: &mut Command,
        runtime: &Path,
        manifest_dir: &str,
    ) -> Result<(), ChatError> {
        if self.skills.is_none() && self.mcp.is_none() {
            return Ok(());
        }
        let plugin = runtime.join("project-plugin");
        std::fs::create_dir_all(plugin.join(manifest_dir)).map_err(io("准备项目插件目录"))?;
        let manifest = serde_json::json!({"name":"ai-excalidraw-project","version":"1.0.0"});
        std::fs::write(
            plugin.join(manifest_dir).join("plugin.json"),
            manifest.to_string(),
        )
        .map_err(io("准备项目插件清单"))?;
        if let Some(skills) = &self.skills {
            #[cfg(unix)]
            std::os::unix::fs::symlink(skills, plugin.join("skills"))
                .map_err(io("关联项目 Skills"))?;
            #[cfg(windows)]
            std::os::windows::fs::symlink_dir(skills, plugin.join("skills"))
                .map_err(io("关联项目 Skills（需要系统允许创建符号链接）"))?;
        }
        if manifest_dir == ".cursor-plugin" {
            if let Some(mcp) = &self.mcp {
                std::fs::write(
                    plugin.join("mcp.json"),
                    Self::enabled_config(mcp).to_string(),
                )
                .map_err(io("准备 Cursor 项目 MCP"))?;
            }
        }
        command.arg("--plugin-dir").arg(plugin);
        Ok(())
    }

    fn enabled_config(mcp: &McpConfig) -> Value {
        let servers: BTreeMap<_, _> = mcp
            .mcp_servers
            .iter()
            .filter(|(_, v)| enabled(v))
            .map(|(k, v)| {
                let mut value = v.clone();
                if let Some(object) = value.as_object_mut() {
                    object.remove("enabled");
                    object.remove("disabled");
                }
                (k, value)
            })
            .collect();
        serde_json::json!({"mcpServers": servers})
    }
}

fn enabled(value: &Value) -> bool {
    value.get("disabled").and_then(Value::as_bool) != Some(true)
        && value.get("enabled").and_then(Value::as_bool) != Some(false)
}

fn toml_value(value: &Value) -> Result<toml_edit::Value, ChatError> {
    match value {
        Value::String(text) => Ok(toml_edit::Value::from(text.as_str())),
        Value::Bool(value) => Ok(toml_edit::Value::from(*value)),
        Value::Number(number) => number
            .as_i64()
            .map(toml_edit::Value::from)
            .or_else(|| number.as_f64().map(toml_edit::Value::from))
            .ok_or(ChatError::ProjectConfig),
        Value::Array(values) => {
            let mut array = toml_edit::Array::new();
            for value in values {
                array.push(toml_value(value)?);
            }
            Ok(toml_edit::Value::Array(array))
        }
        Value::Object(values) => {
            let mut table = toml_edit::InlineTable::new();
            for (key, value) in values {
                table.insert(key, toml_value(value)?);
            }
            Ok(toml_edit::Value::InlineTable(table))
        }
        Value::Null => Err(ChatError::ProjectConfig),
    }
}
