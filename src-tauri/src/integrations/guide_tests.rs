#![expect(
    clippy::unwrap_used,
    reason = "Isolated test fixtures fail immediately on unexpected errors"
)]
use super::*;
use std::{fs, path::PathBuf};

struct Fixture {
    root: PathBuf,
    context: Context,
}
impl Fixture {
    fn new() -> Self {
        let root = std::env::temp_dir()
            .canonicalize()
            .unwrap()
            .join(format!("guide-{}", uuid::Uuid::new_v4()));
        let data = root.join("QA \"空间\" \\ instance");
        fs::create_dir_all(&data).unwrap();
        let context = Context {
            home: root.join("home"),
            codex: root.join("home/.codex"),
            executable: root.join("AI \"Excalidraw\".app/Contents/MacOS/app"),
            bridge: data.join("bridge.json"),
            search: vec![],
            applications: vec![],
        };
        fs::write(&context.bridge, br#"{"token":"do-not-export-this-secret"}"#).unwrap();
        Self { root, context }
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.root).unwrap();
    }
}

#[test]
fn exports_seven_files_and_quoted_instance_config_without_reading_client_or_bridge_secrets() {
    // Given unusual absolute paths, a secret-bearing bridge and a malformed client config.
    let fixture = Fixture::new();
    fs::create_dir_all(&fixture.context.codex).unwrap();
    let client = fixture.context.codex.join("config.toml");
    fs::write(&client, b"malformed-client-secret[").unwrap();
    // When generating a guide.
    let guide = export(&fixture.context, None).unwrap();
    // Then exported bytes match and machine-readable examples round-trip actual paths.
    let blocks: Vec<&str> = guide
        .text
        .split("```json\n")
        .skip(1)
        .map(|s| s.split("```").next().unwrap())
        .collect();
    let metadata: serde_json::Value = serde_json::from_str(blocks[0]).unwrap();
    let source = PathBuf::from(metadata["skillSourcePath"].as_str().unwrap());
    assert_eq!(
        source,
        fixture
            .context
            .bridge
            .parent()
            .unwrap()
            .join("integrations")
            .join(SKILL_NAME)
    );
    for (relative, bytes) in PAYLOAD {
        assert_eq!(fs::read(source.join(relative)).unwrap(), *bytes);
    }
    let json: serde_json::Value = serde_json::from_str(blocks[1]).unwrap();
    let entry = &json["mcpServers"]["ai-excalidraw"];
    assert_eq!(
        entry["command"],
        fixture.context.executable.to_str().unwrap()
    );
    assert_eq!(entry["args"], json!(["--mcp-stdio"]));
    assert_eq!(
        entry["env"],
        json!({"AI_EXCALIDRAW_BRIDGE_FILE": fixture.context.bridge})
    );
    assert_eq!(entry["type"], "stdio");
    let toml = guide
        .text
        .split("```toml\n")
        .nth(1)
        .unwrap()
        .split("```")
        .next()
        .unwrap();
    let parsed: toml_edit::Document = toml.parse().unwrap();
    assert_eq!(
        parsed["mcp_servers"]["ai-excalidraw"]["command"].as_str(),
        fixture.context.executable.to_str()
    );
    assert_eq!(
        parsed["mcp_servers"]["ai-excalidraw"]["args"][0].as_str(),
        Some("--mcp-stdio")
    );
    assert_eq!(fs::read(client).unwrap(), b"malformed-client-secret[");
    assert!(!guide.text.contains("do-not-export-this-secret"));
    assert!(!guide.text.contains("malformed-client-secret"));
    if let Some(directory) = std::env::var_os("INTEGRATION_TEST_ARTIFACT_DIR") {
        let directory = PathBuf::from(directory);
        fs::create_dir_all(&directory).unwrap();
        fs::write(directory.join("guide.txt"), &guide.text).unwrap();
        fs::write(
            directory.join("guide.json"),
            serde_json::to_vec_pretty(&guide).unwrap(),
        )
        .unwrap();
        for (relative, _) in PAYLOAD {
            let target = directory.join("guide-payload").join(relative);
            fs::create_dir_all(target.parent().unwrap()).unwrap();
            fs::copy(source.join(relative), target).unwrap();
        }
    }
}

#[cfg(unix)]
#[test]
fn guide_export_blocks_symlink_without_touching_external_files() {
    // Given an export path redirected to an external directory.
    let fixture = Fixture::new();
    let parent = fixture
        .context
        .bridge
        .parent()
        .unwrap()
        .join("integrations");
    let outside = fixture.root.join("outside");
    fs::create_dir(&parent).unwrap();
    fs::create_dir(&outside).unwrap();
    std::os::unix::fs::symlink(&outside, parent.join(SKILL_NAME)).unwrap();
    // When exporting the payload.
    let result = export(&fixture.context, None);
    // Then the export fails without writing the external directory.
    assert!(result.is_err());
    assert_eq!(fs::read_dir(outside).unwrap().count(), 0);
}

#[test]
fn project_guide_exports_to_project_and_identifies_canonical_paths_without_reading_configs() {
    // Given a project with private malformed configuration and a secret-bearing bridge.
    let fixture = Fixture::new();
    let root = fixture.root.join("画布 \"project\"");
    let config = root.join(".agents/mcp.json");
    fs::create_dir_all(config.parent().unwrap()).unwrap();
    fs::write(&config, b"malformed-project-secret{").unwrap();
    // When generating project-scoped setup instructions.
    let guide = export(&fixture.context, Some(&root)).unwrap();
    // Then metadata points only at the selected project, whose payload is exported in place.
    let blocks: Vec<_> = guide
        .text
        .split("```json\n")
        .skip(1)
        .map(|s| s.split("```").next().unwrap())
        .collect();
    let metadata: serde_json::Value = serde_json::from_str(blocks[0]).unwrap();
    let skill = root.join(".agents/skills").join(SKILL_NAME);
    assert_eq!(metadata["projectRoot"], json!(root));
    assert_eq!(metadata["mcpConfigPath"], json!(config));
    assert_eq!(metadata["skillPath"], json!(skill));
    assert_eq!(metadata["skillSourcePath"], json!(skill));
    for (relative, bytes) in PAYLOAD {
        assert_eq!(fs::read(skill.join(relative)).unwrap(), *bytes);
    }
    let connector: serde_json::Value = serde_json::from_str(blocks[1]).unwrap();
    assert_eq!(
        connector["mcpServers"]["ai-excalidraw"]["command"],
        json!(fixture.context.executable)
    );
    assert_eq!(fs::read(config).unwrap(), b"malformed-project-secret{");
    assert!(!fixture.context.home.exists());
    assert!(!fixture
        .context
        .bridge
        .parent()
        .unwrap()
        .join("integrations")
        .exists());
    assert!(!guide.text.contains("do-not-export-this-secret"));
    assert!(!guide.text.contains("malformed-project-secret"));
}
