use super::*;
use serde_json::json;
pub(super) fn fixture() -> Result<(tempfile::TempDir, Header, HashSet<PathBuf>)> {
    let dir = tempfile::tempdir_in(fs::canonicalize(std::env::temp_dir()).map_err(|_| "tmp")?)
        .map_err(|_| "tmp")?;
    fs::write(
        dir.path().join("workspace.json"),
        json!({"projects":[{"id":"p","name":"p","directory":dir.path()}]}).to_string(),
    )
    .map_err(|_| "registry")?;
    let header = Header {
        provider_session_id: None,
        id: Uuid::new_v4(),
        conversation_id: "c".into(),
        parent_run_id: None,
        backend: Backend::Api,
        project_id: Some("p".into()),
        canvas_id: Some("p:canvas.excalidraw".into()),
        context_snapshot: r#"{"apiKey":"context-secret"}"#.into(),
        state: State::Created,
        created_at: 0,
        updated_at: 0,
        capabilities: vec![],
    };
    let mut seen = HashSet::new();
    execute(
        dir.path(),
        Request::Create {
            header: header.clone(),
        },
        &mut seen,
    )?;
    Ok((dir, header, seen))
}
#[test]
fn secrets_are_redacted_on_disk_and_active_runs_survive_list() -> Result<()> {
    let (dir, h, mut seen) = fixture()?;
    execute(
        dir.path(),
        Request::Append {
            run_id: h.id,
            project_id: h.project_id.clone(),
            event: Event::Tool {
                call_id: "call".into(),
                name: "test".into(),
                status: ToolStatus::Running,
                input: Some(r#"{"nested":{"Authorization":"tool-secret"}}"#.into()),
                output: Some("password=output-secret".into()),
            },
        },
        &mut seen,
    )?;
    execute(
        dir.path(),
        Request::List {
            project_id: h.project_id.clone(),
        },
        &mut seen,
    )?;
    let path = dir
        .path()
        .join(".ai-excalidraw/runs")
        .join(format!("{}.json", h.id));
    let bytes = fs::read_to_string(&path).map_err(|_| "read")?;
    for secret in ["context-secret", "tool-secret", "output-secret"] {
        assert!(!bytes.contains(secret))
    }
    assert_eq!(load(&path)?.header.state, State::Created);
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        assert_eq!(
            fs::metadata(path)
                .map_err(|_| "metadata")?
                .permissions()
                .mode()
                & 0o777,
            0o600
        );
    }
    Ok(())
}
#[test]
fn scope_and_artifact_escape_are_rejected() -> Result<()> {
    let (dir, mut h, mut seen) = fixture()?;
    h.canvas_id = Some("other:canvas.excalidraw".into());
    assert!(security::header(&h, Some("p")).is_err());
    h.canvas_id = Some("p:canvas.excalidraw".into());
    assert!(security::header(&h, None).is_err());
    assert!(execute(
        dir.path(),
        Request::Append {
            run_id: h.id,
            project_id: Some("p".into()),
            event: Event::Artifact {
                path: "../outside".into(),
                kind: "image".into(),
                verified: true
            }
        },
        &mut seen
    )
    .is_err());
    let path = dir
        .path()
        .join(".ai-excalidraw/runs")
        .join(format!("{}.json", h.id));
    let mut run = load(&path)?;
    run.header.project_id = Some("other".into());
    save(&path, &run)?;
    assert!(execute(
        dir.path(),
        Request::Read {
            run_id: h.id,
            project_id: Some("p".into())
        },
        &mut seen
    )
    .is_err());
    Ok(())
}
#[test]
fn pending_blocks_success_and_terminal_events_do_not_mutate() -> Result<()> {
    let (dir, h, mut seen) = fixture()?;
    let path = dir
        .path()
        .join(".ai-excalidraw/runs")
        .join(format!("{}.json", h.id));
    let mut run = load(&path)?;
    run.header.state = State::Validating;
    let op = Uuid::new_v4();
    run.operations.insert(
        op,
        Receipt {
            operation_id: op,
            status: OperationStatus::Unknown,
            target_ids: vec![],
            tool: None,
            fingerprint: None,
            base_revision: None,
            before_snapshot: None,
            intended_after_snapshot: None,
            result: None,
            after_revision: None,
            after_snapshot: None,
            proof: None,
        },
    );
    assert!(append(
        &mut run,
        Event::State {
            state: State::Succeeded,
            message: None
        }
    )
    .is_err());
    run.operations.get_mut(&op).ok_or("receipt")?.status = OperationStatus::Failed;
    append(
        &mut run,
        Event::State {
            state: State::Succeeded,
            message: None,
        },
    )?;
    save(&path, &run)?;
    let before = fs::read(&path).map_err(|_| "read")?;
    assert!(execute(
        dir.path(),
        Request::Append {
            run_id: h.id,
            project_id: h.project_id,
            event: Event::Session {
                session_id: "session".into(),
                backend: Backend::Api,
                resumed: true
            }
        },
        &mut seen
    )
    .is_err());
    assert_eq!(before, fs::read(path).map_err(|_| "read")?);
    Ok(())
}
#[test]
fn session_association_is_persisted() -> Result<()> {
    let (dir, h, mut seen) = fixture()?;
    let value = execute(
        dir.path(),
        Request::Append {
            run_id: h.id,
            project_id: h.project_id,
            event: Event::Session {
                session_id: "session-1".into(),
                backend: Backend::Api,
                resumed: true,
            },
        },
        &mut seen,
    )?;
    let run: Run = serde_json::from_value(value).map_err(|_| "decode")?;
    assert_eq!(run.header.provider_session_id.as_deref(), Some("session-1"));
    Ok(())
}

#[cfg(unix)]
#[test]
fn directory_lock_excludes_other_writers_and_read_rejects_links() -> Result<()> {
    let (dir, h, _) = fixture()?;
    let root = dir.path().join(".ai-excalidraw/runs");
    let guard = unix_io::lock(&root)?;
    let other = unix_io::directory(&root, false)?;
    assert!(
        rustix::fs::flock(&other, rustix::fs::FlockOperation::NonBlockingLockExclusive).is_err()
    );
    drop(guard);
    rustix::fs::flock(&other, rustix::fs::FlockOperation::NonBlockingLockExclusive)
        .map_err(|_| "lock")?;
    let link = dir.path().join("linked.json");
    std::os::unix::fs::symlink(root.join(format!("{}.json", h.id)), &link).map_err(|_| "link")?;
    assert!(unix_io::read(&link).is_err());
    Ok(())
}
