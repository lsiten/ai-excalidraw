#![expect(
    clippy::unwrap_used,
    reason = "Isolated test fixtures fail immediately on unexpected errors"
)]
use super::*;
use std::{fs, path::PathBuf};
use uuid::Uuid;

pub(super) struct Fixture {
    pub(super) root: PathBuf,
    pub(super) context: Context,
}
impl Fixture {
    pub(super) fn new() -> Self {
        let root = std::env::temp_dir()
            .canonicalize()
            .unwrap()
            .join(format!("integration-test-{}", Uuid::new_v4()));
        fs::create_dir(&root).unwrap();
        Self {
            context: Context {
                home: root.clone(),
                codex: root.join("codex-override"),
                executable: root.join("AI Excalidraw"),
                bridge: root.join("qa/bridge.json"),
                search: vec![root.join("bin")],
                applications: vec![root.join("Applications")],
            },
            root,
        }
    }
    pub(super) fn put(path: &std::path::Path, bytes: &[u8]) {
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, bytes).unwrap();
    }
    fn client(&self) {
        let path = self.root.join("bin/pi");
        Self::put(&path, b"#!/bin/sh\n");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(path, fs::Permissions::from_mode(0o700)).unwrap();
        }
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.root).unwrap();
    }
}

#[test]
fn installs_all_clients_with_instance_binding_and_idempotence() {
    // Given an isolated home with no client configuration.
    let f = Fixture::new();
    for id in ToolId::ALL {
        // When installing and then repeating the same installation.
        let first = install(&f.context, id, None);
        let second = install(&f.context, id, None);
        // Then all payloads match and the second operation needs no backups.
        assert!(first.success, "{}", first.message);
        assert!(second.success, "{}", second.message);
        assert!(second.backup_paths.is_empty());
        assert_eq!(status(&f.context, id, None).mcp_status, Status::Current);
        assert_eq!(status(&f.context, id, None).skill_status, Status::Current);
        let data = fs::read_to_string(f.context.paths(id).0).unwrap();
        assert!(data.contains("AI_EXCALIDRAW_BRIDGE_FILE"));
        assert!(data.contains(f.context.bridge.to_str().unwrap()));
        assert!(data.contains("--mcp-stdio"));
        if let Some(directory) = std::env::var_os("INTEGRATION_TEST_ARTIFACT_DIR") {
            let directory = PathBuf::from(directory);
            fs::create_dir_all(&directory).unwrap();
            fs::write(directory.join(format!("{id:?}-config.txt")), data).unwrap();
            fs::write(
                directory.join(format!("{id:?}-status.json")),
                serde_json::to_vec_pretty(&status(&f.context, id, None)).unwrap(),
            )
            .unwrap();
        }
    }
}

#[test]
fn preserves_toml_comments_policies_env_and_custom_skill_files() {
    // Given an old connector and local customization.
    let f = Fixture::new();
    let (path, skill) = f.context.paths(ToolId::Codex);
    let original = b"# keep comment\nmodel = 'custom'\n[mcp_servers.ai-excalidraw]\ncommand = 'node'\nargs = ['old.mjs']\nenabled = false\nstartup_timeout_sec = 90\n[mcp_servers.ai-excalidraw.env]\nOTHER = 'private'\n";
    Fixture::put(&path, original);
    Fixture::put(&skill.join("custom.txt"), b"custom");
    Fixture::put(&skill.join("SKILL.md"), b"old skill");
    // When replacing the named connector.
    let result = install(&f.context, ToolId::Codex, None);
    // Then unrelated preferences survive and overwritten files have exact backups.
    assert!(result.success, "{}", result.message);
    let text = fs::read_to_string(path).unwrap();
    let doc: toml_edit::Document = text.parse().unwrap();
    assert!(text.contains("# keep comment"));
    assert_eq!(doc["model"].as_str(), Some("custom"));
    assert_eq!(doc["mcp_servers"][NAME]["enabled"].as_bool(), Some(false));
    assert_eq!(
        doc["mcp_servers"][NAME]["env"]["OTHER"].as_str(),
        Some("private")
    );
    assert_eq!(fs::read(skill.join("custom.txt")).unwrap(), b"custom");
    assert!(result
        .backup_paths
        .iter()
        .any(|p| fs::read(p).unwrap() == original));
    assert!(result
        .backup_paths
        .iter()
        .any(|p| fs::read(p).unwrap() == b"old skill"));
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        for p in result.backup_paths {
            assert_eq!(fs::metadata(p).unwrap().permissions().mode() & 0o777, 0o600);
        }
    }
}

#[test]
fn updates_jsonc_without_losing_other_servers_or_policies() {
    // Given JSONC with a remote transport and preserved policies.
    let f = Fixture::new();
    let path = f.context.paths(ToolId::Cursor).0;
    Fixture::put(&path, br#"{ // comment
      "mcpServers": {"other":{"command":"keep"},"ai-excalidraw":{"url":"https://old","headers":{"secret":"x"},"disabled":true,"env":{"OTHER":"keep"},},},
      "preferences":{"x":1},
    }"#);
    // When installing the native transport.
    let result = install(&f.context, ToolId::Cursor, None);
    // Then all unrelated JSON values survive and remote transport fields disappear.
    assert!(result.success, "{}", result.message);
    let doc: serde_json::Value = serde_json::from_slice(&fs::read(path).unwrap()).unwrap();
    assert_eq!(doc["mcpServers"]["other"]["command"], "keep");
    assert_eq!(doc["mcpServers"][NAME]["disabled"], true);
    assert_eq!(doc["mcpServers"][NAME]["env"]["OTHER"], "keep");
    assert!(doc["mcpServers"][NAME].get("url").is_none());
    assert!(doc["mcpServers"][NAME].get("headers").is_none());
    assert_eq!(doc["preferences"]["x"], 1);
}

