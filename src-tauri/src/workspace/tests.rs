#![expect(
    clippy::unwrap_used,
    reason = "Test setup and assertions fail immediately on unexpected errors"
)]
use super::{dispatch, files::EMPTY_CANVAS, io, Request};
use serde_json::{json, Value};
use std::path::PathBuf;
use uuid::Uuid;
pub(super) struct Fixture {
    base: PathBuf,
    pub(super) data: PathBuf,
    pub(super) root: PathBuf,
    pub(super) id: String,
}
impl Fixture {
    pub(super) fn new() -> Self {
        let base = std::env::temp_dir().join(format!("ai-excalidraw-test-{}", Uuid::new_v4()));
        let data = base.join("app");
        let root = base.join("project");
        std::fs::create_dir_all(&root).unwrap();
        let opened = dispatch(
            &data,
            Request::OpenProject {
                directory: root.to_string_lossy().into_owned(),
            },
        )
        .unwrap();
        Self {
            base,
            data,
            root,
            id: opened["id"].as_str().unwrap().to_owned(),
        }
    }
    pub(super) fn call(&self, tool: &str, arguments: Value) -> super::io::Result<Value> {
        let mut arguments = arguments;
        arguments["projectId"] = json!(self.id);
        let request = serde_json::from_value(json!({"tool": tool, "arguments": arguments}))
            .map_err(|e| e.to_string())?;
        dispatch(&self.data, request)
    }

    pub(super) fn snapshot(&self, name: &str) -> Value {
        let bytes = std::fs::read(self.root.join(".ai-excalidraw/project.json")).unwrap();
        if let Some(directory) = std::env::var_os("PRESENTATION_EVIDENCE_DIR") {
            let directory = std::path::PathBuf::from(directory);
            std::fs::create_dir_all(&directory).unwrap();
            std::fs::write(directory.join(format!("{name}.json")), &bytes).unwrap();
        }
        serde_json::from_slice(&bytes).unwrap()
    }

    pub(super) fn write_presentation(&self, presentation: &Value) -> super::io::Result<Value> {
        self.call("write_presentation", json!({"presentation": presentation}))
    }
    pub(super) fn with_presentation() -> (Self, Value) {
        let f = Self::new();
        std::fs::create_dir_all(f.root.join("模块/子目录")).unwrap();
        for path in [
            "模块/子目录/A.excalidraw",
            "模块/B.excalidraw",
            "模块外.excalidraw",
        ] {
            f.create(path);
        }
        let area = json!({"x": -12.5, "y": 20, "width": 640, "height": 480});
        let value = json!({
            "decks": {
                "": {"order": ["模块外.excalidraw", "模块/子目录/A.excalidraw"], "includeSubfolders": true},
                "模块": {"order": ["模块/B.excalidraw", "模块/子目录/A.excalidraw"], "includeSubfolders": false},
                "模块/子目录": {"order": ["模块/子目录/A.excalidraw"], "includeSubfolders": true}
            },
            "areas": {"模块/子目录/A.excalidraw": area, "模块外.excalidraw": area}
        });
        f.call(
            "write_chats",
            json!({"path": "模块/子目录/A.excalidraw", "chats": [{"id": "original"}]}),
        )
        .unwrap();
        f.write_presentation(&value).unwrap();
        (f, value)
    }

