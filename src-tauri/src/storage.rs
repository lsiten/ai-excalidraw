use serde_json::Value;
use std::{collections::BTreeMap, io::Write, path::PathBuf};
use tauri::Manager;
use uuid::Uuid;

fn canvas_key(key: &str) -> bool {
    key.starts_with("excalidraw-canvas-data")
        && key.len() <= 200
        && key
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-' || byte == b'_')
}

fn directory(app: &tauri::AppHandle) -> Result<PathBuf, String> {
    app.path()
        .app_data_dir()
        .map(|path| path.join("canvases"))
        .map_err(|error| error.to_string())
}

#[tauri::command]
#[expect(
    clippy::needless_pass_by_value,
    reason = "Tauri extracts command arguments by value"
)]
pub fn save_canvas_snapshot(
    app: tauri::AppHandle,
    key: String,
    value: Value,
) -> Result<(), String> {
    if !canvas_key(&key) || !value.get("elements").is_some_and(Value::is_array) {
        return Err("Invalid canvas snapshot".to_owned());
    }
    let folder = directory(&app)?;
    std::fs::create_dir_all(&folder).map_err(|error| error.to_string())?;
    let temporary = folder.join(format!("{}.tmp", Uuid::new_v4()));
    let bytes = serde_json::to_vec(&value).map_err(|error| error.to_string())?;
    let mut options = std::fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = options
        .open(&temporary)
        .map_err(|error| error.to_string())?;
    file.write_all(&bytes)
        .and_then(|()| file.sync_all())
        .map_err(|error| error.to_string())?;
    std::fs::rename(temporary, folder.join(format!("{key}.json")))
        .map_err(|error| error.to_string())
}

#[tauri::command]
#[expect(
    clippy::needless_pass_by_value,
    reason = "Tauri extracts command arguments by value"
)]
pub fn remove_canvas_snapshot(app: tauri::AppHandle, key: String) -> Result<(), String> {
    if !canvas_key(&key) {
        return Err("Invalid canvas key".to_owned());
    }
    match std::fs::remove_file(directory(&app)?.join(format!("{key}.json"))) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error.to_string()),
    }
}

#[tauri::command]
#[expect(
    clippy::needless_pass_by_value,
    reason = "Tauri extracts command arguments by value"
)]
pub fn load_canvas_snapshots(app: tauri::AppHandle) -> Result<BTreeMap<String, Value>, String> {
    let folder = directory(&app)?;
    if !folder.exists() {
        return Ok(BTreeMap::new());
    }
    let mut snapshots = BTreeMap::new();
    for entry in std::fs::read_dir(folder).map_err(|error| error.to_string())? {
        let path = entry.map_err(|error| error.to_string())?.path();
        if path.extension().is_none_or(|extension| extension != "json") {
            continue;
        }
        if let Some(key) = path
            .file_stem()
            .and_then(|stem| stem.to_str())
            .filter(|key| canvas_key(key))
        {
            let bytes = std::fs::read(&path).map_err(|error| error.to_string())?;
            let value = serde_json::from_slice(&bytes).map_err(|error| error.to_string())?;
            snapshots.insert(key.to_owned(), value);
        }
    }
    Ok(snapshots)
}

#[cfg(test)]
mod tests {
    use super::canvas_key;
    #[test]
    fn rejects_paths_outside_the_canvas_directory() {
        // Given traversal input, when a storage key is parsed, then it is rejected.
        assert!(!canvas_key("excalidraw-canvas-data/../../config"));
    }
    #[test]
    fn accepts_session_canvas_keys() {
        // Given a session key, when parsed, then it is accepted.
        assert!(canvas_key("excalidraw-canvas-data-1234-abcd"));
    }
}
