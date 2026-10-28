use super::*;
use serde::Deserialize;
use serde_json::{Map, Value};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Deserialize)]
#[serde(
    tag = "tool",
    content = "arguments",
    rename_all = "snake_case",
    deny_unknown_fields
)]
pub(super) enum Command {
    #[serde(rename = "add_elements")]
    Add { elements: Vec<Map<String, Value>> },
    #[serde(rename = "update_elements")]
    Update { elements: Vec<Map<String, Value>> },
    #[serde(rename = "delete_elements")]
    Delete { ids: Vec<String> },
}
fn id(element: &Map<String, Value>) -> Result<&str> {
    element
        .get("id")
        .and_then(Value::as_str)
        .filter(|id| !id.is_empty())
        .ok_or("element id required".into())
}
fn index(elements: &[Value]) -> Result<BTreeMap<&str, &Map<String, Value>>> {
    let mut result = BTreeMap::new();
    for element in elements {
        let element = element.as_object().ok_or("element object required")?;
        if element.get("isDeleted") == Some(&Value::Bool(true)) {
            continue;
        }
        if result.insert(id(element)?, element).is_some() {
            return Err("duplicate canvas id".into());
        }
    }
    Ok(result)
}
fn equal_fields(actual: &Map<String, Value>, expected: &Map<String, Value>) -> bool {
    expected.iter().all(|(key, value)| {
        actual
            .get(key)
            .is_some_and(|actual| equal_value(actual, value))
    })
}

fn equal_value(actual: &Value, expected: &Value) -> bool {
    match (actual, expected) {
        (Value::Number(a), Value::Number(b)) => a.as_f64() == b.as_f64(),
        (Value::Array(a), Value::Array(b)) => {
            a.len() == b.len() && a.iter().zip(b).all(|(a, b)| equal_value(a, b))
        }
        (Value::Object(a), Value::Object(b)) => a.len() == b.len() && equal_fields(a, b),
        _ => actual == expected,
    }
}
fn explicit_add(patch: &Map<String, Value>) -> Result<Map<String, Value>> {
    for field in ["type", "x", "y", "width", "height"] {
        if !patch.contains_key(field) {
            return Err("incomplete add command".into());
        }
    }
    if !matches!(
        patch.get("type").and_then(Value::as_str),
        Some("rectangle" | "ellipse" | "diamond" | "text" | "arrow" | "line")
    ) {
        return Err("unsupported element type".into());
    }
    Ok(patch.clone())
}
fn patches(command: &Command) -> Result<Vec<Map<String, Value>>> {
    match command {
        Command::Add { elements } => elements.iter().map(explicit_add).collect(),
        Command::Update { elements } => elements
            .iter()
            .map(|patch| {
                if patch.len() < 2 {
                    return Err("empty update projection".into());
                }
                let mut patch = patch.clone();
                if let Some(text) = patch.get("text").cloned() {
                    patch.insert("originalText".into(), text);
                }
                Ok(patch)
            })
            .collect(),
        Command::Delete { .. } => Ok(Vec::new()),
    }
}
pub(super) fn verify(actual: &Value, receipt: &Receipt, status: &OperationStatus) -> Result<()> {
    let intended = receipt
        .intended_after_snapshot
        .as_deref()
        .ok_or("intended snapshot required")?;
    let intended: Value = serde_json::from_str(intended).map_err(|_| "snapshot JSON")?;
    // Existing full-document receipts retain their exact-match verification contract.
    if intended.get("tool").is_none() {
        let expected = match status {
            OperationStatus::Applied => Some(intended),
            OperationStatus::Failed => receipt
                .before_snapshot
                .as_deref()
                .and_then(|s| serde_json::from_str(s).ok()),
            OperationStatus::Prepared | OperationStatus::Unknown => None,
        };
        return if expected.as_ref() == Some(actual) {
            Ok(())
        } else {
            Err("canvas snapshot conflict".into())
        };
    }
    let command: Command =
        serde_json::from_value(intended.clone()).map_err(|_| "unsupported command snapshot")?;
    if receipt
        .tool
        .as_deref()
        .is_some_and(|tool| Some(tool) != intended.get("tool").and_then(Value::as_str))
    {
        return Err("snapshot tool mismatch".into());
    }
    let ids: Vec<&str> = match &command {
        Command::Add { elements } | Command::Update { elements } => {
            elements.iter().map(id).collect::<Result<_>>()?
        }
        Command::Delete { ids } => ids.iter().map(String::as_str).collect(),
    };
    let target_set: BTreeSet<_> = ids.iter().copied().collect();
    if ids.is_empty()
        || target_set.len() != ids.len()
        || target_set != receipt.target_ids.iter().map(String::as_str).collect()
    {
        return Err("snapshot target mismatch".into());
    }
    let actual = index(
        actual
            .get("elements")
            .and_then(Value::as_array)
            .ok_or("canvas elements required")?,
    )?;
    match status {
        OperationStatus::Applied => match &command {
            Command::Delete { .. } => {
                if ids.iter().any(|id| actual.contains_key(id)) {
                    return Err("delete not applied".into());
                }
            }
            Command::Add { .. } | Command::Update { .. } => {
                for patch in patches(&command)? {
                    if !actual
                        .get(id(&patch)?)
                        .is_some_and(|element| equal_fields(element, &patch))
                    {
                        return Err("target projection conflict".into());
                    }
                }
            }
        },
        OperationStatus::Failed => {
            if matches!(command, Command::Add { .. }) {
                if ids.iter().any(|id| actual.contains_key(id)) {
                    return Err("add outcome uncertain".into());
                }
            } else {
                let before: Value = serde_json::from_str(
                    receipt
                        .before_snapshot
                        .as_deref()
                        .ok_or("before snapshot required")?,
                )
                .map_err(|_| "snapshot JSON")?;
                let before = index(
                    before
                        .as_array()
                        .or_else(|| before.get("elements").and_then(Value::as_array))
                        .ok_or("before elements required")?,
                )?;
                match &command {
                    Command::Update { .. } => {
                        for patch in patches(&command)? {
                            let id = id(&patch)?;
                            let prior = before.get(id).ok_or("before target missing")?;
                            let current = actual.get(id).ok_or("current target missing")?;
                            for key in patch.keys() {
                                if !prior.contains_key(key)
                                    || !current
                                        .get(key)
                                        .zip(prior.get(key))
                                        .is_some_and(|(a, b)| equal_value(a, b))
                                {
                                    return Err("before projection unproved".into());
                                }
                            }
                        }
                    }
                    Command::Delete { .. } => {
                        for id in ids {
                            let prior = before.get(id).ok_or("before target missing")?;
                            if prior.len() < 2
                                || !actual
                                    .get(id)
                                    .is_some_and(|current| equal_fields(current, prior))
                            {
                                return Err("before deletion unproved".into());
                            }
                        }
                    }
                    Command::Add { .. } => return Err("invalid projection".into()),
                }
            }
        }
        OperationStatus::Prepared | OperationStatus::Unknown => {
            return Err("settled decision required".into())
        }
    }
    Ok(())
}
