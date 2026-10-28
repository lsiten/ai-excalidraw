use super::security_tests::fixture;
use super::*;
use crate::agent_project::{dispatch, Operation, Request as ProjectRequest};
use serde_json::json;
const CONTENT: &str = "偏好：简洁回答。\nQuote: \"yes\"\t路径 /画布";
const DIGEST: &str = "sha256:51d46d0fe21182600995cf13b424d38b6e97e28d4e0f5e108e98290d77060f6d";
pub(super) const TARGET: &str = ".agents/memory/preference.json";
fn install(data: &Path, content: &str, expected: Option<String>) -> Result<String> {
    let value = dispatch(
        data,
        ProjectRequest {
            project_id: "p".into(),
            operation: Operation::WriteMemory {
                name: "preference".into(),
                content: content.into(),
                expected_revision: expected,
                authorized: true,
                confirmed: true,
            },
        },
    )?;
    value["revision"]
        .as_str()
        .map(str::to_owned)
        .ok_or("revision".into())
}
pub(super) fn prepare(
    data: &Path,
    h: &Header,
    seen: &mut HashSet<PathBuf>,
    base: Option<String>,
) -> Result<Uuid> {
    let id = Uuid::new_v4();
    execute(
        data,
        Request::PrepareOperation {
            run_id: h.id,
            project_id: h.project_id.clone(),
            operation_id: id,
            tool: Some("write_memory".into()),
            fingerprint: None,
            base_revision: base,
            before_snapshot: None,
            intended_after_snapshot: Some(
                json!({"kind":"memory","name":"preference","contentRevision":DIGEST}).to_string(),
            ),
            target_ids: vec![TARGET.into()],
        },
        seen,
    )?;
    Ok(id)
}
pub(super) fn reconcile(
    data: &Path,
    h: &Header,
    seen: &mut HashSet<PathBuf>,
    op: Uuid,
    revision: &str,
    status: OperationStatus,
) -> Result<Run> {
    let value = execute(
        data,
        Request::ReconcileOperation {
            run_id: h.id,
            project_id: h.project_id.clone(),
            operation_id: op,
            current_revision: Some(revision.into()),
            expected_after_revision: None,
            status,
            proof: Some("untrusted".into()),
        },
        seen,
    )?;
    serde_json::from_value(value).map_err(|_| "decode".into())
}
pub(super) fn recover_scope(data: &Path, h: &Header, seen: &mut HashSet<PathBuf>) -> Result<()> {
    seen.clear();
    execute(
        data,
        Request::List {
            project_id: h.project_id.clone(),
        },
        seen,
    )?;
    Ok(())
}
#[test]
fn confirmed_memory_ack_loss_reconciles_native_file_with_typescript_digest() -> Result<()> {
    let (dir, h, mut seen) = fixture()?;
    let op = prepare(dir.path(), &h, &mut seen, None)?;
    let revision = install(dir.path(), CONTENT, None)?;
    assert_eq!(revision, DIGEST);
    recover_scope(dir.path(), &h, &mut seen)?;
    let path = dir.path().join(TARGET);
    let before = fs::read(&path).map_err(|_| "read")?;
    let run = reconcile(
        dir.path(),
        &h,
        &mut seen,
        op,
        DIGEST,
        OperationStatus::Applied,
    )?;
    assert_eq!(run.operations[&op].status, OperationStatus::Applied);
    assert_eq!(run.header.state, State::NeedsReconciliation);
    assert_eq!(before, fs::read(path).map_err(|_| "read")?);
    let record = fs::read_to_string(
        dir.path()
            .join(".ai-excalidraw/runs")
            .join(format!("{}.json", h.id)),
    )
    .map_err(|_| "read")?;
    assert!(!record.contains("偏好"));
    assert!(!record.contains("explicit-user-command"));
    assert!(!fs::read_dir(dir.path().join(".agents/memory"))
        .map_err(|_| "list")?
        .any(|e| e.is_ok_and(|e| e.file_name().to_string_lossy().starts_with(".backup-"))));
    Ok(())
}
#[test]
fn memory_failed_requires_original_revision_or_independently_proved_absence() -> Result<()> {
    let (dir, h, mut seen) = fixture()?;
    let op = prepare(dir.path(), &h, &mut seen, None)?;
    recover_scope(dir.path(), &h, &mut seen)?;
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
    assert!(!dir.path().join(TARGET).exists());
    let (dir, h, mut seen) = fixture()?;
    let base = install(dir.path(), "Previous confirmed preference", None)?;
    let op = prepare(dir.path(), &h, &mut seen, Some(base.clone()))?;
    recover_scope(dir.path(), &h, &mut seen)?;
    assert!(reconcile(
        dir.path(),
        &h,
        &mut seen,
        op,
        "absent",
        OperationStatus::Failed
    )
    .is_err());
    assert!(reconcile(
        dir.path(),
        &h,
        &mut seen,
        op,
        &base,
        OperationStatus::Applied
    )
    .is_err());
    let run = reconcile(
        dir.path(),
        &h,
        &mut seen,
        op,
        &base,
        OperationStatus::Failed,
    )?;
    assert_eq!(run.operations[&op].status, OperationStatus::Failed);
    Ok(())
}
#[test]
fn memory_rejects_stale_revision_wrong_digest_scope_and_false_failure() -> Result<()> {
    let (dir, h, mut seen) = fixture()?;
    let op = prepare(dir.path(), &h, &mut seen, None)?;
    let actual = install(dir.path(), "Different confirmed content", None)?;
    recover_scope(dir.path(), &h, &mut seen)?;
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
        &actual,
        OperationStatus::Applied
    )
    .is_err());
    assert!(reconcile(
        dir.path(),
        &h,
        &mut seen,
        op,
        &actual,
        OperationStatus::Failed
    )
    .is_err());
    let run = load(
        &dir.path()
            .join(".ai-excalidraw/runs")
            .join(format!("{}.json", h.id)),
    )?;
    assert_eq!(run.operations[&op].status, OperationStatus::Unknown);
    let mut receipt = run.operations[&op].clone();
    receipt.target_ids = vec![".agents/memory/../outside".into()];
    assert!(memory_reconciliation::reconcile(
        dir.path(),
        &h,
        &receipt,
        &actual,
        &OperationStatus::Applied
    )
    .is_err());
    let mut no_project = h;
    no_project.project_id = None;
    assert!(memory_reconciliation::reconcile(
        dir.path(),
        &no_project,
        &receipt,
        &actual,
        &OperationStatus::Applied
    )
    .is_err());
    Ok(())
}
