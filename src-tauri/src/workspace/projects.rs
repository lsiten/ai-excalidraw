use super::io::{atomic_write, Result};
use super::presentation::{self, PresentationData};
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
};
use uuid::Uuid;

#[derive(Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Project {
    pub id: String,
    pub name: String,
    pub directory: PathBuf,
}
#[derive(Default, Deserialize, Serialize)]
pub struct Registry {
    pub projects: Vec<Project>,
    #[serde(default)]
    pub session: serde_json::Value,
}
#[derive(Default, Deserialize, Serialize)]
pub struct Metadata {
    #[serde(default)]
    pub chats: BTreeMap<String, serde_json::Value>,
    #[serde(default)]
    pub presentation: PresentationData,
}
pub fn load_registry(data: &Path) -> Result<Registry> {
    let path = data.join("workspace.json");
    if !path.exists() {
        return Ok(Registry::default());
    }
    serde_json::from_slice(&std::fs::read(path).map_err(|e| e.to_string())?)
        .map_err(|e| e.to_string())
}
pub fn save_registry(data: &Path, registry: &Registry) -> Result<()> {
    std::fs::create_dir_all(data).map_err(|e| e.to_string())?;
    atomic_write(
        &data.join("workspace.json"),
        &serde_json::to_vec(registry).map_err(|e| e.to_string())?,
    )
}
pub fn open_project(data: &Path, directory: &str) -> Result<Project> {
    let root = std::fs::canonicalize(directory).map_err(|e| e.to_string())?;
    if !root.is_dir() {
        return Err("请选择目录".into());
    }
    let mut registry = load_registry(data)?;
    if let Some(project) = registry.projects.iter().find(|p| p.directory == root) {
        return Ok(project.clone());
    }
    let project = Project {
        id: Uuid::new_v4().to_string(),
        name: root
            .file_name()
            .map_or_else(|| "项目".into(), |s| s.to_string_lossy().into_owned()),
        directory: root,
    };
    registry.projects.insert(0, project.clone());
    save_registry(data, &registry)?;
    Ok(project)
}
pub fn project_root(data: &Path, id: &str) -> Result<PathBuf> {
    let project = load_registry(data)?
        .projects
        .into_iter()
        .find(|p| p.id == id)
        .ok_or("项目未打开，请先 open_project")?;
    let root = std::fs::canonicalize(&project.directory).map_err(|e| e.to_string())?;
    if root != project.directory {
        return Err("项目目录已改变，请重新打开".into());
    }
    Ok(root)
}
fn metadata_path(root: &Path) -> Result<PathBuf> {
    let directory = root.join(".ai-excalidraw");
    if let Ok(info) = directory.symlink_metadata() {
        if info.file_type().is_symlink() {
            return Err("项目辅助目录不能是符号链接".into());
        }
    }
    std::fs::create_dir_all(&directory).map_err(|e| e.to_string())?;
    let path = directory.join("project.json");
    if path
        .symlink_metadata()
        .is_ok_and(|info| info.file_type().is_symlink())
    {
        return Err("辅助文件不能是符号链接".into());
    }
    Ok(path)
}
pub fn load_metadata(root: &Path) -> Result<Metadata> {
    let path = metadata_path(root)?;
    if !path.exists() {
        return Ok(Metadata::default());
    }
    let metadata: Metadata =
        serde_json::from_slice(&std::fs::read(path).map_err(|e| e.to_string())?)
            .map_err(|e| e.to_string())?;
    metadata.presentation.validate(root)?;
    Ok(metadata)
}
pub fn save_metadata(root: &Path, metadata: &Metadata) -> Result<()> {
    atomic_write(
        &metadata_path(root)?,
        &serde_json::to_vec(metadata).map_err(|e| e.to_string())?,
    )
}
pub fn move_metadata(root: &Path, from: &str, to: Option<&str>) -> Result<()> {
    let mut metadata = load_metadata(root)?;
    presentation::move_keys(&mut metadata.chats, from, to);
    metadata.presentation.move_paths(from, to);
    save_metadata(root, &metadata)
}
