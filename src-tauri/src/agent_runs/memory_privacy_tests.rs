use super::memory_tests::{prepare, reconcile, recover_scope, TARGET};
use super::security_tests::fixture;
use super::*;
use serde_json::json;
#[test]
fn unconfirmed_file_and_symlink_cannot_prove_absence() -> Result<()> {
    let (dir, h, mut seen) = fixture()?;
    let op = prepare(dir.path(), &h, &mut seen, None)?;
    fs::create_dir_all(dir.path().join(".agents/memory")).map_err(|_| "mkdir")?;
    fs::write(
        dir.path().join(TARGET),
        json!({"confirmed":false,"source":"other","content":"not confirmed"}).to_string(),
    )
    .map_err(|_| "write")?;
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
    #[cfg(unix)]
    {
        fs::remove_file(dir.path().join(TARGET)).map_err(|_| "remove")?;
        std::os::unix::fs::symlink(dir.path().join("missing"), dir.path().join(TARGET))
            .map_err(|_| "link")?;
        assert!(reconcile(
            dir.path(),
            &h,
            &mut seen,
            op,
            "absent",
            OperationStatus::Failed
        )
        .is_err());
    }
    Ok(())
}
