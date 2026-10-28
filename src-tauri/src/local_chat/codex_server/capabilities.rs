use super::super::process::{capture, ProcessControl};
use crate::integrations::ToolId;
use serde::Serialize;
use std::{
    path::Path,
    process::Stdio,
    sync::{Arc, Mutex},
    time::Duration,
};
#[expect(
    clippy::struct_excessive_bools,
    reason = "Independent public capability flags are the IPC contract"
)]
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Capabilities {
    pub backend: &'static str,
    pub supports_resume: bool,
    pub supports_supplement: bool,
    pub supports_approval: bool,
    pub supports_usage: bool,
    pub supports_summary: bool,
}
impl Capabilities {
    pub fn for_tool(tool: ToolId, native: bool) -> Self {
        Self {
            backend: match tool {
                ToolId::Codex if native => "codex-app-server",
                ToolId::Codex => "codex-exec",
                ToolId::Claude => "claude-cli",
                ToolId::Pi => "pi-cli",
                ToolId::Cursor => "cursor-cli",
            },
            supports_resume: native,
            supports_supplement: false,
            supports_approval: false,
            supports_usage: native,
            supports_summary: native,
        }
    }
}
pub(in crate::local_chat) async fn detect(binary: &Path) -> bool {
    for (args, marker) in [
        (vec!["--help"], "app-server"),
        (vec!["app-server", "--help"], "Usage:"),
    ] {
        let mut command = tokio::process::Command::new(binary);
        let paths = binary.parent().into_iter().map(Path::to_path_buf).chain(
            std::env::var_os("PATH")
                .into_iter()
                .flat_map(|value| std::env::split_paths(&value).collect::<Vec<_>>()),
        );
        if let Ok(path) = std::env::join_paths(paths) {
            command.env("PATH", path);
        }

        command
            .args(args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        let (_tx, cancel) = tokio::sync::watch::channel(false);
        let mut control = ProcessControl {
            cancel,
            process: Arc::new(Mutex::new(None)),
            duration: Duration::from_secs(3),
        };
        match capture(&mut command, "", &mut control).await {
            Ok(output)
                if output.status.success()
                    && String::from_utf8_lossy(&output.stdout).contains(marker) => {}
            _ => return false,
        }
    }
    true
}
