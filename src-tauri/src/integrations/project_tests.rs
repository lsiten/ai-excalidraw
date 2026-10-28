#![expect(
    clippy::unwrap_used,
    reason = "Isolated fixtures fail immediately on unexpected errors"
)]
use super::{tests::Fixture, *};
use serde_json::{json, Value};
use std::fs;

fn registered_project(f: &Fixture, name: &str) -> (String, PathBuf) {
    let directory = f.root.join(name);
    fs::create_dir(&directory).unwrap();
    let project = crate::workspace::dispatch(
        &f.root.join("registry"),
        crate::workspace::Request::OpenProject {
            directory: directory.to_str().unwrap().into(),
        },
    )
    .unwrap();
    (project["id"].as_str().unwrap().into(), directory)
}

#[test]
fn project_install_is_isolated_from_other_projects_and_global_configuration() {
    // Given two registered projects and distinct global native configurations.
    let f = Fixture::new();
    let (id, root) = registered_project(&f, "selected");
    let (_, other) = registered_project(&f, "other");
    for tool in ToolId::ALL {
        Fixture::put(&f.context.paths(tool).0, b"global-private-config");
    }
    let resolved = project::resolve(&f.root.join("registry"), Some(&id))
        .unwrap()
        .unwrap();
    // When installing one tool into the selected registered project.
    let result = install(&f.context, ToolId::Codex, Some(&resolved));
    // Then all tool statuses refer to the shared project files, leaving globals and peers alone.
    assert!(result.success, "{}", result.message);
    for tool in ToolId::ALL {
        let status = status(&f.context, tool, Some(&resolved));
        assert_eq!(
            status.config_path,
            root.join(".agents/mcp.json").to_str().unwrap()
        );
        assert_eq!(
            status.skill_path,
            root.join(".agents/skills")
                .join(SKILL_NAME)
                .to_str()
                .unwrap()
        );
        assert_eq!(status.mcp_status, Status::Current);
        assert_eq!(status.skill_status, Status::Current);
        assert_eq!(
            fs::read(f.context.paths(tool).0).unwrap(),
            b"global-private-config"
        );
    }
    assert!(!other.join(".agents").exists());
    assert_eq!(
        status(&f.context, ToolId::Codex, Some(&other)).mcp_status,
        Status::Missing
    );
}

#[test]
fn registered_resolution_rejects_unknown_ids_and_uses_global_only_when_absent() {
    // Given a registered project whose path must not substitute for its identifier.
    let f = Fixture::new();
    let (_, root) = registered_project(&f, "selected");
    let registry = f.root.join("registry");
    // When resolving an absent ID, an unknown ID and an arbitrary path.
    let absent = project::resolve(&registry, None).unwrap();
    let unknown = project::resolve(&registry, Some("unknown"));
    let raw_root = project::resolve(&registry, root.to_str());
    // Then only an absent ID selects the existing global paths.
    assert!(absent.is_none());
    assert!(unknown.is_err());
    assert!(raw_root.is_err());
    for tool in ToolId::ALL {
        assert_eq!(
            project::paths(&f.context, tool, absent.as_deref()),
            f.context.paths(tool)
        );
    }
    assert!(!root.join(".agents").exists());
}

#[test]
fn project_install_preserves_unrelated_servers_policies_and_skill_files_with_backups() {
    // Given project configuration with unrelated integrations and a customized skill.
    let f = Fixture::new();
    let (_, root) = registered_project(&f, "selected");
    let (path, skill) = project::paths(&f.context, ToolId::Codex, Some(&root));
    let original = json!({
        "mcpServers": {
            "other": {"command": "keep", "env": {"AUTH": "private"}},
            "ai-excalidraw": {"url": "https://old", "headers": {"secret": "old"},
                "disabled": true, "allowedTools": ["get_workspace"],
                "env": {"OTHER": "keep"}}
        },
        "preferences": {"custom": true}
    });
    let original_bytes = serde_json::to_vec(&original).unwrap();
    Fixture::put(&path, &original_bytes);
    Fixture::put(&skill.join("SKILL.md"), b"old skill");
    Fixture::put(&skill.join("custom.md"), b"custom");
    // When updating the project integration through a selected tool.
    let result = install(&f.context, ToolId::Claude, Some(&root));
    // Then unrelated values survive, the native connector is bound to the instance and backups are exact.
    assert!(result.success, "{}", result.message);
    let doc: Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
    assert_eq!(doc["mcpServers"]["other"], original["mcpServers"]["other"]);
    assert_eq!(doc["preferences"], original["preferences"]);
    let entry = &doc["mcpServers"][NAME];
    assert_eq!(entry["disabled"], true);
    assert_eq!(entry["allowedTools"], json!(["get_workspace"]));
    assert_eq!(entry["env"]["OTHER"], "keep");
    assert_eq!(
        entry["env"]["AI_EXCALIDRAW_BRIDGE_FILE"],
        json!(f.context.bridge)
    );
    assert_eq!(entry["command"], json!(f.context.executable));
    assert_eq!(entry["args"], json!(["--mcp-stdio"]));
    assert_eq!(entry["type"], "stdio");
    assert!(entry.get("url").is_none());
    assert!(entry.get("headers").is_none());
    assert_eq!(fs::read(skill.join("custom.md")).unwrap(), b"custom");
    assert!(result
        .backup_paths
        .iter()
        .any(|path| fs::read(path).unwrap() == original_bytes));
    assert!(result
        .backup_paths
        .iter()
        .any(|path| fs::read(path).unwrap() == b"old skill"));
    assert!(result
        .backup_paths
        .iter()
        .all(|path| Path::new(path).starts_with(&root)));
}

