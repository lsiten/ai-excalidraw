#[cfg(any(target_os = "macos", target_os = "linux"))]
mod digest;
#[cfg(any(target_os = "macos", target_os = "linux"))]
mod fs;
#[cfg(any(target_os = "macos", target_os = "linux"))]
mod global_catalog;
#[cfg(any(target_os = "macos", target_os = "linux"))]
mod manifest;
#[cfg(any(target_os = "macos", target_os = "linux"))]
mod memory;
#[cfg(any(target_os = "macos", target_os = "linux"))]
mod service;
#[cfg(any(target_os = "macos", target_os = "linux"))]
mod skill;
#[cfg(all(test, any(target_os = "macos", target_os = "linux")))]
mod tests;
#[cfg(any(target_os = "macos", target_os = "linux"))]
mod transaction;
use serde::{Deserialize, Serialize};
use std::{collections::BTreeSet, sync::Mutex};
use tauri::Manager;

pub type Result<T> = std::result::Result<T, String>;
static LOCK: Mutex<()> = Mutex::new(());
#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "camelCase")]
pub enum Capability {
    Read,
    WriteSkill,
    WriteMemory,
    WritePolicy,
    Mcp,
}
#[derive(Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Policy {
    #[serde(default)]
    pub allow: BTreeSet<Capability>,
    #[serde(default)]
    pub deny: BTreeSet<Capability>,
    #[serde(default)]
    pub budgets: Budgets,
}
impl Policy {
    pub fn validate(&self) -> Result<()> {
        if self
            .budgets
            .timeout_ms
            .is_some_and(|v| v > 9_007_199_254_740_991)
        {
            return Err("timeoutMs exceeds supported integer range".into());
        }
        Ok(())
    }
    pub fn permits(&self, capability: Capability, native_allowed: bool) -> bool {
        native_allowed
            && !self.deny.contains(&capability)
            && (self.allow.is_empty() || self.allow.contains(&capability))
    }
}
#[derive(Clone, Copy, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Budgets {
    pub max_turns: Option<u32>,
    pub repair_limit: Option<u32>,
    pub timeout_ms: Option<u64>,
}
impl Budgets {
    pub fn restrict(self, native: Self) -> Self {
        fn minimum<T: Ord>(a: Option<T>, b: Option<T>) -> Option<T> {
            match (a, b) {
                (Some(a), Some(b)) => Some(a.min(b)),
                (a, None) => a,
                (None, b) => b,
            }
        }
        Self {
            max_turns: minimum(self.max_turns, native.max_turns),
            repair_limit: minimum(self.repair_limit, native.repair_limit),
            timeout_ms: minimum(self.timeout_ms, native.timeout_ms),
        }
    }
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ResourceFile {
    pub path: String,
    pub content: String,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Request {
    pub project_id: String,
    #[serde(flatten)]
    pub operation: Operation,
}
#[derive(Deserialize)]
#[serde(tag = "op", rename_all = "camelCase", rename_all_fields = "camelCase")]
pub enum Operation {
    Manifest,
    Read {
        path: String,
    },
    WriteSkill {
        name: String,
        content: String,
        #[serde(default)]
        files: Vec<ResourceFile>,
        expected_revision: Option<String>,
        authorized: bool,
    },
    WriteMemory {
        name: String,
        content: String,
        expected_revision: Option<String>,
        authorized: bool,
        confirmed: bool,
    },
    WritePolicy {
        policy: Policy,
        expected_revision: Option<String>,
        authorized: bool,
    },
}
#[tauri::command]
pub fn agent_project_command(app: tauri::AppHandle, request: Request) -> Result<serde_json::Value> {
    let _guard = LOCK.lock().map_err(|e| e.to_string())?;
    let data = app.path().app_data_dir().map_err(|e| e.to_string())?;
    dispatch(&data, request)
}

pub fn dispatch(data: &std::path::Path, request: Request) -> Result<serde_json::Value> {
    let root = crate::workspace::project_root(data, &request.project_id)?;
    #[cfg(any(target_os = "macos", target_os = "linux"))]
    {
        service::dispatch(&root, request.operation)
    }
    #[cfg(not(any(target_os = "macos", target_os = "linux")))]
    {
        let _ = (root, request);
        Err("secure project file transactions require macOS or Linux".into())
    }
}
