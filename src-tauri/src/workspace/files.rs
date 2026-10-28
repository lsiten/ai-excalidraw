use super::{
    io::{self, Result},
    native, projects,
};
use serde::Serialize;
use serde_json::{json, Value};
use std::path::Path;

pub const EMPTY_CANVAS: &str = r#"{"type":"excalidraw","version":2,"source":"AI Excalidraw Desktop","elements":[],"appState":{},"files":{}}"#;
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Entry {
    path: String,
    name: String,
    kind: &'static str,
}
pub fn list(root: &Path, parent: &Path, entries: &mut Vec<Entry>) -> Result<()> {
    for entry in std::fs::read_dir(parent).map_err(|e| e.to_string())? {
        let entry = entry.map_err(|e| e.to_string())?;
        let name = entry.file_name().to_string_lossy().into_owned();
        let kind = entry.file_type().map_err(|e| e.to_string())?;
        if name.starts_with('.')
            || kind.is_symlink()
            || matches!(name.as_str(), "node_modules" | "target")
        {
            continue;
        }
        let path = entry.path();
        let is_canvas = path.extension().is_some_and(|ext| ext == "excalidraw");
        if kind.is_dir() || is_canvas {
            if entries.len() >= 5000 {
                return Err("项目超过 5000 个目录或画布，请打开较小的子目录".into());
            }
            entries.push(Entry {
                path: path
                    .strip_prefix(root)
                    .map_err(|e| e.to_string())?
                    .to_string_lossy()
                    .into_owned(),
                name,
                kind: if kind.is_dir() { "directory" } else { "canvas" },
            });
            if kind.is_dir() {
                list(root, &path, entries)?;
            }
        }
    }
    entries.sort_by(|a, b| a.path.cmp(&b.path));
    Ok(())
}
pub fn read(root: &Path, name: &str) -> Result<Value> {
    let path = io::canvas_path(root, name)?;
    let metadata = path.metadata().map_err(|e| e.to_string())?;
    if metadata.len() > 64 * 1024 * 1024 {
        return Err("画布超过 64 MB".into());
    }
    let content = std::fs::read_to_string(path).map_err(|e| e.to_string())?;
    io::validate_canvas(&content)?;
    Ok(json!({"content": content, "revision": io::revision(content.as_bytes())}))
}
pub fn rename(root: &Path, from: &str, to: &str) -> Result<Value> {
    let source = io::relative(root, from)?;
    let target = io::relative(root, to)?;
    if source.is_file() {
        io::canvas_path(root, from)?;
        io::canvas_path(root, to)?;
    }
    if target.exists() {
        return Err("目标已存在，请选择其他名称".into());
    }
    if target.starts_with(&source) {
        return Err("不能将目录移动到自身内部".into());
    }
    // Move metadata first so an invalid metadata file never leaves the document silently renamed.
    let previous = projects::load_metadata(root)?;
    projects::move_metadata(root, from, Some(to))?;
    if let Err(error) = std::fs::rename(&source, &target) {
        projects::save_metadata(root, &previous)?;
        return Err(error.to_string());
    }
    Ok(json!({"from": from, "to": to}))
}

pub fn duplicate(root: &Path, from: &str, to: &str) -> Result<Value> {
    let original = read(root, from)?;
    let content = original["content"].as_str().ok_or("无效的画布")?;
    let mut metadata = projects::load_metadata(root)?;
    let area = metadata.presentation.areas.get(from).copied();
    if let Some(area) = area {
        metadata.presentation.areas.insert(to.into(), area);
        metadata.presentation.validate(root)?;
    }
    io::create_canvas(&io::canvas_path(root, to)?, content)?;
    if area.is_some() {
        projects::save_metadata(root, &metadata)?;
    }
    Ok(json!({"path": to}))
}

pub fn delete(root: &Path, path: &str) -> Result<Value> {
    let file = io::relative(root, path)?;
    if file.is_file() {
        io::canvas_path(root, path)?;
    }
    native::trash(&file.to_string_lossy())?;
    projects::move_metadata(root, path, None)?;
    Ok(json!({"trashed": true}))
}
