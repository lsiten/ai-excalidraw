use super::security_tests::fixture;
use super::*;
use crate::agent_project::{dispatch, Operation, Request as ProjectRequest, ResourceFile};
use serde_json::{json, Value};
const SKILL: &str =
    "---\nname: demo\ndescription: Demo skill\n---\nRead [guide](references/guide.md).";
fn intent() -> Value {
    json!({"kind":"skill","name":"demo","files":[
     {"path":".agents/skills/demo/SKILL.md","revision":"sha256:55d26d2f3647deee988b62d8785944b05e0b790c80e151a178f270ba7d794c69"},
     {"path":".agents/skills/demo/references/guide.md","revision":"sha256:83ca68be6227af2feb15f227485ed18aff8ecae99416a4bd6df3be1b5e8059b4"}
    ]})
}
fn install(data: &Path) -> Result<String> {
    let value = dispatch(
        data,
        ProjectRequest {
            project_id: "p".into(),
            operation: Operation::WriteSkill {
                name: "demo".into(),
                content: SKILL.into(),
                files: vec![ResourceFile {
                    path: "references/guide.md".into(),
                    content: "guide".into(),
                }],
                expected_revision: None,
                authorized: true,
            },
        },
    )?;
    value["revision"]
        .as_str()
        .map(str::to_owned)
        .ok_or("package revision".into())
}
fn prepare(
    data: &Path,
    h: &Header,
    seen: &mut HashSet<PathBuf>,
    base: Option<String>,
    intent: Value,
) -> Result<Uuid> {
    let op = Uuid::new_v4();
    execute(
        data,
        Request::PrepareOperation {
            run_id: h.id,
            project_id: h.project_id.clone(),
            operation_id: op,
            target_ids: vec![
                ".agents/skills/demo/SKILL.md".into(),
                ".agents/skills/demo/references/guide.md".into(),
            ],
            tool: Some("write_skill".into()),
            fingerprint: Some("skill-test".into()),
            base_revision: base,
            before_snapshot: None,
            intended_after_snapshot: Some(intent.to_string()),
        },
        seen,
    )?;
    seen.clear();
    execute(
        data,
        Request::List {
            project_id: h.project_id.clone(),
        },
        seen,
    )?;
    Ok(op)
}
fn reconcile(
    data: &Path,
    h: &Header,
    seen: &mut HashSet<PathBuf>,
    op: Uuid,
    current: &str,
    status: OperationStatus,
) -> Result<Run> {
    let result = execute(
        data,
        Request::ReconcileOperation {
            run_id: h.id,
            project_id: h.project_id.clone(),
            operation_id: op,
            current_revision: Some(current.into()),
            expected_after_revision: None,
            status,
            proof: Some("not authorization".into()),
        },
        seen,
    )?;
    serde_json::from_value(result).map_err(|_| "decode".into())
}
#[test]
fn skill_missing_ack_is_verified_from_real_package_without_replay() -> Result<()> {
    let (dir, h, mut seen) = fixture()?;
    let op = prepare(dir.path(), &h, &mut seen, None, intent())?;
    let revision = install(dir.path())?;
    let path = dir.path().join(".agents/skills/demo/references/guide.md");
    let before = fs::read(&path).map_err(|_| "read")?;
    let run = reconcile(
        dir.path(),
        &h,
        &mut seen,
        op,
        &revision,
        OperationStatus::Applied,
    )?;
    assert_eq!(run.operations[&op].status, OperationStatus::Applied);
    assert_eq!(
        run.operations[&op].after_revision.as_deref(),
        Some(revision.as_str())
    );
    assert_eq!(run.header.state, State::NeedsReconciliation);
    assert_eq!(before, fs::read(path).map_err(|_| "read")?);
    assert!(!fs::read_dir(dir.path().join(".agents/skills"))
        .map_err(|_| "list")?
        .any(|e| e.is_ok_and(|e| e.file_name().to_string_lossy().starts_with(".backup-"))));
    Ok(())
}
#[test]
fn skill_failed_requires_unchanged_base_or_proven_local_absence() -> Result<()> {
    let (dir, h, mut seen) = fixture()?;
    let op = prepare(dir.path(), &h, &mut seen, None, intent())?;
    assert!(reconcile(
        dir.path(),
        &h,
        &mut seen,
        op,
        "absent",
        OperationStatus::Applied
    )
    .is_err());
    let run = reconcile(
        dir.path(),
        &h,
        &mut seen,
        op,
        "absent",
        OperationStatus::Failed,
    )?;
    assert_eq!(run.operations[&op].status, OperationStatus::Failed);
    assert!(!dir.path().join(".agents/skills/demo").exists());
    let (dir, h, mut seen) = fixture()?;
    let revision = install(dir.path())?;
    let op = prepare(dir.path(), &h, &mut seen, Some(revision.clone()), intent())?;
    assert!(reconcile(
        dir.path(),
        &h,
        &mut seen,
        op,
        "absent",
        OperationStatus::Failed
    )
    .is_err());
    let run = reconcile(
        dir.path(),
        &h,
        &mut seen,
        op,
        &revision,
        OperationStatus::Failed,
    )?;
    assert_eq!(run.operations[&op].status, OperationStatus::Failed);
    Ok(())
}
#[test]
fn skill_revision_hash_scope_and_false_failure_are_rejected() -> Result<()> {
    let (dir, h, mut seen) = fixture()?;
    let mut wrong = intent();
    wrong["files"][1]["revision"] = json!(format!("sha256:{}", "0".repeat(64)));
    let op = prepare(dir.path(), &h, &mut seen, None, wrong)?;
    let revision = install(dir.path())?;
    assert!(reconcile(
        dir.path(),
        &h,
        &mut seen,
        op,
        "stale",
        OperationStatus::Applied
    )
    .is_err());
    assert!(reconcile(
        dir.path(),
        &h,
        &mut seen,
        op,
        &revision,
        OperationStatus::Applied
    )
    .is_err());
    assert!(reconcile(
        dir.path(),
        &h,
        &mut seen,
        op,
        &revision,
        OperationStatus::Failed
    )
    .is_err());
    let path = dir
        .path()
        .join(".ai-excalidraw/runs")
        .join(format!("{}.json", h.id));
    let run = load(&path)?;
    assert_eq!(run.operations[&op].status, OperationStatus::Unknown);
    let mut receipt = run.operations[&op].clone();
    let mut escape = intent();
    escape["files"][1]["path"] = json!(".agents/skills/demo/../outside");
    receipt.intended_after_snapshot = Some(escape.to_string());
    assert!(skill_reconciliation::reconcile(
        dir.path(),
        &h,
        &receipt,
        &revision,
        &OperationStatus::Applied
    )
    .is_err());
    let mut unscoped = h;
    unscoped.project_id = None;
    assert!(skill_reconciliation::reconcile(
        dir.path(),
        &unscoped,
        &receipt,
        &revision,
        &OperationStatus::Applied
    )
    .is_err());
    Ok(())
}
