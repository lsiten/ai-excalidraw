use super::*;
#[test]
fn lifecycle_rejects_success_without_validation() {
    assert!(!State::Running.allows(State::Succeeded));
    assert!(State::Validating.allows(State::Succeeded));
    assert!(!State::Succeeded.allows(State::Running));
}
#[test]
fn journal_and_receipts_survive_restart() -> Result<()> {
    let dir =
        tempfile::tempdir_in(std::fs::canonicalize(std::env::temp_dir()).map_err(|_| "temp root")?)
            .map_err(|_| "temp")?;
    let mut seen = HashSet::new();
    let id = Uuid::new_v4();
    let header = Header {
        provider_session_id: None,
        id,
        conversation_id: "conversation".into(),
        parent_run_id: None,
        backend: Backend::Api,
        project_id: None,
        canvas_id: None,
        context_snapshot: "{}".into(),
        state: State::Created,
        created_at: 0,
        updated_at: 0,
        capabilities: vec![],
    };
    execute(dir.path(), Request::Create { header }, &mut seen)?;
    let op = Uuid::new_v4();
    for _ in 0..2 {
        execute(
            dir.path(),
            Request::PrepareOperation {
                intended_after_snapshot: None,
                run_id: id,
                project_id: None,
                operation_id: op,
                target_ids: vec!["a".into()],
                tool: None,
                fingerprint: None,
                base_revision: None,
                before_snapshot: None,
            },
            &mut seen,
        )?;
    }
    let path = dir.path().join("runs").join(format!("{id}.json"));
    assert_eq!(load(&path)?.events.len(), 1);
    let recovered = recover(&path, &mut HashSet::new(), None)?;
    assert_eq!(recovered.header.state, State::NeedsReconciliation);
    assert_eq!(recovered.operations[&op].status, OperationStatus::Unknown);
    assert_eq!(recovered.events.len(), 3);
    Ok(())
}
#[cfg(unix)]
#[test]
fn symlink_directory_is_rejected() -> Result<()> {
    let dir =
        tempfile::tempdir_in(std::fs::canonicalize(std::env::temp_dir()).map_err(|_| "temp root")?)
            .map_err(|_| "temp")?;
    let target =
        tempfile::tempdir_in(std::fs::canonicalize(std::env::temp_dir()).map_err(|_| "temp root")?)
            .map_err(|_| "temp")?;
    std::os::unix::fs::symlink(target.path(), dir.path().join("runs")).map_err(|_| "link")?;
    assert!(directory(dir.path(), None).is_err());
    Ok(())
}

#[test]
fn unknown_project_cannot_fall_back_to_global_storage() -> Result<()> {
    let dir =
        tempfile::tempdir_in(std::fs::canonicalize(std::env::temp_dir()).map_err(|_| "temp root")?)
            .map_err(|_| "temp")?;
    assert!(directory(dir.path(), Some("missing-project")).is_err());
    assert!(!dir.path().join("runs").exists());
    Ok(())
}
#[test]
fn replay_rejects_nonmonotonic_sequence() -> Result<()> {
    let dir =
        tempfile::tempdir_in(std::fs::canonicalize(std::env::temp_dir()).map_err(|_| "temp root")?)
            .map_err(|_| "temp")?;
    let id = Uuid::new_v4();
    let mut run = Run {
        header: Header {
            provider_session_id: None,
            id,
            conversation_id: "c".into(),
            parent_run_id: None,
            backend: Backend::Api,
            project_id: None,
            canvas_id: None,
            context_snapshot: "{}".into(),
            state: State::Created,
            created_at: 0,
            updated_at: 0,
            capabilities: vec![],
        },
        events: vec![],
        operations: Default::default(),
    };
    append(
        &mut run,
        Event::State {
            state: State::Preparing,
            message: None,
        },
    )?;
    run.events[0].seq = 2;
    let path = dir.path().join(format!("{id}.json"));
    save(&path, &run)?;
    assert!(load(&path).is_err());
    Ok(())
}

#[test]
fn receipt_result_replay_and_conflict_are_durable() -> Result<()> {
    let temp = tempfile::tempdir_in(fs::canonicalize(std::env::temp_dir()).map_err(|_| "temp")?)
        .map_err(|_| "temp")?;
    let mut seen = HashSet::new();
    let id = Uuid::new_v4();
    let op = Uuid::new_v4();
    let header = Header {
        provider_session_id: None,
        id,
        conversation_id: "c".into(),
        parent_run_id: None,
        backend: Backend::Api,
        project_id: None,
        canvas_id: None,
        context_snapshot: "{}".into(),
        state: State::Created,
        created_at: 0,
        updated_at: 0,
        capabilities: vec![],
    };
    execute(temp.path(), Request::Create { header }, &mut seen)?;
    let prepare = || Request::PrepareOperation {
        intended_after_snapshot: None,
        run_id: id,
        project_id: None,
        operation_id: op,
        target_ids: vec![],
        tool: Some("edit".into()),
        fingerprint: Some("hash".into()),
        base_revision: Some("r1".into()),
        before_snapshot: Some("{}".into()),
    };
    execute(temp.path(), prepare(), &mut seen)?;
    let settle = |result: &str| Request::SettleOperation {
        run_id: id,
        project_id: None,
        operation_id: op,
        status: OperationStatus::Failed,
        result: Some(result.into()),
        after_revision: Some("r1".into()),
        after_snapshot: Some("{}".into()),
    };
    execute(temp.path(), settle("error"), &mut seen)?;
    execute(temp.path(), settle("error"), &mut seen)?;
    assert!(execute(temp.path(), settle("changed"), &mut seen).is_err());
    let value = execute(temp.path(), prepare(), &mut seen)?;
    let run: Run = serde_json::from_value(value).map_err(|_| "decode")?;
    assert_eq!(run.events.len(), 2);
    assert_eq!(run.operations[&op].result.as_deref(), Some("error"));
    assert_eq!(run.header.state, State::Created);
    assert!(State::Validating.allows(State::Repairing));
    assert!(State::Repairing.allows(State::Running));
    Ok(())
}