#[test]
fn shared_project_install_is_idempotent_across_selected_tools() {
    // Given a current installation made through Codex.
    let f = Fixture::new();
    let (_, root) = registered_project(&f, "selected");
    assert!(install(&f.context, ToolId::Codex, Some(&root)).success);
    let config = root.join(".agents/mcp.json");
    let before = fs::read(&config).unwrap();
    let modified = fs::metadata(&config).unwrap().modified().unwrap();
    // When installing the remaining selected clients against the same canonical configuration.
    let results: Vec<_> = ToolId::ALL
        .into_iter()
        .map(|tool| install(&f.context, tool, Some(&root)))
        .collect();
    // Then every tool reports success without rewriting or backing up identical files.
    for result in results {
        assert!(result.success, "{}", result.message);
        assert!(result.backup_paths.is_empty());
    }
    assert_eq!(fs::read(&config).unwrap(), before);
    assert_eq!(fs::metadata(&config).unwrap().modified().unwrap(), modified);
}

#[test]
fn malformed_project_configs_block_without_mutation_or_global_fallback() {
    // Given invalid JSON and invalid MCP structure variants, including an empty existing file.
    let f = Fixture::new();
    let (_, root) = registered_project(&f, "selected");
    let (config, skill) = project::paths(&f.context, ToolId::Codex, Some(&root));
    for bytes in [
        b"{private-secret:".as_slice(),
        b"",
        b"[]",
        br#"{"mcpServers":[]}"#,
        br#"{"mcpServers":{"ai-excalidraw":false}}"#,
        br#"{"mcpServers":{"ai-excalidraw":{"env":[]}}}"#,
    ] {
        Fixture::put(&config, bytes);
        // When reading status and attempting installation for malformed project config.
        let status = status(&f.context, ToolId::Codex, Some(&root));
        let result = install(&f.context, ToolId::Codex, Some(&root));
        // Then no target is changed and private config contents stay out of responses.
        assert_eq!(status.mcp_status, Status::Blocked);
        assert!(!result.success);
        assert!(result.backup_paths.is_empty());
        assert_eq!(fs::read(&config).unwrap(), bytes);
        assert!(!skill.exists());
        assert!(!serde_json::to_string(&status)
            .unwrap()
            .contains("private-secret"));
        assert!(!result.message.contains("private-secret"));
    }
    for tool in ToolId::ALL {
        assert!(!f.context.paths(tool).0.exists());
    }
}

#[cfg(unix)]
#[test]
fn project_install_rolls_back_config_and_skill_after_later_payload_failure() {
    use std::os::unix::fs::PermissionsExt;
    // Given an existing config and an unwritable payload subdirectory that passes read preflight.
    let f = Fixture::new();
    let (_, root) = registered_project(&f, "selected");
    let (config, skill) = project::paths(&f.context, ToolId::Pi, Some(&root));
    let original = br#"{"mcpServers":{"other":{"command":"keep"}}}"#;
    Fixture::put(&config, original);
    Fixture::put(&skill.join("SKILL.md"), b"old skill");
    let references = skill.join("references");
    fs::create_dir(&references).unwrap();
    fs::set_permissions(&references, fs::Permissions::from_mode(0o500)).unwrap();
    // When writing configuration and the first payload before the later payload write fails.
    let result = install(&f.context, ToolId::Pi, Some(&root));
    fs::set_permissions(&references, fs::Permissions::from_mode(0o700)).unwrap();
    // Then the install reports failure and restores all prior bytes.
    assert!(!result.success);
    assert_eq!(fs::read(&config).unwrap(), original);
    assert_eq!(fs::read(skill.join("SKILL.md")).unwrap(), b"old skill");
    assert!(!references.join("elements.md").exists());
    assert!(result
        .backup_paths
        .iter()
        .any(|path| fs::read(path).unwrap() == original));
    assert!(result
        .backup_paths
        .iter()
        .any(|path| fs::read(path).unwrap() == b"old skill"));
}

#[cfg(unix)]
#[test]
fn project_symlinks_are_blocked_before_any_installation_writes() {
    // Given symlinked agent directory, config file and payload paths in separate projects.
    let f = Fixture::new();
    let outside = f.root.join("outside");
    fs::create_dir(&outside).unwrap();
    let outside_config = outside.join("mcp.json");
    fs::write(&outside_config, b"{}").unwrap();
    for (index, relative) in [
        ".agents",
        ".agents/mcp.json",
        ".agents/skills/ai-excalidraw-desktop",
        ".agents/skills/ai-excalidraw-desktop/SKILL.md",
    ]
    .into_iter()
    .enumerate()
    {
        let (_, root) = registered_project(&f, &format!("project-{index}"));
        let target = root.join(relative);
        fs::create_dir_all(target.parent().unwrap()).unwrap();
        std::os::unix::fs::symlink(
            if Path::new(relative).extension().is_some() {
                &outside_config
            } else {
                &outside
            },
            target,
        )
        .unwrap();
        // When installing into an unsafe project target.
        let result = install(&f.context, ToolId::Cursor, Some(&root));
        // Then installation fails without following the link or modifying a config target.
        assert!(!result.success);
        assert!(result.backup_paths.is_empty());
        assert_eq!(fs::read(&outside_config).unwrap(), b"{}");
        assert_eq!(fs::read_dir(&outside).unwrap().count(), 1);
        if relative.starts_with(".agents/skills") {
            assert!(!root.join(".agents/mcp.json").exists());
        }
    }
}
