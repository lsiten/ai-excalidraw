#![expect(
    clippy::unwrap_used,
    reason = "Tests fail immediately on fixture errors"
)]
use super::*;

#[test]
fn codex_rejects_inherited_opposite_transport_instead_of_deep_merging() {
    // Given a global HTTP server and a same-name canonical stdio server.
    let global: toml_edit::Document = "[mcp_servers.canvas]\nurl='https://example.invalid/mcp'\n"
        .parse()
        .unwrap();
    let project: McpConfig =
        serde_json::from_str(r#"{"mcpServers":{"canvas":{"command":"local-tool"}}}"#).unwrap();
    // When checking transport compatibility before spawning.
    let result = check_codex(global.as_table(), &project);
    // Then the request rejects rather than retaining both url and command.
    assert!(result.is_err());
}

#[test]
fn codex_allows_same_transport_overrides_and_unrelated_global_fallback() {
    // Given a global stdio server with a same-name project override.
    let global: toml_edit::Document =
        "[mcp_servers.canvas]\ncommand='old'\n[mcp_servers.other]\nurl='https://example.invalid'\n"
            .parse()
            .unwrap();
    let project: McpConfig =
        serde_json::from_str(r#"{"mcpServers":{"canvas":{"command":"new"}}}"#).unwrap();
    // When checking compatibility.
    let result = check_codex(global.as_table(), &project);
    // Then explicit project values can override without affecting unrelated servers.
    assert!(result.is_ok());
}

#[test]
fn pi_rejects_native_normalized_name_collision_for_enabled_project_server() {
    // Given a file-configured native server that would defeat extension registration.
    let native: McpConfig =
        serde_json::from_str(r#"{"mcpServers":{"ai_excalidraw":{"command":"old"}}}"#).unwrap();
    let project: McpConfig =
        serde_json::from_str(r#"{"mcpServers":{"ai-excalidraw":{"command":"new"}}}"#).unwrap();
    // When validating project precedence before starting Pi.
    let result = check_pi(&native, &project);
    // Then the request fails clearly rather than silently using the native server.
    assert!(result.is_err());
}

#[test]
fn pi_rejects_enabled_native_connection_for_disabled_project_server() {
    // Given a disabled project server and an enabled native server with an equivalent name.
    let native: McpConfig =
        serde_json::from_str(r#"{"mcpServers":{"ai_excalidraw":{"command":"must-not-connect"}}}"#)
            .unwrap();
    let project: McpConfig =
        serde_json::from_str(r#"{"mcpServers":{"ai-excalidraw":{"enabled":false}}}"#).unwrap();
    // When checking connection policy before the native CLI can connect.
    let result = check_pi(&native, &project);
    // Then tool exclusion alone is not mistaken for a disabled connection.
    assert!(result.is_err());
}

#[test]
fn pi_keeps_unrelated_native_servers_as_fallback() {
    // Given unrelated native/project servers and already disabled matching entries.
    let native: McpConfig = serde_json::from_str(
        r#"{"mcpServers":{"other":{"command":"keep"},"off_server":{"enabled":false}}}"#,
    )
    .unwrap();
    let project: McpConfig = serde_json::from_str(
        r#"{"mcpServers":{"canvas":{"command":"new"},"off-server":{"enabled":false}}}"#,
    )
    .unwrap();
    // When validating native fallback.
    let result = check_pi(&native, &project);
    // Then unrelated global configuration is left available.
    assert!(result.is_ok());
}

#[test]
fn pi_excludes_hyphenated_server_tools_using_native_underscore_namespace() {
    // Given a disabled canonical server with a hyphenated name.
    let resources = ProjectResources {
        root: "/project".into(),
        skills: None,
        mcp: Some(
            serde_json::from_str(r#"{"mcpServers":{"ai-excalidraw":{"disabled":true}}}"#).unwrap(),
        ),
    };
    let mut command = Command::new("/fake/pi");
    // When generating the provider-specific exclusion flag.
    resources.permissions(ToolId::Pi, &mut command);
    // Then the pattern matches Pi's actual native tool namespace.
    assert_eq!(
        command.as_std().get_args().collect::<Vec<_>>(),
        ["--exclude-tools", "mcp__ai_excalidraw__*"]
    );
}

#[test]
fn pi_accepts_equivalent_migrated_native_and_project_servers() {
    // Given the existing global installer entry and its canonical project equivalent.
    let native: McpConfig = serde_json::from_str(r#"{"mcpServers":{"ai_excalidraw":{"command":"/app/canvas","args":["--mcp-stdio"],"env":{"AI_EXCALIDRAW_BRIDGE_FILE":"/bridge"}}}}"#).unwrap();
    let project: McpConfig = serde_json::from_str(r#"{"mcpServers":{"ai-excalidraw":{"type":"stdio","enabled":true,"disabled":false,"command":"/app/canvas","args":["--mcp-stdio"],"env":{"AI_EXCALIDRAW_BRIDGE_FILE":"/bridge"}}}}"#).unwrap();
    // When checking Pi's native precedence using equivalent effective configuration.
    let result = check_pi(&native, &project);
    // Then the common migration setup remains usable without changing native auth/config.
    assert!(result.is_ok());
}

#[test]
fn pi_rejects_same_command_when_project_environment_or_policy_differs() {
    // Given a same-name native server whose command matches the canonical project entry.
    let native: McpConfig = serde_json::from_str(r#"{"mcpServers":{"canvas":{"command":"/app/canvas","env":{"BRIDGE":"old"},"toolExposure":{"write":"hidden"}}}}"#).unwrap();
    for server in [
        r#"{"command":"/app/canvas","env":{"BRIDGE":"new"},"toolExposure":{"write":"hidden"}}"#,
        r#"{"command":"/app/canvas","env":{"BRIDGE":"old"},"toolExposure":{"write":"direct"}}"#,
    ] {
        let mut project = McpConfig::default();
        project
            .mcp_servers
            .insert("canvas".into(), serde_json::from_str(server).unwrap());
        // When the native entry would override different project environment or policy.
        let result = check_pi(&native, &project);
        // Then matching executables alone are insufficient to bypass the conflict check.
        assert!(result.is_err());
    }
}
