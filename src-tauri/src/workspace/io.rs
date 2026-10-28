use serde_json::Value;
use std::{
    hash::{DefaultHasher, Hash, Hasher},
    io::Write,
    path::{Component, Path, PathBuf},
};
use uuid::Uuid;

pub type Result<T> = std::result::Result<T, String>;
pub fn revision(bytes: &[u8]) -> String {
    let mut hash = DefaultHasher::new();
    bytes.hash(&mut hash);
    format!("{:016x}", hash.finish())
}
pub fn relative(root: &Path, name: &str) -> Result<PathBuf> {
    let path = Path::new(name);
    if name.is_empty()
        || name.contains('\\')
        || path
            .components()
            .any(|c| !matches!(c, Component::Normal(_)))
    {
        return Err("路径必须是项目内的相对路径".into());
    }
    let mut current = root.to_path_buf();
    for part in path.components() {
        let value = part.as_os_str().to_string_lossy();
        if value.starts_with('.') || value.contains(':') {
            return Err("不允许隐藏目录或特殊文件名".into());
        }
        current.push(part);
        if let Ok(metadata) = current.symlink_metadata() {
            if metadata.file_type().is_symlink() {
                return Err("不允许通过符号链接访问项目外文件".into());
            }
        }
    }
    Ok(current)
}
pub fn canvas_path(root: &Path, name: &str) -> Result<PathBuf> {
    let path = relative(root, name)?;
    if path.extension().is_none_or(|ext| ext != "excalidraw") {
        return Err("画布文件需要 .excalidraw 扩展名".into());
    }
    Ok(path)
}
pub fn validate_canvas(content: &str) -> Result<Value> {
    if content.len() > 64 * 1024 * 1024 {
        return Err("画布超过 64 MB".into());
    }
    let value: Value = serde_json::from_str(content).map_err(|e| format!("画布 JSON 无效：{e}"))?;
    if value.get("type").and_then(Value::as_str) != Some("excalidraw")
        || !value.get("elements").is_some_and(Value::is_array)
    {
        return Err("不是有效的 Excalidraw 画布".into());
    }
    Ok(value)
}
pub fn atomic_write(path: &Path, bytes: &[u8]) -> Result<()> {
    let parent = path.parent().ok_or("文件缺少父目录")?;
    let temporary = parent.join(format!(".ai-excalidraw-{}.tmp", Uuid::new_v4()));
    let result = (|| {
        let mut options = std::fs::OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        let mut file = options.open(&temporary).map_err(|e| e.to_string())?;
        file.write_all(bytes)
            .and_then(|()| file.sync_all())
            .map_err(|e| e.to_string())?;
        std::fs::rename(&temporary, path).map_err(|e| e.to_string())
    })();
    if result.is_err() && temporary.exists() {
        if let Err(error) = std::fs::remove_file(&temporary) {
            eprintln!("Cannot remove temporary canvas file: {error}");
        }
    }
    result
}
pub fn create_canvas(path: &Path, content: &str) -> Result<String> {
    validate_canvas(content)?;
    // create_new never replaces an existing canvas, including a concurrent external creation.
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .map_err(|e| e.to_string())?;
    file.write_all(content.as_bytes())
        .and_then(|()| file.sync_all())
        .map_err(|e| e.to_string())?;
    Ok(revision(content.as_bytes()))
}
pub fn save_canvas(path: &Path, content: &str, expected: &str) -> Result<String> {
    validate_canvas(content)?;
    let previous = std::fs::read(path).map_err(|e| format!("无法保存，原文件可能已移动：{e}"))?;
    if revision(&previous) != expected {
        return Err("CONFLICT: 文件已被外部修改，请重新加载或另存副本".into());
    }
    atomic_write(path, content.as_bytes())?;
    Ok(revision(content.as_bytes()))
}
