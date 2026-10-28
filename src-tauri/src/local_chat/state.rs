use super::{error::ChatError, process::ProcessControl};
use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};
use tokio::sync::watch;

type Requests = Arc<Mutex<Registry>>;

#[derive(Default)]
struct Registry {
    active: HashMap<String, ActiveRequest>,
    // Stop IPC can reach Rust before the async Start IPC is first polled.
    cancelled: HashMap<String, Instant>,
    stopping: bool,
}

struct ActiveRequest {
    cancel: watch::Sender<bool>,
    process: Arc<Mutex<Option<u32>>>,
}

#[derive(Default)]
pub struct LocalChatState {
    requests: Requests,
}

pub(super) struct RequestLease {
    requests: Requests,
    id: String,
    pub control: ProcessControl,
}

fn valid_id(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= 128
        && id
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || c == b'-' || c == b'_')
}

impl LocalChatState {
    pub(super) fn register(&self, id: String, prompt: &str) -> Result<RequestLease, ChatError> {
        if !valid_id(&id) || prompt.trim().is_empty() || prompt.len() > 2 * 1024 * 1024 {
            return Err(ChatError::InvalidRequest);
        }
        let mut registry = self.requests.lock().map_err(|_| ChatError::State)?;
        if registry.stopping || registry.cancelled.remove(&id).is_some() {
            return Err(ChatError::Cancelled);
        }
        if registry.active.contains_key(&id) {
            return Err(ChatError::Duplicate);
        }
        if registry.active.len() >= 4 {
            return Err(ChatError::Busy);
        }
        let (cancel, receiver) = watch::channel(false);
        let process = Arc::new(Mutex::new(None));
        registry.active.insert(
            id.clone(),
            ActiveRequest {
                cancel,
                process: Arc::clone(&process),
            },
        );
        Ok(RequestLease {
            requests: Arc::clone(&self.requests),
            id,
            control: ProcessControl {
                cancel: receiver,
                process,
                duration: super::process::TIME_LIMIT,
            },
        })
    }

    pub(super) fn cancel(&self, id: &str) -> Result<(), ChatError> {
        if !valid_id(id) {
            return Err(ChatError::InvalidRequest);
        }
        let mut registry = self.requests.lock().map_err(|_| ChatError::State)?;
        if let Some(active) = registry.active.get(id) {
            active.cancel.send_replace(true);
            super::process::kill_registered(&active.process);
        } else {
            registry
                .cancelled
                .retain(|_, at| at.elapsed() < Duration::from_secs(300));
            if registry.cancelled.len() >= 256 && !registry.cancelled.contains_key(id) {
                return Err(ChatError::Busy);
            }
            registry.cancelled.insert(id.to_owned(), Instant::now());
        }
        Ok(())
    }

    pub(crate) fn cancel_all(&self) {
        if let Ok(mut registry) = self.requests.lock() {
            registry.stopping = true;
            for active in registry.active.values() {
                active.cancel.send_replace(true);
                // Tauri may terminate before async futures are polled again.
                super::process::kill_registered(&active.process);
            }
        }
    }
}

impl Drop for RequestLease {
    fn drop(&mut self) {
        match self.requests.lock() {
            Ok(mut registry) => {
                registry.active.remove(&self.id);
            }
            Err(error) => eprintln!("Cannot release local chat request: {error}"),
        }
    }
}

pub(super) async fn cancelled(receiver: &mut watch::Receiver<bool>) {
    loop {
        if *receiver.borrow_and_update() {
            return;
        }
        if receiver.changed().await.is_err() {
            return;
        }
    }
}

pub(super) fn validate_canvas(
    project: Option<&str>,
    canvas: Option<&str>,
) -> Result<(), ChatError> {
    if canvas.is_some_and(|id| id.is_empty() || id.len() > 2100) {
        return Err(ChatError::CanvasContext);
    }
    if let (Some(project), Some(canvas)) = (project, canvas) {
        if canvas
            .strip_prefix(&format!("{project}:"))
            .is_none_or(str::is_empty)
        {
            return Err(ChatError::CanvasContext);
        }
    }
    Ok(())
}
