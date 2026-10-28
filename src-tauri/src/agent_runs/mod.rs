mod memory_reconciliation;
mod model;
mod projection;
mod security;
mod skill_reconciliation;
#[cfg(unix)]
mod unix_io;
pub use model::*;
use std::{
    collections::HashSet,
    fs,
    io::{Read, Write},
    path::{Path, PathBuf},
    sync::Mutex,
    time::{SystemTime, UNIX_EPOCH},
};
use tauri::Manager;
use uuid::Uuid;
type Result<T> = std::result::Result<T, String>;
static SESSION: Mutex<Option<HashSet<PathBuf>>> = Mutex::new(None);
const LIMIT: u64 = 16 * 1024 * 1024;
mod dispatch;
mod operations;
mod recovery;
mod storage;
pub use dispatch::execute;
use recovery::{append, recover};
use storage::{directory, load, safe, save};
fn now() -> Result<u64> {
    u64::try_from(
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|_| "clock")?
            .as_millis(),
    )
    .map_err(|_| "clock".into())
}
fn bounded(value: &serde_json::Value) -> Result<()> {
    match value {
        serde_json::Value::String(s) if s.len() > 65536 => Err("string limit".into()),
        serde_json::Value::Array(a) => {
            if a.len() > 256 {
                return Err("array limit".into());
            }
            for v in a {
                bounded(v)?
            }
            Ok(())
        }
        serde_json::Value::Object(o) => {
            for v in o.values() {
                bounded(v)?
            }
            Ok(())
        }
        _ => Ok(()),
    }
}
#[tauri::command]
pub fn agent_run_command(app: tauri::AppHandle, request: Request) -> Result<serde_json::Value> {
    let data = app.path().app_data_dir().map_err(|_| "app data")?;
    let mut guard = SESSION.lock().map_err(|_| "store lock")?;
    execute(&data, request, guard.get_or_insert_with(HashSet::new))
}
#[cfg(test)]
mod tests;

/// Call during startup for global storage and each registered project.
/// Loads journals and records interruption without executing backend operations.
pub fn reconcile_scope(data: &Path, project_id: Option<String>) -> Result<serde_json::Value> {
    let mut guard = SESSION.lock().map_err(|_| "store lock")?;
    execute(
        data,
        Request::List { project_id },
        guard.get_or_insert_with(HashSet::new),
    )
}
#[cfg(test)]
mod security_tests;

#[cfg(test)]
mod context_tests;
#[cfg(test)]
mod memory_tests;
#[cfg(test)]
mod reconcile_tests;
#[cfg(test)]
mod skill_tests;

#[cfg(test)]
mod memory_privacy_tests;
