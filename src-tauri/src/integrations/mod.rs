mod binaries;
mod config;
mod files;
mod guide;
mod model;
mod project;
#[cfg(test)]
mod project_tests;
#[cfg(test)]
mod tests;

pub use guide::Guide;
pub(crate) use model::Context;
pub use model::{InstallResult, IntegrationSnapshot, ToolId};
use model::{Status, ToolStatus};
use std::{
    path::{Path, PathBuf},
    sync::Mutex,
};
use tauri::{AppHandle, Manager};

const NAME: &str = "ai-excalidraw";
const SKILL_NAME: &str = "ai-excalidraw-desktop";
static INSTALL_LOCK: Mutex<()> = Mutex::new(());
const PAYLOAD: &[(&str, &[u8])] = &[
    (
        "SKILL.md",
        include_bytes!("../../../assets/agent-skill/SKILL.md"),
    ),
    (
        "references/elements.md",
        include_bytes!("../../../assets/agent-skill/references/elements.md"),
    ),
    (
        "references/local-setup.md",
        include_bytes!("../../../assets/agent-skill/references/local-setup.md"),
    ),
    (
        "scripts/canvas.mjs",
        include_bytes!("../../../assets/agent-skill/scripts/canvas.mjs"),
    ),
    (
        "agents/openai.yaml",
        include_bytes!("../../../assets/agent-skill/agents/openai.yaml"),
    ),
    (
        "assets/LICENSE-ai-excalidraw.txt",
        include_bytes!("../../../assets/agent-skill/assets/LICENSE-ai-excalidraw.txt"),
    ),
    (
        "VERSION",
        include_bytes!("../../../assets/agent-skill/VERSION"),
    ),
];

pub(crate) fn context(app: &AppHandle) -> Result<Context, String> {
    let home = match model::home_override(std::env::var_os("HOME"), std::env::var_os("USERPROFILE"))
    {
        Some(home) => home,
        None => app.path().home_dir().map_err(|e| e.to_string())?,
    };
    Ok(Context {
        codex: std::env::var_os("CODEX_HOME").map_or_else(|| home.join(".codex"), PathBuf::from),
        applications: vec![PathBuf::from("/Applications"), home.join("Applications")],
        home,
        executable: std::env::current_exe().map_err(|e| e.to_string())?,
        bridge: app
            .path()
            .app_data_dir()
            .map_err(|e| e.to_string())?
            .join("bridge.json"),
        search: std::env::var_os("PATH")
            .map(|p| std::env::split_paths(&p).collect())
            .unwrap_or_default(),
    })
}

/// # Errors
/// Returns an error when application paths, the registered project, or the installer lock are unavailable.
#[expect(
    clippy::needless_pass_by_value,
    reason = "Tauri command injection requires an owned AppHandle"
)]
#[tauri::command]
pub fn integration_status(
    app: AppHandle,
    project_id: Option<String>,
) -> Result<IntegrationSnapshot, String> {
    let _lock = INSTALL_LOCK.lock().map_err(|_| "安装器暂时不可用")?;
    let root = project::resolve(
        &app.path().app_data_dir().map_err(|e| e.to_string())?,
        project_id.as_deref(),
    )?;
    let context = context(&app)?;
    Ok(IntegrationSnapshot {
        version: env!("CARGO_PKG_VERSION").into(),
        // This command can only be invoked after Tauri has initialized the desktop service.
        service_running: true,
        tools: ToolId::ALL
            .into_iter()
            .map(|id| status(&context, id, root.as_deref()))
            .collect(),
    })
}

/// # Errors
/// Returns an error when application paths, the registered project, or the installer lock are unavailable.
#[expect(
    clippy::needless_pass_by_value,
    reason = "Tauri command injection requires an owned AppHandle"
)]
#[tauri::command]
pub fn integration_install(
    app: AppHandle,
    tools: Vec<ToolId>,
    project_id: Option<String>,
) -> Result<Vec<InstallResult>, String> {
    let _lock = INSTALL_LOCK.lock().map_err(|_| "安装器暂时不可用")?;
    let root = project::resolve(
        &app.path().app_data_dir().map_err(|e| e.to_string())?,
        project_id.as_deref(),
    )?;
    let context = context(&app)?;
    let mut results = Vec::new();
    for id in tools {
        if !results.iter().any(|r: &InstallResult| r.id == id) {
            results.push(install(&context, id, root.as_deref()));
        }
    }
    Ok(results)
}