#[test]
fn malformed_config_blocks_without_exposing_or_changing_content() {
    // Given malformed private configuration.
    let f = Fixture::new();
    let path = f.context.paths(ToolId::Claude).0;
    Fixture::put(&path, b"{secret-credential:");
    // When inspecting and attempting installation.
    let status = status(&f.context, ToolId::Claude, None);
    let result = install(&f.context, ToolId::Claude, None);
    // Then the config is unchanged and neither response leaks its contents.
    assert_eq!(status.mcp_status, Status::Blocked);
    assert!(!result.success);
    assert_eq!(fs::read(path).unwrap(), b"{secret-credential:");
    assert!(!serde_json::to_string(&status)
        .unwrap()
        .contains("secret-credential"));
    assert!(!result.message.contains("secret-credential"));
}

#[test]
fn rollback_restores_prior_files_after_later_write_failure() {
    // Given a transaction that will encounter a directory where a file is needed.
    let f = Fixture::new();
    let original = f.root.join("original");
    Fixture::put(&original, b"before");
    let new = f.root.join("new");
    let blocked = f.root.join("blocked");
    fs::create_dir(&blocked).unwrap();
    let mut tx = files::Transaction::default();
    // When earlier writes succeeded and a later write fails.
    tx.write(&original, b"after").unwrap();
    tx.write(&new, b"new").unwrap();
    assert!(tx.write(&blocked, b"fail").is_err());
    tx.rollback().unwrap();
    // Then prior contents are restored and newly created files removed.
    assert_eq!(fs::read(original).unwrap(), b"before");
    assert!(!new.exists());
    assert_eq!(fs::read(&tx.backups()[0]).unwrap(), b"before");
}

#[cfg(unix)]
#[test]
fn symlinked_skill_targets_block_before_config_write() {
    // Given a skill directory redirected outside the target.
    let f = Fixture::new();
    let (config, skill) = f.context.paths(ToolId::Pi);
    fs::create_dir_all(skill.parent().unwrap()).unwrap();
    let outside = f.root.join("outside");
    fs::create_dir(&outside).unwrap();
    std::os::unix::fs::symlink(&outside, &skill).unwrap();
    // When installing.
    let result = install(&f.context, ToolId::Pi, None);
    // Then no config or external payload is written.
    assert!(!result.success);
    assert!(!config.exists());
    assert_eq!(fs::read_dir(outside).unwrap().count(), 0);
}

#[test]
fn prompt_waits_for_client_and_claims_only_once_without_config_writes() {
    // Given a home without an installed client.
    let f = Fixture::new();
    let flag = f.root.join("app/integration-prompt.json");
    assert!(!claim_prompt(&f.context, &flag).unwrap());
    assert!(!flag.exists());
    f.client();
    // When the first eligible prompt is claimed, then requested again.
    assert!(claim_prompt(&f.context, &flag).unwrap());
    assert!(!claim_prompt(&f.context, &flag).unwrap());
    // Then the durable claim exists but client configuration does not.
    assert_eq!(
        serde_json::from_slice::<serde_json::Value>(&fs::read(flag).unwrap()).unwrap()["claimed"],
        true
    );
    assert!(!f.context.paths(ToolId::Pi).0.exists());
}

#[test]
fn outdated_install_does_not_trigger_prompt() {
    // Given a detected tool with an installed skill and old connector.
    let f = Fixture::new();
    f.client();
    let (config, skill) = f.context.paths(ToolId::Pi);
    Fixture::put(
        &config,
        br#"{"mcpServers":{"ai-excalidraw":{"command":"old"}}}"#,
    );
    Fixture::put(&skill.join("SKILL.md"), b"old skill");
    // When considering the startup prompt.
    let flag = f.root.join("claim.json");
    let claimed = claim_prompt(&f.context, &flag).unwrap();
    // Then updates alone do not request a prompt.
    assert!(!claimed);
    assert!(!flag.exists());
}

#[test]
fn home_override_prefers_absolute_home_and_rejects_relative_paths() {
    // Given valid HOME and USERPROFILE overrides plus a relative HOME.
    let f = Fixture::new();
    let home = f.root.join("home");
    let profile = f.root.join("profile");
    // When resolving overrides without changing process environment.
    let primary = model::home_override(
        Some(home.clone().into_os_string()),
        Some(profile.clone().into_os_string()),
    );
    let fallback = model::home_override(
        Some("relative".into()),
        Some(profile.clone().into_os_string()),
    );
    let rejected = model::home_override(Some("relative".into()), None);
    // Then only absolute homes are selected and HOME takes precedence.
    assert_eq!(primary, Some(home));
    assert_eq!(fallback, Some(profile));
    assert_eq!(rejected, None);
}
