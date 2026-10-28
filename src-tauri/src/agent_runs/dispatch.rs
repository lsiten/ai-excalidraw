use super::*;
pub fn execute(
    data: &Path,
    request: Request,
    seen: &mut HashSet<PathBuf>,
) -> Result<serde_json::Value> {
    let mut value = serde_json::to_value(&request).map_err(|_| "request")?;
    bounded(&value)?;
    security::redact(&mut value, 0)?;
    let request: Request = serde_json::from_value(value).map_err(|_| "request")?;
    let (project, id) = match &request {
        Request::Create { header } => (header.project_id.as_deref(), Some(header.id)),
        Request::List { project_id } => (project_id.as_deref(), None),
        Request::Read { project_id, run_id }
        | Request::Append {
            project_id, run_id, ..
        }
        | Request::PrepareOperation {
            project_id, run_id, ..
        }
        | Request::SettleOperation {
            project_id, run_id, ..
        }
        | Request::ReconcileOperation {
            project_id, run_id, ..
        } => (project_id.as_deref(), Some(*run_id)),
    };
    let project = project.map(str::to_owned);
    let dir = directory(data, project.as_deref())?;
    #[cfg(unix)]
    let _lock = unix_io::lock(&dir)?;
    #[cfg(not(unix))]
    let _lock = {
        let lock_path = dir.join(".lock");
        safe(&lock_path)?;
        let mut options = fs::OpenOptions::new();
        options.read(true).write(true).create(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        let lock = options.open(lock_path).map_err(|_| "store lock")?;
        lock.lock().map_err(|_| "store lock")?;

        lock
    };
    if let Request::List { .. } = request {
        let mut runs = Vec::new();
        for entry in fs::read_dir(&dir).map_err(|_| "list")? {
            let path = entry.map_err(|_| "entry")?.path();
            if path.extension().and_then(|s| s.to_str()) == Some("json") {
                runs.push(recover(&path, seen, project.as_deref())?);
            }
            if runs.len() > 10000 {
                return Err("run count limit".into());
            }
        }
        return serde_json::to_value(runs).map_err(|_| "encode".into());
    }
    let path = dir.join(format!("{}.json", id.ok_or("id")?));
    let mut run = match request {
        Request::Create { mut header } => {
            security::header(&header, project.as_deref())?;
            bounded(&serde_json::to_value(&header).map_err(|_| "header")?)?;
            serde_json::from_str::<serde_json::Value>(&header.context_snapshot)
                .map_err(|_| "context must be JSON")?;
            if header.state != State::Created {
                return Err("initial state".into());
            }
            if path.exists() {
                return Err("run already exists".into());
            }
            header.created_at = now()?;
            header.updated_at = header.created_at;
            let run = Run {
                header,
                events: Vec::new(),
                operations: Default::default(),
            };
            save(&path, &run)?;
            seen.insert(path.clone());
            run
        }
        other => {
            let mut run = recover(&path, seen, project.as_deref())?;
            operations::apply(data, &mut run, other)?;
            save(&path, &run)?;
            run
        }
    };
    serde_json::to_value(&mut run).map_err(|_| "encode".into())
}
