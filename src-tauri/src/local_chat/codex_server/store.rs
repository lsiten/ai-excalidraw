use super::super::error::{io, ChatError};
use serde::{Deserialize, Serialize};
use std::{
    collections::HashSet,
    path::{Path, PathBuf},
    sync::{Mutex, OnceLock},
};
static ACTIVE: OnceLock<Mutex<HashSet<PathBuf>>> = OnceLock::new();
#[derive(Serialize, Deserialize)]
struct Mapping {
    project: Option<PathBuf>,
    canvas: Option<String>,
    backend: String,
    thread: String,
}
pub(in crate::local_chat) struct Store {
    path: PathBuf,
    project: Option<PathBuf>,
    canvas: Option<String>,
    pub conversation: String,
    pub thread: Option<String>,
}
impl Store {
    pub fn open(
        data: &Path,
        conversation: Option<String>,
        project: Option<PathBuf>,
        canvas: Option<String>,
    ) -> Result<Self, ChatError> {
        let conversation = conversation.unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
        if conversation.is_empty()
            || conversation.len() > 128
            || !conversation
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
        {
            return Err(ChatError::InvalidRequest);
        }
        let project = project
            .map(std::fs::canonicalize)
            .transpose()
            .map_err(io("resolve project"))?;
        let dir = data.join("local-chat-sessions");
        std::fs::create_dir_all(&dir).map_err(io("create session store"))?;
        let path = dir.join(format!("{conversation}.json"));
        let mut active = ACTIVE
            .get_or_init(Default::default)
            .lock()
            .map_err(|_| ChatError::State)?;
        if active.contains(&path) {
            return Err(ChatError::Busy);
        }
        let thread = match std::fs::read(&path) {
            Ok(bytes) => {
                if bytes.len() > 16384 {
                    return Err(ChatError::Protocol);
                }
                let mapping: Mapping =
                    serde_json::from_slice(&bytes).map_err(|_| ChatError::Protocol)?;
                if mapping.project != project
                    || mapping.canvas != canvas
                    || mapping.backend != "codex-app-server"
                {
                    return Err(ChatError::CanvasContext);
                }
                Some(mapping.thread)
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => None,
            Err(e) => return Err(io("read session mapping")(e)),
        };
        active.insert(path.clone());
        Ok(Self {
            path,
            project,
            canvas,
            conversation,
            thread,
        })
    }
    pub fn save(&mut self, thread: &str) -> Result<(), ChatError> {
        use std::io::Write;
        let mapping = Mapping {
            project: self.project.clone(),
            canvas: self.canvas.clone(),
            backend: "codex-app-server".into(),
            thread: thread.into(),
        };
        let mut temp = tempfile::NamedTempFile::new_in(self.path.parent().ok_or(ChatError::State)?)
            .map_err(io("create session mapping"))?;
        serde_json::to_writer(&mut temp, &mapping).map_err(|_| ChatError::Protocol)?;
        temp.flush().map_err(io("flush session mapping"))?;
        temp.as_file()
            .sync_all()
            .map_err(io("sync session mapping"))?;
        temp.persist(&self.path)
            .map_err(|e| io("persist session mapping")(e.error))?;
        self.thread = Some(thread.into());
        Ok(())
    }
}
impl Drop for Store {
    fn drop(&mut self) {
        if let Ok(mut active) = ACTIVE.get_or_init(Default::default).lock() {
            active.remove(&self.path);
        }
    }
}