fn status(context: &Context, id: ToolId, root: Option<&Path>) -> ToolStatus {
    let (config_path, skill_path) = project::paths(context, id, root);
    let config = config::prepare_scoped(context, id, root);
    let mcp_status = match &config {
        Ok((old, new)) => match old {
            None => Status::Missing,
            Some(old) if old == new => Status::Current,
            Some(_) => {
                if config::has_entry_scoped(context, id, root).unwrap_or(false) {
                    Status::Outdated
                } else {
                    Status::Missing
                }
            }
        },
        Err(_) => Status::Blocked,
    };
    let skill_status = match files::skill_status(&skill_path) {
        Ok(status) => status,
        Err(_) => Status::Blocked,
    };
    let detected = context.detected(id);
    let note = if config.is_err() || skill_status == Status::Blocked {
        Some("配置格式错误、无法读取或路径不安全；不会重置现有配置。".into())
    } else if !detected && (config_path.exists() || skill_path.exists()) {
        Some("发现配置文件，但未找到已安装的客户端程序。".into())
    } else {
        None
    };
    ToolStatus {
        id,
        name: id.name().into(),
        detected,
        config_path: config_path.display().to_string(),
        skill_path: skill_path.display().to_string(),
        mcp_status,
        skill_status,
        note,
    }
}

fn install(context: &Context, id: ToolId, root: Option<&Path>) -> InstallResult {
    let mut transaction = files::Transaction::default();
    let outcome = (|| {
        let (old, config) = config::prepare_scoped(context, id, root)?;
        let (config_path, skill_path) = project::paths(context, id, root);
        // Preflight every payload target before touching the configuration.
        files::skill_status(&skill_path)?;
        if files::read_optional(&config_path)? != old {
            return Err("安装期间配置已被其他程序修改，请重试。".into());
        }
        if old.as_deref() != Some(config.as_slice()) {
            transaction.write(&config_path, &config)?;
        }
        for (relative, data) in PAYLOAD {
            transaction.write(&skill_path.join(relative), data)?;
        }
        Ok::<(), String>(())
    })();
    let backup_paths = transaction.backups();
    let (success, message) = match outcome {
        Ok(()) => {
            transaction.commit();
            (
                true,
                "Skill 和原生 MCP 已安装，请重新加载客户端以应用更改。".into(),
            )
        }
        Err(error) => match transaction.rollback() {
            Ok(()) => (false, error),
            Err(rollback) => (
                false,
                format!("{error}；回滚失败：{rollback}。请从列出的备份恢复。"),
            ),
        },
    };
    InstallResult {
        id,
        success,
        message,
        backup_paths,
    }
}

/// # Errors
/// Returns an error when application paths or the installer lock are unavailable.
#[expect(
    clippy::needless_pass_by_value,
    reason = "Tauri command injection requires an owned AppHandle"
)]
#[tauri::command]
pub fn integration_claim_prompt(app: AppHandle) -> Result<bool, String> {
    let _lock = INSTALL_LOCK.lock().map_err(|_| "安装器暂时不可用")?;
    let context = context(&app)?;
    let flag = app
        .path()
        .app_data_dir()
        .map_err(|e| e.to_string())?
        .join("integration-prompt.json");
    claim_prompt(&context, &flag)
}

fn claim_prompt(context: &Context, flag: &std::path::Path) -> Result<bool, String> {
    files::safe_path(flag)?;
    if files::read_optional(flag)?.is_some() {
        return Ok(false);
    }
    let needed = ToolId::ALL.into_iter().any(|id| {
        let tool = status(context, id, None);
        tool.detected
            && (tool.mcp_status == Status::Missing || tool.skill_status == Status::Missing)
    });
    if !needed {
        return Ok(false);
    }
    let mut transaction = files::Transaction::default();
    transaction.write(flag, b"{\"claimed\":true}\n")?;
    transaction.commit();
    Ok(true)
}

/// # Errors
/// Returns an error when application paths, the registered project, payload export, or the installer lock fail.
#[expect(
    clippy::needless_pass_by_value,
    reason = "Tauri command injection requires an owned AppHandle"
)]
#[tauri::command]
pub fn integration_guide(app: AppHandle, project_id: Option<String>) -> Result<Guide, String> {
    let _lock = INSTALL_LOCK.lock().map_err(|_| "安装器暂时不可用")?;
    let root = project::resolve(
        &app.path().app_data_dir().map_err(|e| e.to_string())?,
        project_id.as_deref(),
    )?;
    guide::export(&context(&app)?, root.as_deref())
}
