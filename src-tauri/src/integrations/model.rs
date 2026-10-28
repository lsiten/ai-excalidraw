use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum ToolId {
    Codex,
    Claude,
    Cursor,
    Pi,
}
impl ToolId {
    pub const ALL: [Self; 4] = [Self::Codex, Self::Claude, Self::Cursor, Self::Pi];
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Codex => "Codex",
            Self::Claude => "Claude Code",
            Self::Cursor => "Cursor",
            Self::Pi => "Pi",
        }
    }
    const fn binary(self) -> &'static str {
        match self {
            Self::Codex => "codex",
            Self::Claude => "claude",
            Self::Cursor => "cursor",
            Self::Pi => "pi",
        }
    }
}
#[derive(Clone, Copy, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Status {
    Missing,
    Current,
    Outdated,
    Blocked,
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolStatus {
    pub id: ToolId,
    pub name: String,
    pub detected: bool,
    pub config_path: String,
    pub skill_path: String,
    pub mcp_status: Status,
    pub skill_status: Status,
    pub note: Option<String>,
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct IntegrationSnapshot {
    pub version: String,
    pub service_running: bool,
    pub tools: Vec<ToolStatus>,
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InstallResult {
    pub id: ToolId,
    pub success: bool,
    pub message: String,
    pub backup_paths: Vec<String>,
}

pub struct Context {
    pub home: PathBuf,
    pub codex: PathBuf,
    pub executable: PathBuf,
    pub bridge: PathBuf,
    pub search: Vec<PathBuf>,
    pub applications: Vec<PathBuf>,
}
impl Context {
    pub fn paths(&self, id: ToolId) -> (PathBuf, PathBuf) {
        let (config, base) = match id {
            ToolId::Codex => (self.codex.join("config.toml"), self.codex.clone()),
            ToolId::Claude => (self.home.join(".claude.json"), self.home.join(".claude")),
            ToolId::Cursor => (
                self.home.join(".cursor/mcp.json"),
                self.home.join(".cursor"),
            ),
            ToolId::Pi => (
                self.home.join(".pi/agent/mcp.json"),
                self.home.join(".pi/agent"),
            ),
        };
        (config, base.join("skills").join(super::SKILL_NAME))
    }
    pub fn detected(&self, id: ToolId) -> bool {
        let mut candidates: Vec<PathBuf> =
            self.search.iter().map(|p| p.join(id.binary())).collect();
        candidates.push(self.home.join(".local/bin").join(id.binary()));
        match id {
            ToolId::Pi => candidates.push(self.home.join(".pi/agent/bin/pi")),
            ToolId::Codex => candidates.push(self.codex.join("bin/codex")),
            ToolId::Claude => candidates.push(self.home.join(".claude/local/claude")),
            ToolId::Cursor => {}
        }
        if candidates.iter().any(|path| {
            path.metadata().is_ok_and(|m| {
                #[cfg(unix)]
                {
                    use std::os::unix::fs::PermissionsExt;
                    m.is_file() && m.permissions().mode() & 0o111 != 0
                }
                #[cfg(not(unix))]
                {
                    m.is_file()
                }
            })
        }) {
            return true;
        }
        let app = match id {
            ToolId::Codex => Some("Codex"),
            ToolId::Cursor => Some("Cursor"),
            ToolId::Claude | ToolId::Pi => None,
        };
        app.is_some_and(|name| {
            self.applications
                .iter()
                .any(|base| base.join(format!("{name}.app/Contents/MacOS")).is_dir())
        })
    }
}

pub fn home_override(
    home: Option<std::ffi::OsString>,
    profile: Option<std::ffi::OsString>,
) -> Option<PathBuf> {
    home.into_iter()
        .chain(profile)
        .map(PathBuf::from)
        .find(|path| path.is_absolute())
}
