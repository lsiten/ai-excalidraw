use super::*;
use serde_json::Value;
use std::path::Component;

fn credential(key: &str) -> bool {
    let key: String = key
        .chars()
        .filter(|c| c.is_ascii_alphanumeric())
        .flat_map(char::to_lowercase)
        .collect();
    matches!(
        key.as_str(),
        "authorization"
            | "proxyauthorization"
            | "cookie"
            | "setcookie"
            | "password"
            | "passwd"
            | "secret"
            | "clientsecret"
            | "token"
            | "accesstoken"
            | "refreshtoken"
            | "idtoken"
            | "apikey"
            | "privatekey"
    ) || key.ends_with("apikey")
        || key.ends_with("accesstoken")
}
pub(super) fn redact(value: &mut Value, depth: usize) -> Result<()> {
    if depth > 32 {
        return Err("payload nesting limit".into());
    }
    match value {
        Value::Object(map) => {
            for (key, value) in map {
                if credential(key) {
                    *value = Value::String("[REDACTED]".into())
                } else {
                    redact(value, depth + 1)?
                }
            }
        }
        Value::Array(items) => {
            for item in items {
                redact(item, depth + 1)?
            }
        }
        Value::String(text) => {
            if let Ok(mut nested) = serde_json::from_str::<Value>(text) {
                if nested.is_object() || nested.is_array() {
                    redact(&mut nested, depth + 1)?;
                    *text = serde_json::to_string(&nested).map_err(|_| "redaction")?;
                }
            } else if (text.contains(':') || text.contains('='))
                && text
                    .split(|c: char| !c.is_ascii_alphanumeric() && c != '_' && c != '-')
                    .any(credential)
            {
                *text = "[REDACTED]".into();
            }
        }
        Value::Null | Value::Bool(_) | Value::Number(_) => {}
    }
    Ok(())
}
pub(super) fn scoped(root: &Path, name: &str) -> Result<PathBuf> {
    let path = Path::new(name);
    if name.is_empty()
        || name.contains(['\\', ':'])
        || path
            .components()
            .any(|c| !matches!(c, Component::Normal(_)))
    {
        return Err("invalid scoped path".into());
    }
    let path = root.join(path);
    safe(&path)?;
    Ok(path)
}
pub(super) fn header(header: &Header, project: Option<&str>) -> Result<()> {
    if header.project_id.as_deref() != project {
        return Err("run scope mismatch".into());
    }
    if let Some(canvas) = &header.canvas_id {
        let (owner, path) = canvas.split_once(':').ok_or("invalid canvas id")?;
        if Some(owner) != project
            || path.is_empty()
            || Path::new(path)
                .extension()
                .is_none_or(|e| e != "excalidraw")
        {
            return Err("canvas scope mismatch".into());
        }
        scoped(Path::new("."), path)?;
    }
    Ok(())
}
pub(super) fn artifact(data: &Path, header: &Header, name: &str) -> Result<()> {
    let root = match &header.project_id {
        Some(id) => crate::workspace::project_root(data, id)?,
        None => data.to_owned(),
    };
    scoped(&root, name)?;
    Ok(())
}
pub(super) fn reconcile(
    data: &Path,
    header: &Header,
    receipt: &Receipt,
    revision: &str,
    status: &OperationStatus,
) -> Result<String> {
    if receipt.tool.as_deref() == Some("write_memory") {
        return memory_reconciliation::reconcile(data, header, receipt, revision, status);
    }
    if receipt.tool.as_deref() == Some("write_skill") {
        return skill_reconciliation::reconcile(data, header, receipt, revision, status);
    }
    let project = header
        .project_id
        .as_deref()
        .ok_or("canvas project required")?;
    let canvas = header.canvas_id.as_deref().ok_or("canvas required")?;
    let (owner, name) = canvas.split_once(':').ok_or("canvas id")?;
    if owner != project {
        return Err("canvas scope mismatch".into());
    }
    let path = scoped(&crate::workspace::project_root(data, project)?, name)?;
    let mut content = String::new();
    #[cfg(unix)]
    let file = unix_io::read(&path)?;
    #[cfg(not(unix))]
    let file = fs::File::open(path).map_err(|_| "canvas read")?;
    file.take(LIMIT + 1)
        .read_to_string(&mut content)
        .map_err(|_| "canvas read")?;
    if content.len() > LIMIT as usize {
        return Err("canvas limit".into());
    }
    use std::hash::{Hash, Hasher};
    let mut hasher = std::hash::DefaultHasher::new();
    content.as_bytes().hash(&mut hasher);
    if format!("{:016x}", hasher.finish()) != revision {
        return Err("canvas revision conflict".into());
    }
    let mut actual: Value = serde_json::from_str(&content).map_err(|_| "canvas JSON")?;
    projection::verify(&actual, receipt, status)?;
    redact(&mut actual, 0)?;
    serde_json::to_string(&actual).map_err(|_| "snapshot encoding".into())
}
