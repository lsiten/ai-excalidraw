use std::sync::{
    atomic::{AtomicBool, Ordering},
    Mutex,
};
use tauri::{Emitter, Manager};
#[derive(Default)]
pub struct Lifecycle {
    pub exiting: AtomicBool,
    pub files: Mutex<Vec<String>>,
}
#[tauri::command]
#[expect(
    clippy::needless_pass_by_value,
    reason = "Tauri extracts native command arguments by value"
)]
pub fn take_launch_files(state: tauri::State<'_, Lifecycle>) -> Result<Vec<String>, String> {
    Ok(std::mem::take(
        &mut *state.files.lock().map_err(|e| e.to_string())?,
    ))
}
#[tauri::command]
#[expect(
    clippy::needless_pass_by_value,
    reason = "Tauri extracts native command arguments by value"
)]
pub fn workspace_exit(app: tauri::AppHandle, state: tauri::State<'_, Lifecycle>) {
    state.exiting.store(true, Ordering::Release);
    app.exit(0);
}
pub fn open_files(app: &tauri::AppHandle, files: Vec<String>) {
    let state = app.state::<Lifecycle>();
    if let Ok(mut pending) = state.files.lock() {
        pending.extend(files.clone());
    }
    if let Err(error) = app.emit_to("main", "open-files", files) {
        eprintln!("Cannot open canvas files: {error}");
    }
}
