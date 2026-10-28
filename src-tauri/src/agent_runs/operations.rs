use super::*;
pub(super) fn apply(data: &Path, run: &mut Run, request: Request) -> Result<()> {
    match request {
        Request::Append { event, .. } => {
            if matches!(event, Event::Operation { .. }) {
                return Err("use receipt commands".into());
            }
            if let Event::Artifact { path, .. } = &event {
                security::artifact(data, &run.header, path)?;
            }
            append(run, event)?;
        }
        Request::PrepareOperation {
            operation_id,
            target_ids,
            intended_after_snapshot,
            tool,
            fingerprint,
            base_revision,
            before_snapshot,
            ..
        } => {
            if let Some(existing) = run.operations.get(&operation_id) {
                if existing.intended_after_snapshot != intended_after_snapshot
                    || existing.target_ids != target_ids
                    || existing.tool != tool
                    || existing.fingerprint != fingerprint
                    || existing.base_revision != base_revision
                    || existing.before_snapshot != before_snapshot
                {
                    return Err("operation conflict".into());
                }
            } else {
                if matches!(
                    run.header.state,
                    State::Interrupted | State::NeedsReconciliation | State::Cancelling
                ) {
                    return Err("run cannot execute operations".into());
                }
                let receipt = Receipt {
                    operation_id,
                    intended_after_snapshot,
                    status: OperationStatus::Prepared,
                    target_ids,
                    tool,
                    fingerprint,
                    base_revision,
                    before_snapshot,
                    result: None,
                    after_revision: None,
                    after_snapshot: None,
                    proof: None,
                };
                bounded(&serde_json::to_value(&receipt).map_err(|_| "receipt")?)?;
                append(
                    run,
                    Event::Operation {
                        operation_id,
                        status: receipt.status.clone(),
                        target_ids: receipt.target_ids.clone(),
                    },
                )?;
                run.operations.insert(operation_id, receipt);
            }
        }
        Request::SettleOperation {
            operation_id,
            status,
            result,
            after_revision,
            after_snapshot,
            ..
        } => {
            let old = run
                .operations
                .get(&operation_id)
                .ok_or("operation missing")?
                .clone();
            if old.status == status
                && (old.result != result
                    || old.after_revision != after_revision
                    || old.after_snapshot != after_snapshot)
            {
                return Err("settlement conflict".into());
            }
            if old.status != status {
                if status == OperationStatus::Prepared
                    || !matches!(old.status, OperationStatus::Prepared)
                {
                    return Err("settlement conflict".into());
                }
                append(
                    run,
                    Event::Operation {
                        operation_id,
                        status: status.clone(),
                        target_ids: old.target_ids.clone(),
                    },
                )?;
                run.operations.insert(
                    operation_id,
                    Receipt {
                        status,
                        result,
                        after_revision,
                        after_snapshot,
                        ..old
                    },
                );
                bounded(
                    &serde_json::to_value(&run.operations[&operation_id]).map_err(|_| "receipt")?,
                )?;
            }
        }
        Request::ReconcileOperation {
            operation_id,
            expected_after_revision,
            current_revision,
            status,
            proof,
            ..
        } => {
            let old = run
                .operations
                .get(&operation_id)
                .ok_or("operation missing")?
                .clone();
            let revision = current_revision
                .or(expected_after_revision)
                .ok_or("current revision required")?;
            let snapshot = security::reconcile(data, &run.header, &old, &revision, &status)?;
            if old.status == status && old.proof == proof {
            } else {
                if old.status != OperationStatus::Unknown {
                    return Err("reconciliation requires unknown".into());
                }
                let receipt = Receipt {
                    status: status.clone(),
                    proof,
                    after_revision: Some(revision),
                    after_snapshot: Some(snapshot),
                    ..old
                };
                bounded(&serde_json::to_value(&receipt).map_err(|_| "receipt")?)?;
                append(
                    run,
                    Event::Operation {
                        operation_id,
                        status,
                        target_ids: receipt.target_ids.clone(),
                    },
                )?;
                run.operations.insert(operation_id, receipt);
            }
        }
        Request::Read { .. } => {}
        Request::Create { .. } | Request::List { .. } => return Err("request".into()),
    }

    Ok(())
}
