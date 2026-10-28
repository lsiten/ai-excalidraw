mod files;
mod io;
mod native;
mod presentation;
#[cfg(test)]
mod presentation_tests;
mod projects;
pub(crate) use projects::{load_registry, project_root};
#[cfg(test)]
mod tests;
use io::Result;
use serde::Deserialize;
use serde_json::{json, Value};
use std::{path::Path, sync::Mutex};
use tauri::Manager;

#[derive(Default)]
pub struct WorkspaceState(Mutex<()>);
#[derive(Deserialize)]
#[serde(
    tag = "tool",
    content = "arguments",
    rename_all = "snake_case",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum Request {
    ReadPresentation {
        project_id: String,
    },
    WritePresentation {
        project_id: String,
        presentation: presentation::PresentationData,
    },
    ListProjects {},
    OpenProject {
        directory: String,
    },
    ListFiles {
        project_id: String,
    },
    CreateCanvas {
        project_id: String,
        path: String,
        content: Option<String>,
    },
    ReadCanvasFile {
        project_id: String,
        path: String,
    },
    SaveCanvasFile {
        project_id: String,
        path: String,
        content: String,
        revision: String,
    },
    RenameFile {
        project_id: String,
        from: String,
        to: String,
    },
    DuplicateCanvas {
        project_id: String,
        from: String,
        to: String,
    },
    CreateDirectory {
        project_id: String,
        path: String,
    },
    DeleteFile {
        project_id: String,
        path: String,
    },
    RevealFile {
        project_id: String,
        path: Option<String>,
    },
    ImportCanvas {
        project_id: String,
        source: String,
        path: String,
    },
    PickPath {
        kind: String,
    },
    ExportCanvas {
        content: String,
    },
    LoadWorkspaceSession {},
    SaveWorkspaceSession {
        session: Value,
    },
    ReadChats {
        project_id: String,
        path: String,
    },
    WriteChats {
        project_id: String,
        path: String,
        chats: Value,
    },
}
#[tauri::command]
pub async fn workspace_command(
    app: tauri::AppHandle,
    state: tauri::State<'_, WorkspaceState>,
    request: Request,
) -> Result<Value> {
    let _guard = state.0.lock().map_err(|e| e.to_string())?;
    let data = app.path().app_data_dir().map_err(|e| e.to_string())?;
    dispatch(&data, request)
}
#[expect(
    clippy::too_many_lines,
    reason = "Exhaustive request dispatcher; file and IO logic lives in separate modules"
)]
pub fn dispatch(data: &Path, request: Request) -> Result<Value> {
    use Request::{
        CreateCanvas, CreateDirectory, DeleteFile, DuplicateCanvas, ExportCanvas, ImportCanvas,
        ListFiles, ListProjects, LoadWorkspaceSession, OpenProject, PickPath, ReadCanvasFile,
        ReadChats, ReadPresentation, RenameFile, RevealFile, SaveCanvasFile, SaveWorkspaceSession,
        WriteChats, WritePresentation,
    };
    match request {
        ReadPresentation { project_id } => {
            presentation::read(&projects::project_root(data, &project_id)?)
        }
        WritePresentation {
            project_id,
            presentation,
        } => presentation::write(&projects::project_root(data, &project_id)?, presentation),
        ListProjects {} => Ok(json!({"projects": projects::load_registry(data)?.projects})),
        OpenProject { directory } => Ok(json!(projects::open_project(data, &directory)?)),
        PickPath { kind } => Ok(json!({"path": native::pick(&kind)?})),
        LoadWorkspaceSession {} => Ok(projects::load_registry(data)?.session),
        SaveWorkspaceSession { session } => {
            let mut registry = projects::load_registry(data)?;
            registry.session = session;
            projects::save_registry(data, &registry)?;
            Ok(json!({"saved": true}))
        }
        ExportCanvas { content } => {
            io::validate_canvas(&content)?;
            let selected = native::pick("save")?;
            if selected.is_empty() {
                return Ok(json!({"cancelled": true}));
            }
            let mut path = std::path::PathBuf::from(&selected);
            if path.extension().is_none_or(|ext| ext != "excalidraw") {
                path.set_extension("excalidraw");
            }
            io::atomic_write(&path, content.as_bytes())?;
            Ok(json!({"path": path}))
        }
        ListFiles { project_id } => {
            let root = projects::project_root(data, &project_id)?;
            let mut entries = Vec::new();
            files::list(&root, &root, &mut entries)?;
            Ok(json!({"entries": entries}))
        }
        CreateCanvas {
            project_id,
            path,
            content,
        } => {
            let root = projects::project_root(data, &project_id)?;
            let revision = io::create_canvas(
                &io::canvas_path(&root, &path)?,
                content.as_deref().unwrap_or(files::EMPTY_CANVAS),
            )?;
            Ok(json!({"path": path, "revision": revision}))
        }
        ReadCanvasFile { project_id, path } => {
            files::read(&projects::project_root(data, &project_id)?, &path)
        }
        SaveCanvasFile {
            project_id,
            path,
            content,
            revision,
        } => {
            let root = projects::project_root(data, &project_id)?;
            let next = io::save_canvas(&io::canvas_path(&root, &path)?, &content, &revision)?;
            Ok(json!({"revision": next}))
        }
        CreateDirectory { project_id, path } => {
            std::fs::create_dir(io::relative(
                &projects::project_root(data, &project_id)?,
                &path,
            )?)
            .map_err(|e| e.to_string())?;
            Ok(json!({"path": path}))
        }
        RenameFile {
            project_id,
            from,
            to,
        } => files::rename(&projects::project_root(data, &project_id)?, &from, &to),
        DuplicateCanvas {
            project_id,
            from,
            to,
        } => files::duplicate(&projects::project_root(data, &project_id)?, &from, &to),
        DeleteFile { project_id, path } => {
            files::delete(&projects::project_root(data, &project_id)?, &path)
        }
        RevealFile { project_id, path } => {
            let root = projects::project_root(data, &project_id)?;
            let target = path.map_or_else(|| Ok(root.clone()), |p| io::relative(&root, &p))?;
            native::reveal(&target.to_string_lossy())?;
            Ok(json!({"revealed": true}))
        }
        ImportCanvas {
            project_id,
            source,
            path,
        } => {
            let source = std::path::Path::new(&source);
            if source.extension().is_none_or(|ext| ext != "excalidraw") {
                return Err("请选择 .excalidraw 文件".into());
            }
            if source.metadata().map_err(|e| e.to_string())?.len() > 64 * 1024 * 1024 {
                return Err("画布超过 64 MB".into());
            }
            let content = std::fs::read_to_string(source).map_err(|e| e.to_string())?;
            let root = projects::project_root(data, &project_id)?;
            io::create_canvas(&io::canvas_path(&root, &path)?, &content)?;
            Ok(json!({"path": path}))
        }
        ReadChats { project_id, path } => {
            let root = projects::project_root(data, &project_id)?;
            io::canvas_path(&root, &path)?;
            Ok(projects::load_metadata(&root)?
                .chats
                .remove(&path)
                .unwrap_or_else(|| json!([])))
        }
        WriteChats {
            project_id,
            path,
            chats,
        } => {
            let root = projects::project_root(data, &project_id)?;
            io::canvas_path(&root, &path)?;
            if !chats.is_array() {
                return Err("对话记录必须是数组".into());
            }
            let mut metadata = projects::load_metadata(&root)?;
            metadata.chats.insert(path, chats);
            projects::save_metadata(&root, &metadata)?;
            Ok(json!({"saved": true}))
        }
    }
}
