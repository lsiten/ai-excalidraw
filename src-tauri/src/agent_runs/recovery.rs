use super::*;
pub(super) fn append(run: &mut Run, event: Event) -> Result<()> {
    let mut value = serde_json::to_value(&event).map_err(|_| "event")?;
    bounded(&value)?;
    security::redact(&mut value, 0)?;
    let event: Event = serde_json::from_value(value).map_err(|_| "event")?;
    if run.header.state.terminal() {
        return Err("terminal run".into());
    }
    if let Event::Context {
        snapshot,
        capabilities,
    } = &event
    {
        if run.header.state != State::Preparing {
            return Err("context requires preparing state".into());
        }
        serde_json::from_str::<serde_json::Value>(snapshot).map_err(|_| "context must be JSON")?;
        run.header.context_snapshot = snapshot.clone();
        run.header.capabilities = capabilities.clone();
    }
    if let Event::Session {
        session_id,
        backend,
        ..
    } = &event
    {
        if backend != &run.header.backend {
            return Err("session backend mismatch".into());
        }
        run.header.provider_session_id = Some(session_id.clone());
    }
    if matches!(
        event,
        Event::State {
            state: State::Succeeded,
            ..
        }
    ) && run.operations.values().any(|r| {
        matches!(
            r.status,
            OperationStatus::Prepared | OperationStatus::Unknown
        )
    }) {
        return Err("unresolved operations".into());
    }

    if let Event::State { state, .. } = &event {
        if !run.header.state.allows(*state) {
            return Err("invalid lifecycle transition".into());
        }
        run.header.state = *state;
    } else if run.header.state.terminal() {
        return Err("terminal run".into());
    }
    let at = now()?.max(run.header.updated_at);
    run.header.updated_at = at;
    run.events.push(Envelope {
        run_id: run.header.id,
        seq: run.events.len() as u64 + 1,
        at,
        event,
    });
    Ok(())
}
pub(super) fn recover(
    path: &Path,
    seen: &mut HashSet<PathBuf>,
    project: Option<&str>,
) -> Result<Run> {
    let mut run = load(path)?;
    security::header(&run.header, project)?;
    if !seen.contains(path) {
        if !run.header.state.terminal()
            && !matches!(
                run.header.state,
                State::Interrupted | State::NeedsReconciliation
            )
        {
            let pending = run.operations.values().any(|r| {
                matches!(
                    r.status,
                    OperationStatus::Prepared | OperationStatus::Unknown
                )
            });
            let state = if pending {
                State::NeedsReconciliation
            } else {
                State::Interrupted
            };
            let ids: Vec<_> = run
                .operations
                .values()
                .filter(|r| r.status == OperationStatus::Prepared)
                .map(|r| r.operation_id)
                .collect();
            for id in ids {
                if let Some(r) = run.operations.get_mut(&id) {
                    r.status = OperationStatus::Unknown;
                    let event = Event::Operation {
                        operation_id: id,
                        status: r.status.clone(),
                        target_ids: r.target_ids.clone(),
                    };
                    append(&mut run, event)?;
                }
            }
            append(
                &mut run,
                Event::State {
                    state,
                    message: None,
                },
            )?;
            save(path, &run)?;
        }
        seen.insert(path.to_owned());
    }
    Ok(run)
}
