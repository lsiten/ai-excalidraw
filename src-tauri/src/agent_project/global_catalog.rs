use super::{
    digest::revision,
    fs::{Directory, Tree},
    manifest, skill, Policy, Result,
};
use serde_json::{json, Value};
use std::collections::BTreeMap;

fn entry(directory: &Directory, name: &str) -> Result<Value> {
    let content = directory.read("SKILL.md")?;
    let (_, description) = skill::metadata(&content)?;
    let inventory = directory.inventory()?;
    let mut skeleton: Tree = inventory
        .keys()
        .map(|path| (path.clone(), String::new()))
        .collect();
    skeleton.insert("SKILL.md".into(), content.clone());
    skill::validate(name, &skeleton)?;
    let hash = revision(content.as_bytes());
    let prefix = format!("global-skill://{name}");
    let files:Vec<_>=inventory.into_iter().map(|(path,bytes)|json!({"path":format!("{prefix}/{path}"),"bytes":bytes,"revision":if path=="SKILL.md"{Some(&hash)}else{None}})).collect();
    Ok(
        json!({"name":name,"description":description,"path":format!("{prefix}/SKILL.md"),"source":"global","revision":hash,"files":files}),
    )
}
/// Invalid external entries never erase valid project or sibling global entries.
pub fn extend(
    global: &Directory,
    skills: &mut BTreeMap<String, Value>,
    mcp: &mut BTreeMap<String, Value>,
    policy: &Policy,
    warnings: &mut Vec<String>,
) {
    let catalog: Result<Option<(Directory, Vec<String>)>> = (|| {
        if !global.exists("skills")? {
            return Ok(None);
        }
        let dir = global.child("skills", false)?;
        let names = dir.entries()?;
        Ok(Some((dir, names)))
    })();
    let mut invalid = 0;
    let mut unsafe_roots = 0;
    let mut omitted = 0;
    match catalog {
        Ok(Some((directory, names))) => {
            for name in names {
                if name.starts_with(".backup-") || skills.contains_key(&name) {
                    continue;
                }
                if skills.len() >= 128 {
                    omitted += 1;
                    continue;
                }
                if skill::name(&name).is_err() {
                    unsafe_roots += 1;
                    continue;
                }
                match directory.child(&name, false) {
                    Ok(dir) => match entry(&dir, &name) {
                        Ok(value) => {
                            skills.insert(name, value);
                        }
                        Err(_) => invalid += 1,
                    },
                    Err(_) => unsafe_roots += 1,
                }
            }
        }
        Ok(None) => (),
        Err(_) => warnings.push("Global Skill directory unavailable or over entry budget".into()),
    }
    if unsafe_roots > 0 {
        warnings.push(format!(
            "Quarantined global Skill roots (unsafe path/type): {unsafe_roots}"
        ));
    }
    if invalid > 0 {
        warnings.push(format!(
            "Quarantined global Skills (invalid metadata/resource/budget): {invalid}"
        ));
    }
    if omitted > 0 {
        warnings.push(format!("Global Skills omitted for entry budget: {omitted}"));
    }
    match manifest::mcp_catalog(global, policy, Some(mcp)) {
        Ok(entries) => {
            for (name, value) in entries {
                if mcp.len() < 128 {
                    mcp.entry(name).or_insert(value);
                }
            }
        }
        Err(_) => warnings.push("Global MCP configuration unavailable or invalid".into()),
    }
}
