#![expect(
    clippy::unwrap_used,
    reason = "Tests fail immediately on fixture errors"
)]
use super::{cli, project::ProjectResources};
use crate::integrations::ToolId;
use serde_json::Value;
use std::{fs, path::Path};

fn project() -> tempfile::TempDir {
    let root = tempfile::tempdir().unwrap();
    fs::create_dir_all(root.path().join(".agents/skills/draw")).unwrap();
    fs::write(
        root.path().join(".agents/skills/draw/SKILL.md"),
        "---\nname: draw\ndescription: Draw canvas\n---\n",
    )
    .unwrap();
    fs::write(root.path().join(".agents/mcp.json"), r#"{"mcpServers":{
      "canvas":{"type":"stdio","command":"project-cli","args":["--mcp-stdio"],"env":{"CUSTOM":"value"},"enabled_tools":["read"],"disabled_tools":["delete"]},
      "off":{"command":"must-not-register","disabled":true},
      "also-off":{"command":"also-must-not-register","enabled":false}
    }}"#).unwrap();
    root
}

#[test]
fn canonical_project_resources_use_provider_flags_without_mutating_project() {
    // Given a shared project Skills/MCP configuration with explicit policies.
    let project = project();
    let original = fs::read(project.path().join(".agents/mcp.json")).unwrap();
    for tool in ToolId::ALL {
        let runtime = tempfile::tempdir().unwrap();
        let mut command = cli::command(tool, Path::new("/fake/tool"), project.path());
        // When adapting it for a native provider.
        ProjectResources::load(project.path())
            .unwrap()
            .apply(tool, &mut command, runtime.path())
            .unwrap();
        let args: Vec<_> = command
            .as_std()
            .get_args()
            .map(|s| s.to_string_lossy().into_owned())
            .collect();
        // Then the CLI retains its actual project cwd and uses canonical data, preserving policies.
        assert_eq!(command.as_std().get_current_dir(), Some(project.path()));
        match tool {
            ToolId::Codex => {
                let config = args
                    .iter()
                    .find(|s| s.starts_with("mcp_servers.\"canvas\"="))
                    .unwrap();
                let doc: toml_edit::Document = config.parse().unwrap();
                let server = &doc["mcp_servers"]["canvas"];
                assert_eq!(server["command"].as_str(), Some("project-cli"));
                assert_eq!(server["env"]["CUSTOM"].as_str(), Some("value"));
                assert_eq!(server["enabled_tools"][0].as_str(), Some("read"));
                assert_eq!(server["disabled_tools"][0].as_str(), Some("delete"));
                assert!(!args.iter().any(|s| s.contains("must-not-register")));
                assert!(args
                    .iter()
                    .any(|s| s == "mcp_servers.\"off\".enabled=false"));
            }
            ToolId::Claude | ToolId::Cursor => {
                assert!(args.iter().any(|s| s == "--plugin-dir"));
                let path = if tool == ToolId::Claude {
                    runtime.path().join("project-mcp.json")
                } else {
                    runtime.path().join("project-plugin/mcp.json")
                };
                let config: Value = serde_json::from_slice(&fs::read(path).unwrap()).unwrap();
                assert_eq!(config["mcpServers"]["canvas"]["env"]["CUSTOM"], "value");
                assert_eq!(
                    config["mcpServers"]["canvas"]["enabled_tools"],
                    serde_json::json!(["read"])
                );
                assert_eq!(
                    config["mcpServers"]["canvas"]["disabled_tools"],
                    serde_json::json!(["delete"])
                );
                assert!(config["mcpServers"].get("off").is_none());
                assert!(config["mcpServers"].get("also-off").is_none());
                assert_eq!(
                    fs::read_to_string(runtime.path().join("project-plugin/skills/draw/SKILL.md"))
                        .unwrap(),
                    fs::read_to_string(project.path().join(".agents/skills/draw/SKILL.md"))
                        .unwrap()
                );
            }
            ToolId::Pi => {
                assert!(args.iter().any(|s| s == "--skill"));
                let script = fs::read_to_string(runtime.path().join("project-mcp.mjs")).unwrap();
                let encoded = script
                    .strip_prefix("const servers = ")
                    .unwrap()
                    .split(";\n")
                    .next()
                    .unwrap();
                let servers: Value = serde_json::from_str(encoded).unwrap();
                assert_eq!(
                    servers["canvas"]["enabled_tools"],
                    serde_json::json!(["read"])
                );
                assert_eq!(servers["canvas"]["env"]["CUSTOM"], "value");
                assert!(servers.get("off").is_none());
                assert!(servers.get("also-off").is_none());
            }
        }
    }
    assert_eq!(
        fs::read(project.path().join(".agents/mcp.json")).unwrap(),
        original
    );
    assert!(!project.path().join(".claude").exists());
    assert!(!project.path().join(".cursor").exists());
}

#[test]
fn missing_canonical_resources_leave_native_global_discovery_intact() {
    // Given a registered project with no shared resource overrides.
    let root = tempfile::tempdir().unwrap();
    for tool in ToolId::ALL {
        let runtime = tempfile::tempdir().unwrap();
        let mut command = cli::command(tool, Path::new("/fake/tool"), root.path());
        let before: Vec<_> = command
            .as_std()
            .get_args()
            .map(std::ffi::OsStr::to_owned)
            .collect();
        // When loading project resources.
        ProjectResources::load(root.path())
            .unwrap()
            .apply(tool, &mut command, runtime.path())
            .unwrap();
        // Then argv stays unchanged, so the provider uses its own global configuration.
        assert_eq!(command.as_std().get_args().collect::<Vec<_>>(), before);
    }
}

#[test]
fn invalid_project_mcp_never_silently_falls_back_to_global() {
    // Given malformed canonical project configuration.
    let root = tempfile::tempdir().unwrap();
    fs::create_dir(root.path().join(".agents")).unwrap();
    fs::write(root.path().join(".agents/mcp.json"), b"{bad").unwrap();
    // When loading it for execution.
    let result = ProjectResources::load(root.path());
    // Then the request rejects rather than quietly using unrelated global tools.
    assert!(result.is_err());
}
