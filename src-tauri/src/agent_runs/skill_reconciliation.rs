use super::*;
use crate::agent_project::{dispatch, Operation, Request as ProjectRequest};
use serde::Deserialize;
use serde_json::{json, Value};
use std::collections::BTreeSet;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Intended {
    kind: String,
    name: String,
    files: Vec<ExpectedFile>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ExpectedFile {
    path: String,
    revision: String,
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
fn manifest_entry(data: &Path, project: &str, name: &str) -> Result<Option<Value>> {
    let manifest = call(data, project, Operation::Manifest)?;
    let skills = manifest
        .get("skills")
        .and_then(Value::as_array)
        .ok_or("invalid manifest")?;
    Ok(skills
        .iter()
        .find(|s| s["name"].as_str() == Some(name) && s["source"] == "project")
        .cloned())
}
fn validate(receipt: &Receipt) -> Result<Intended> {
    if receipt.tool.as_deref() != Some("write_skill") {
        return Err("skill receipt tool mismatch".into());
    }
    let intended: Intended = serde_json::from_str(
        receipt
            .intended_after_snapshot
            .as_deref()
            .ok_or("skill intent required")?,
    )
    .map_err(|_| "invalid skill intent")?;
    if intended.kind != "skill"
        || intended.name.is_empty()
        || intended.name.len() > 64
        || intended.name.starts_with('-')
        || intended.name.ends_with('-')
        || intended.name.contains("--")
        || !intended
            .name
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
    {
        return Err("invalid skill identity".into());
    }
    let prefix = format!(".agents/skills/{}/", intended.name);
    let mut paths = BTreeSet::new();
    for file in &intended.files {
        let relative = file
            .path
            .strip_prefix(&prefix)
            .ok_or("skill file scope mismatch")?;
        if relative.is_empty()
            || relative
                .split('/')
                .any(|part| part.is_empty() || part.starts_with('.') || part.contains(['\\', ':']))
            || !paths.insert(file.path.as_str())
        {
            return Err("invalid skill file path".into());
        }
        let hash = file
            .revision
            .strip_prefix("sha256:")
            .ok_or("SHA-256 revision required")?;
        if hash.len() != 64 || !hash.bytes().all(|b| b.is_ascii_hexdigit()) {
            return Err("invalid SHA-256 revision".into());
        }
    }
    if intended.files.is_empty()
        || intended.files.len() > 64
        || !paths.contains(format!("{prefix}SKILL.md").as_str())
        || paths != receipt.target_ids.iter().map(String::as_str).collect()
        || paths.len() != receipt.target_ids.len()
    {
        return Err("skill targets mismatch".into());
    }
    Ok(intended)
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
        .ok_or("skill project required")?;
    let intended = validate(receipt)?;
    let entry = manifest_entry(data, project, &intended.name)?;
    if current == "absent" {
        if *status != OperationStatus::Failed || receipt.base_revision.is_some() || entry.is_some()
        {
            return Err("skill absence unproved".into());
        }
        return Ok(json!({"kind":"skill","name":intended.name,"absent":true}).to_string());
    }
    let entry = entry.ok_or("skill package missing")?;
    if entry["revision"].as_str() != Some(current) {
        return Err("skill package revision conflict".into());
    }
    let skill_path = format!(".agents/skills/{}/SKILL.md", intended.name);
    let package = call(data, project, Operation::Read { path: skill_path })?;
    if package["revision"].as_str() != Some(current) {
        return Err("skill changed during reconciliation".into());
    }
    match status {
        OperationStatus::Applied => {
            let catalog = entry["files"]
                .as_array()
                .ok_or("skill file manifest missing")?;
            for file in &intended.files {
                if !catalog.iter().any(|v| {
                    v["path"].as_str() == Some(&file.path)
                        && v["revision"].as_str() == Some(&file.revision)
                }) {
                    return Err("skill manifest hash mismatch".into());
                }
                let actual = call(
                    data,
                    project,
                    Operation::Read {
                        path: file.path.clone(),
                    },
                )?;
                if actual["contentRevision"].as_str() != Some(&file.revision) {
                    return Err("skill content hash mismatch".into());
                }
            }
        }
        OperationStatus::Failed => {
            if receipt.base_revision.as_deref() != Some(current) {
                return Err("skill before revision unproved".into());
            }
        }
        OperationStatus::Prepared | OperationStatus::Unknown => {
            return Err("settled decision required".into())
        }
    }
    if manifest_entry(data, project, &intended.name)?.as_ref() != Some(&entry) {
        return Err("skill changed during reconciliation".into());
    }
    Ok(
        json!({"kind":"skill","name":intended.name,"revision":current,"files":entry["files"]})
            .to_string(),
    )
}
