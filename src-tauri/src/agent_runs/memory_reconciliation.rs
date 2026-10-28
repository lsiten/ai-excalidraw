use super::*;
use crate::agent_project::{dispatch, Operation, Request as ProjectRequest};
use serde::Deserialize;
use serde_json::{json, Value};

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Intended {
    kind: String,
    name: String,
    #[serde(rename = "contentRevision")]
    content_revision: String,
}
fn call(data: &Path, project: &str, operation: Operation) -> Result<Value> {
    dispatch(
        data,
        ProjectRequest {
            project_id: project.into(),
            operation,
        },
    )
}
fn entry(data: &Path, project: &str, path: &str) -> Result<Option<String>> {
    let manifest = call(data, project, Operation::Manifest)?;
    let memory = manifest["memory"]
        .as_array()
        .ok_or("invalid memory manifest")?;
    memory
        .iter()
        .find(|v| v["path"].as_str() == Some(path))
        .map(|v| {
            if v["confirmed"] != true {
                return Err("unconfirmed memory".into());
            }
            v["revision"]
                .as_str()
                .map(str::to_owned)
                .ok_or("invalid memory revision".into())
        })
        .transpose()
}
fn validate(receipt: &Receipt) -> Result<Intended> {
    let intent: Intended = serde_json::from_str(
        receipt
            .intended_after_snapshot
            .as_deref()
            .ok_or("memory intent required")?,
    )
    .map_err(|_| "invalid memory intent")?;
    if receipt.tool.as_deref() != Some("write_memory")
        || intent.kind != "memory"
        || intent.name.is_empty()
        || intent.name.len() > 64
        || intent.name.starts_with('-')
        || intent.name.ends_with('-')
        || intent.name.contains("--")
        || !intent
            .name
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
    {
        return Err("invalid memory identity".into());
    }
    let hash = intent
        .content_revision
        .strip_prefix("sha256:")
        .ok_or("SHA-256 required")?;
    if hash.len() != 64
        || !hash
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
    {
        return Err("invalid memory digest".into());
    }
    if receipt.target_ids != [format!(".agents/memory/{}.json", intent.name)] {
        return Err("memory target mismatch".into());
    }
    Ok(intent)
}
#[cfg(unix)]
fn absent(root: &Path, name: &str) -> Result<bool> {
    use rustix::fs::{openat, statat, AtFlags, Mode, OFlags};
    let mut directory = unix_io::directory(root, false)?;
    for component in [".agents", "memory"] {
        match openat(
            &directory,
            component,
            OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
            Mode::empty(),
        ) {
            Ok(fd) => directory = fs::File::from(fd),
            Err(rustix::io::Errno::NOENT) => return Ok(true),
            Err(_) => return Err("memory absence unproved".into()),
        }
    }
    match statat(
        &directory,
        format!("{name}.json"),
        AtFlags::SYMLINK_NOFOLLOW,
    ) {
        Ok(_) => Ok(false),
        Err(rustix::io::Errno::NOENT) => Ok(true),
        Err(_) => Err("memory absence unproved".into()),
    }
}
#[cfg(not(unix))]
fn absent(_: &Path, _: &str) -> Result<bool> {
    Err("secure memory verification unavailable".into())
}
pub(super) fn reconcile(
    data: &Path,
    header: &Header,
    receipt: &Receipt,
    current: &str,
    status: &OperationStatus,
) -> Result<String> {
    let project = header
        .project_id
        .as_deref()
        .ok_or("memory project required")?;
    let intended = validate(receipt)?;
    let path = format!(".agents/memory/{}.json", intended.name);
    let revision = entry(data, project, &path)?;
    if current == "absent" {
        let root = crate::workspace::project_root(data, project)?;
        if *status != OperationStatus::Failed
            || receipt.base_revision.is_some()
            || revision.is_some()
            || !absent(&root, &intended.name)?
        {
            return Err("memory absence unproved".into());
        }
        return Ok(json!({"kind":"memory","path":path,"absent":true}).to_string());
    }
    if revision.as_deref() != Some(current) {
        return Err("memory revision conflict".into());
    }
    let actual = call(data, project, Operation::Read { path: path.clone() })?;
    if actual["revision"].as_str() != Some(current)
        || actual["contentRevision"].as_str() != Some(current)
    {
        return Err("memory changed during reconciliation".into());
    }
    match status {
        OperationStatus::Applied => {
            if intended.content_revision != current {
                return Err("memory digest mismatch".into());
            }
        }
        OperationStatus::Failed => {
            if receipt.base_revision.as_deref() != Some(current) {
                return Err("memory before revision unproved".into());
            }
        }
        OperationStatus::Prepared | OperationStatus::Unknown => {
            return Err("settled decision required".into())
        }
    }
    if entry(data, project, &path)?.as_deref() != Some(current) {
        return Err("memory changed during reconciliation".into());
    }
    Ok(
        json!({"kind":"memory","path":path,"revision":current,"contentRevision":current})
            .to_string(),
    )
}
