use super::security_tests::fixture;
use super::*;
#[test]
fn unknown_crash_is_reconciled_from_real_canvas_without_after_revision() -> Result<()> {
    for (decision, content) in [
        (OperationStatus::Applied, r#"{"elements":[1]}"#),
        (OperationStatus::Failed, r#"{"elements":[]}"#),
    ] {
        let (dir, h, mut seen) = fixture()?;
        let op = Uuid::new_v4();
        execute(
            dir.path(),
            Request::PrepareOperation {
                run_id: h.id,
                project_id: h.project_id.clone(),
                operation_id: op,
                target_ids: vec![],
                tool: None,
                fingerprint: Some("f".into()),
                base_revision: None,
                before_snapshot: Some(r#"{"elements":[]}"#.into()),
                intended_after_snapshot: Some(r#"{"elements":[1]}"#.into()),
            },
            &mut seen,
        )?;
        fs::write(dir.path().join("canvas.excalidraw"), content).map_err(|_| "canvas")?;
        seen.clear();
        execute(
            dir.path(),
            Request::List {
                project_id: h.project_id.clone(),
            },
            &mut seen,
        )?;
        use std::hash::{Hash, Hasher};
        let mut hash = std::hash::DefaultHasher::new();
        content.as_bytes().hash(&mut hash);
        let request = |revision: String| Request::ReconcileOperation {
            run_id: h.id,
            project_id: h.project_id.clone(),
            operation_id: op,
            current_revision: Some(revision),
            expected_after_revision: None,
            status: decision.clone(),
            proof: Some("untrusted explanation".into()),
        };
        assert!(execute(dir.path(), request("wrong".into()), &mut seen).is_err());
        let value = execute(
            dir.path(),
            request(format!("{:016x}", hash.finish())),
            &mut seen,
        )?;
        let run: Run = serde_json::from_value(value).map_err(|_| "decode")?;
        assert_eq!(run.operations[&op].status, decision);
        assert_eq!(run.header.state, State::NeedsReconciliation);
    }
    Ok(())
}

fn receipt(command: serde_json::Value, before: serde_json::Value) -> Receipt {
    Receipt {
        operation_id: Uuid::new_v4(),
        status: OperationStatus::Unknown,
        target_ids: vec!["a".into()],
        tool: command["tool"].as_str().map(str::to_owned),
        fingerprint: None,
        base_revision: None,
        before_snapshot: Some(before.to_string()),
        intended_after_snapshot: Some(command.to_string()),
        result: None,
        after_revision: None,
        after_snapshot: None,
        proof: None,
    }
}
#[test]
fn update_projection_ignores_metadata_but_checks_every_requested_field() -> Result<()> {
    use serde_json::json;
    let receipt = receipt(
        json!({"tool":"update_elements","arguments":{"elements":[{"id":"a","x":20,"strokeColor":"red"}]}}),
        json!([{"id":"a","x":10,"strokeColor":"black"}]),
    );
    let mut actual = json!({"elements":[{"id":"a","x":20,"strokeColor":"red","version":42},{"id":"unrelated"}],"appState":{"zoom":2}});
    projection::verify(&actual, &receipt, &OperationStatus::Applied)?;
    actual["elements"][0]["strokeColor"] = json!("black");
    assert!(projection::verify(&actual, &receipt, &OperationStatus::Applied).is_err());
    actual["elements"][0]["x"] = json!(10);
    projection::verify(&actual, &receipt, &OperationStatus::Failed)?;
    actual["elements"][0]["x"] = json!(11);
    assert!(projection::verify(&actual, &receipt, &OperationStatus::Failed).is_err());
    Ok(())
}
#[test]
fn add_explicit_projection_and_delete_absence_are_required() -> Result<()> {
    use serde_json::json;
    let add = receipt(
        json!({"tool":"add_elements","arguments":{"elements":[{"id":"a","type":"rectangle","x":0,"y":0,"width":10,"height":20}]}}),
        json!([]),
    );
    let mut actual = json!({"elements":[{"id":"a","type":"rectangle","x":0,"y":0,"width":10,"height":20,"opacity":50,"fontFamily":8,"seed":99}]});
    projection::verify(&actual, &add, &OperationStatus::Applied)?;
    actual["elements"][0]["width"] = json!(11);
    assert!(projection::verify(&actual, &add, &OperationStatus::Applied).is_err());
    actual["elements"][0]["width"] = json!(10);
    actual["elements"][0]["isDeleted"] = json!(true);
    assert!(projection::verify(&actual, &add, &OperationStatus::Applied).is_err());
    actual["elements"][0]["isDeleted"] = json!(false);
    actual["elements"][0]["type"] = json!("ellipse");
    assert!(projection::verify(&actual, &add, &OperationStatus::Applied).is_err());
    actual["elements"][0]["type"] = json!("rectangle");
    let explicit_style = receipt(
        json!({"tool":"add_elements","arguments":{"elements":[{"id":"a","type":"rectangle","x":0,"y":0,"width":10,"height":20,"opacity":100}]}}),
        json!([]),
    );
    assert!(projection::verify(&actual, &explicit_style, &OperationStatus::Applied).is_err());
    let delete = receipt(
        json!({"tool":"delete_elements","arguments":{"ids":["a"]}}),
        json!([{"id":"a","x":0}]),
    );
    assert!(projection::verify(&actual, &delete, &OperationStatus::Applied).is_err());
    projection::verify(
        &json!({"elements":[{"id":"unrelated"}]}),
        &delete,
        &OperationStatus::Applied,
    )?;
    projection::verify(&json!({"elements":[]}), &add, &OperationStatus::Failed)?;
    Ok(())
}
#[test]
fn missing_before_fields_and_wrong_targets_cannot_prove_outcomes() {
    use serde_json::json;
    let mut receipt = receipt(
        json!({"tool":"update_elements","arguments":{"elements":[{"id":"a","opacity":50}]}}),
        json!([{"id":"a","x":0}]),
    );
    assert!(projection::verify(
        &json!({"elements":[{"id":"a","opacity":100}]}),
        &receipt,
        &OperationStatus::Failed
    )
    .is_err());
    receipt.target_ids = vec!["other".into()];
    assert!(projection::verify(
        &json!({"elements":[{"id":"a","opacity":50}]}),
        &receipt,
        &OperationStatus::Applied
    )
    .is_err());
}
#[test]
fn command_projection_reconciles_real_disk_with_version_guard() -> Result<()> {
    use serde_json::json;
    let (dir, h, mut seen) = fixture()?;
    let op = Uuid::new_v4();
    execute(
        dir.path(),
        Request::PrepareOperation {
            run_id: h.id,
            project_id: h.project_id.clone(),
            operation_id: op,
            target_ids: vec!["a".into()],
            tool: Some("update_elements".into()),
            fingerprint: None,
            base_revision: None,
            before_snapshot: Some(json!([{"id":"a","x":1}]).to_string()),
            intended_after_snapshot: Some(
                json!({"tool":"update_elements","arguments":{"elements":[{"id":"a","x":2}]}})
                    .to_string(),
            ),
        },
        &mut seen,
    )?;
    let content=json!({"type":"excalidraw","elements":[{"id":"a","x":2,"versionNonce":123}],"appState":{"theme":"dark"}}).to_string();
    fs::write(dir.path().join("canvas.excalidraw"), &content).map_err(|_| "canvas")?;
    seen.clear();
    execute(
        dir.path(),
        Request::List {
            project_id: h.project_id.clone(),
        },
        &mut seen,
    )?;
    use std::hash::{Hash, Hasher};
    let mut hash = std::hash::DefaultHasher::new();
    content.as_bytes().hash(&mut hash);
    let result = execute(
        dir.path(),
        Request::ReconcileOperation {
            run_id: h.id,
            project_id: h.project_id,
            current_revision: Some(format!("{:016x}", hash.finish())),
            expected_after_revision: None,
            operation_id: op,
            status: OperationStatus::Applied,
            proof: None,
        },
        &mut seen,
    )?;
    let run: Run = serde_json::from_value(result).map_err(|_| "decode")?;
    assert_eq!(run.operations[&op].status, OperationStatus::Applied);
    Ok(())
}