    pub(super) fn create(&self, name: &str) -> Value {
        dispatch(
            &self.data,
            Request::CreateCanvas {
                project_id: self.id.clone(),
                path: name.into(),
                content: None,
            },
        )
        .unwrap()
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        std::fs::remove_dir_all(&self.base).unwrap();
    }
}
#[test]
fn project_registration_survives_restart_without_duplicate_projects() {
    let f = Fixture::new();
    let opened = dispatch(
        &f.data,
        Request::OpenProject {
            directory: f.root.to_string_lossy().into_owned(),
        },
    )
    .unwrap();
    assert_eq!(opened["id"], f.id);
    let projects = dispatch(&f.data, Request::ListProjects {}).unwrap();
    assert_eq!(projects["projects"].as_array().unwrap().len(), 1);
}
#[test]
fn independent_files_preserve_images_and_canvas_state() {
    let f = Fixture::new();
    f.create("架构.excalidraw");
    f.create("流程.excalidraw");
    let content = json!({"type":"excalidraw","elements":[{"id":"test"}],"appState":{"viewBackgroundColor":"#eee"},"files":{"image":{"dataURL":"data:image/png;base64,123"}}}).to_string();
    let original = std::fs::read(f.root.join("架构.excalidraw")).unwrap();
    io::save_canvas(
        &f.root.join("架构.excalidraw"),
        &content,
        &io::revision(&original),
    )
    .unwrap();
    assert_eq!(
        std::fs::read_to_string(f.root.join("架构.excalidraw")).unwrap(),
        content
    );
    assert_eq!(
        std::fs::read_to_string(f.root.join("流程.excalidraw")).unwrap(),
        EMPTY_CANVAS
    );
}
#[test]
fn external_changes_and_deletions_are_never_overwritten() {
    let f = Fixture::new();
    let created = f.create("A.excalidraw");
    let path = f.root.join("A.excalidraw");
    std::fs::write(&path, "external edit").unwrap();
    assert!(
        io::save_canvas(&path, EMPTY_CANVAS, created["revision"].as_str().unwrap())
            .unwrap_err()
            .contains("CONFLICT")
    );
    assert_eq!(std::fs::read_to_string(&path).unwrap(), "external edit");
    std::fs::remove_file(&path).unwrap();
    assert!(io::save_canvas(&path, EMPTY_CANVAS, "revision").is_err());
    assert!(!path.exists());
}
#[test]
fn rejects_traversal_absolute_hidden_and_invalid_file_paths() {
    let f = Fixture::new();
    for path in [
        "../outside.excalidraw",
        "/tmp/outside.excalidraw",
        ".ai-excalidraw/config.excalidraw",
        "a/../../out.excalidraw",
        "file.txt",
        "C:\\outside.excalidraw",
    ] {
        assert!(io::canvas_path(&f.root, path).is_err(), "{path}");
    }
}
#[cfg(unix)]
#[test]
fn rejects_symlinks_inside_project() {
    let f = Fixture::new();
    std::os::unix::fs::symlink(&f.data, f.root.join("escape")).unwrap();
    assert!(io::canvas_path(&f.root, "escape/file.excalidraw").is_err());
}
#[test]
fn create_and_rename_never_replace_existing_files() {
    let f = Fixture::new();
    f.create("A.excalidraw");
    f.create("B.excalidraw");
    assert!(dispatch(
        &f.data,
        Request::CreateCanvas {
            project_id: f.id.clone(),
            path: "A.excalidraw".into(),
            content: None
        }
    )
    .is_err());
    assert!(dispatch(
        &f.data,
        Request::RenameFile {
            project_id: f.id.clone(),
            from: "A.excalidraw".into(),
            to: "B.excalidraw".into()
        }
    )
    .is_err());
}
#[test]
fn nested_folder_rename_moves_associated_conversations() {
    let f = Fixture::new();
    std::fs::create_dir(f.root.join("模块")).unwrap();
    f.create("模块/A.excalidraw");
    dispatch(
        &f.data,
        Request::WriteChats {
            project_id: f.id.clone(),
            path: "模块/A.excalidraw".into(),
            chats: json!([{"id":"chat1"}]),
        },
    )
    .unwrap();
    dispatch(
        &f.data,
        Request::RenameFile {
            project_id: f.id.clone(),
            from: "模块".into(),
            to: "业务".into(),
        },
    )
    .unwrap();
    let chats = dispatch(
        &f.data,
        Request::ReadChats {
            project_id: f.id.clone(),
            path: "业务/A.excalidraw".into(),
        },
    )
    .unwrap();
    assert_eq!(chats[0]["id"], "chat1");
    assert!(f.root.join("业务/A.excalidraw").exists());
}
#[test]
fn malformed_import_is_rejected_without_creating_target() {
    let f = Fixture::new();
    assert!(io::create_canvas(&f.root.join("bad.excalidraw"), "{\"elements\":[]}").is_err());
    assert!(!f.root.join("bad.excalidraw").exists());
}
#[test]
fn file_tree_skips_dependency_and_build_directories() {
    let f = Fixture::new();
    std::fs::create_dir(f.root.join("node_modules")).unwrap();
    std::fs::create_dir(f.root.join("target")).unwrap();
    std::fs::write(f.root.join("node_modules/hidden.excalidraw"), EMPTY_CANVAS).unwrap();
    f.create("visible.excalidraw");
    let listed = dispatch(
        &f.data,
        Request::ListFiles {
            project_id: f.id.clone(),
        },
    )
    .unwrap();
    assert_eq!(listed["entries"].as_array().unwrap().len(), 1);
    assert_eq!(listed["entries"][0]["name"], "visible.excalidraw");
}
