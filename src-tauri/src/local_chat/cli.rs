use super::{
    error::{io, ChatError},
    project::ProjectResources,
};
use crate::integrations::{Context, ToolId};
use std::{
    path::{Path, PathBuf},
    process::Stdio,
};
use tempfile::TempDir;
use tokio::process::Command;

pub(super) struct Invocation {
    pub command: Command,
    pub directory: TempDir,
    pub tool: ToolId,
    pub project_root: Option<PathBuf>,
}

pub(super) fn prepare(
    context: &Context,
    tool: ToolId,
    project_root: Option<PathBuf>,
) -> Result<Invocation, ChatError> {
    prepare_backend(context, tool, project_root, false)
}

pub(super) fn prepare_backend(
    context: &Context,
    tool: ToolId,
    project_root: Option<PathBuf>,
    native: bool,
) -> Result<Invocation, ChatError> {
    let binary = context
        .cli_binary(tool)
        .ok_or(ChatError::Missing(tool.name()))?;
    let directory = tempfile::Builder::new()
        .prefix("ai-excalidraw-chat-")
        .tempdir()
        .map_err(io("创建本地对话运行目录"))?;
    let cwd = project_root.as_deref().unwrap_or(directory.path());
    let mut command = if native {
        let mut server = Command::new(&binary);
        server
            .args([
                "app-server",
                "-c",
                "sandbox_mode=\"read-only\"",
                "-c",
                "approval_policy=\"never\"",
            ])
            .current_dir(cwd)
            .env("PWD", cwd)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true);
        server
    } else {
        command(tool, &binary, cwd)
    };
    if let Some(root) = &project_root {
        let resources = ProjectResources::load(root)?;
        resources.validate_globals(context, tool)?;
        resources.apply(tool, &mut command, directory.path())?;
    }
    if let Ok(path) = std::env::join_paths(context.cli_directories()) {
        command.env("PATH", path);
    }
    Ok(Invocation {
        command,
        directory,
        tool,
        project_root,
    })
}

pub(super) fn command(tool: ToolId, binary: &Path, directory: &Path) -> Command {
    let mut command = Command::new(binary);
    // Local --help verified 2026-10-27; Cursor parameters and stdin also match Multica.
    // Each native CLI owns its tool loop. No permission-bypass or auto-approval flags.
    match tool {
        ToolId::Codex => {
            command.args([
                "exec",
                "--json",
                "--sandbox",
                "read-only",
                "--skip-git-repo-check",
                "--ephemeral",
                "--color",
                "never",
                "-c",
                "approval_policy=\"never\"",
                "-",
            ]);
        }
        ToolId::Claude => {
            command.args([
                "--print",
                "--verbose",
                "--output-format",
                "stream-json",
                "--include-partial-messages",
                "--no-session-persistence",
                "--permission-mode",
                "dontAsk",
            ]);
        }
        ToolId::Cursor => {
            command
                .args([
                    "--print",
                    "--output-format",
                    "stream-json",
                    "--stream-partial-output",
                    "--workspace",
                ])
                .arg(directory);
        }
        ToolId::Pi => {
            command.args(["--print", "--mode", "json", "--no-session"]);
        }
    }
    command
        .current_dir(directory)
        .env("PWD", directory)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true)
        .env("NO_COLOR", "1")
        .env("TERM", "dumb");
    command
}
