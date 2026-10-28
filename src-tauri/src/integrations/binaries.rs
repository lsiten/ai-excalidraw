use super::{Context, ToolId};
use std::path::{Path, PathBuf};

impl Context {
    /// Search real CLI installations even when a GUI launch has a minimal PATH.
    pub(crate) fn cli_directories(&self) -> Vec<PathBuf> {
        let mut paths: Vec<_> = self
            .search
            .iter()
            .filter(|p| p.is_absolute())
            .cloned()
            .collect();
        paths.extend([
            self.home.join(".local/bin"),
            self.home.join("bin"),
            self.home.join(".volta/bin"),
            self.home.join(".npm-global/bin"),
            self.home.join(".npm/bin"),
            self.home.join(".bun/bin"),
            self.home.join(".asdf/shims"),
            self.home.join(".local/share/mise/shims"),
            self.home.join(".nvm/current/bin"),
            PathBuf::from("/opt/homebrew/bin"),
            PathBuf::from("/usr/local/bin"),
            PathBuf::from("/usr/bin"),
            PathBuf::from("/bin"),
            self.codex.join("bin"),
            self.home.join(".claude/local"),
            self.home.join(".pi/agent/bin"),
        ]);
        for root in [
            self.home.join(".nvm/versions/node"),
            self.home.join(".local/share/fnm/node-versions"),
            self.home.join(".local/share/mise/installs/node"),
        ] {
            if let Ok(entries) = std::fs::read_dir(root) {
                let mut versions: Vec<_> =
                    entries.filter_map(Result::ok).map(|e| e.path()).collect();
                versions.sort();
                for version in versions.into_iter().rev() {
                    paths.push(version.join("bin"));
                    paths.push(version.join("installation/bin"));
                }
            }
        }
        paths.extend(
            self.applications
                .iter()
                .map(|base| base.join("Codex.app/Contents/Resources")),
        );
        paths
    }

    pub(crate) fn cli_binary(&self, id: ToolId) -> Option<PathBuf> {
        let names: &[&str] = match id {
            ToolId::Codex => &["codex"],
            ToolId::Claude => &["claude"],
            // The editor launcher named `cursor` cannot execute headless chat.
            ToolId::Cursor => &["cursor-agent", "agent"],
            ToolId::Pi => &["pi"],
        };
        let paths = self.cli_directories();
        for name in names {
            for directory in &paths {
                let candidate = directory.join(name);
                if executable(&candidate) && (*name != "agent" || cursor_agent(&candidate)) {
                    return Some(candidate);
                }
                #[cfg(windows)]
                {
                    let candidate = directory.join(format!("{name}.exe"));
                    if executable(&candidate) && (*name != "agent" || cursor_agent(&candidate)) {
                        return Some(candidate);
                    }
                }
            }
        }
        None
    }
}

fn cursor_agent(path: &Path) -> bool {
    // `agent` is ambiguous: accept the Cursor installer's symlink/installation path.
    path.canonicalize()
        .is_ok_and(|p| p.to_string_lossy().to_ascii_lowercase().contains("cursor"))
}

fn executable(path: &Path) -> bool {
    path.metadata().is_ok_and(|metadata| {
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            metadata.is_file() && metadata.permissions().mode() & 0o111 != 0
        }
        #[cfg(not(unix))]
        {
            metadata.is_file()
        }
    })
}
