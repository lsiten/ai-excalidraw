use super::{
    digest::{revision, tree_revision},
    fs::Directory,
    skill, Capability, Policy, Result,
};
use serde_json::{json, Value};
use std::{collections::BTreeMap, path::PathBuf};
const MANIFEST_LIMIT: usize = 131_072;
fn catalog(
    agents: &Directory,
    source: &str,
    policy: &Policy,
) -> Result<(BTreeMap<String, Value>, BTreeMap<String, Value>)> {
    let mut skills = BTreeMap::new();
    if agents.exists("skills")? {
        let dir = agents.child("skills", false)?;
        for n in dir.entries()? {
            if n.starts_with(".backup-") {
                continue;
            }
            skill::name(&n)?;
            if skills.len() >= 128 {
                return Err("too many Skills".into());
            }
            let tree = dir.child(&n, false)?.snapshot()?;
            skill::validate(&n, &tree)?;
            let content = tree.get("SKILL.md").ok_or("missing SKILL.md")?;
            let (_, description) = skill::metadata(content)?;
            let prefix = if source == "global" {
                format!("global-skill://{n}")
            } else {
                format!(".agents/skills/{n}")
            };
            let files:Vec<_>=tree.iter().map(|(p,c)|json!({"path":format!("{prefix}/{p}"),"revision":revision(c.as_bytes()),"bytes":c.len()})).collect();
            skills.insert(n.clone(),json!({"name":n,"description":description,"path":format!("{prefix}/SKILL.md"),"revision":tree_revision(&tree),"source":source,"files":files}));
        }
    }
    let mcp = mcp_catalog(agents, policy, None)?;
    Ok((skills, mcp))
}
pub(super) fn mcp_catalog(
    agents: &Directory,
    policy: &Policy,
    project_entries: Option<&BTreeMap<String, Value>>,
) -> Result<BTreeMap<String, Value>> {
    let source = if project_entries.is_some() {
        "global"
    } else {
        "project"
    };
    let mut mcp = BTreeMap::new();
    if agents.exists("mcp.json")? {
        let config: Value = serde_json::from_str(&agents.read("mcp.json")?)
            .map_err(|e| format!("invalid MCP configuration: {e}"))?;
        let servers = config
            .get("mcpServers")
            .and_then(Value::as_object)
            .ok_or("mcpServers object required")?;
        if servers.len() > 128 {
            return Err("too many MCP servers".into());
        }
        for (name, value) in servers {
            if project_entries.is_some_and(|entries| entries.contains_key(name)) {
                continue;
            }
            if name.is_empty() || name.len() > 128 || name.chars().any(char::is_control) {
                return Err("invalid MCP name".into());
            }
            let value = value.as_object().ok_or("MCP server must be an object")?;
            for key in ["enabled", "disabled"] {
                if value.get(key).is_some_and(|v| !v.is_boolean()) {
                    return Err("MCP enabled/disabled must be boolean".into());
                }
            }
            let enabled = value.get("enabled").and_then(Value::as_bool) == Some(true)
                && value.get("disabled").and_then(Value::as_bool) != Some(true)
                && policy.permits(Capability::Mcp, true);
            let connection = if value.contains_key("command") {
                "stdio"
            } else if value.contains_key("url") {
                "http"
            } else {
                "unknown"
            };
            let enabled = enabled && connection != "unknown";
            mcp.insert(
                name.clone(),
                json!({"name":name,"enabled":enabled,"connectionType":connection,"source":source}),
            );
        }
    }
    Ok(mcp)
}
/// Project entries replace whole global entries; disabled entries never inherit enablement.
pub(super) fn resolve(
    project: Option<&Directory>,
    global: Option<&Directory>,
    policy: &Policy,
) -> Result<Value> {
    let (mut skills, mut mcp) = match project {
        Some(p) => catalog(p, "project", policy)?,
        None => (BTreeMap::new(), BTreeMap::new()),
    };
    let mut warnings = Vec::new();
    if let Some(global) = global {
        super::global_catalog::extend(global, &mut skills, &mut mcp, policy, &mut warnings);
    }
    let mut memory = Vec::new();
    if let Some(project) = project {
        if project.exists("memory")? {
            let dir = project.child("memory", false)?;
            for n in dir.entries()? {
                if n.starts_with(".backup-") || !n.ends_with(".json") {
                    continue;
                }
                if memory.len() >= 64 {
                    return Err("memory context limit exceeded".into());
                }
                skill::name(n.strip_suffix(".json").ok_or("invalid memory filename")?)?;
                let text = dir.read(&n)?;
                let fact: Value = serde_json::from_str(&text).map_err(|e| e.to_string())?;
                if fact.get("confirmed") == Some(&Value::Bool(true))
                    && fact.get("source").and_then(Value::as_str) == Some("explicit-user-command")
                {
                    let summary = fact
                        .get("content")
                        .and_then(Value::as_str)
                        .ok_or("invalid confirmed fact")?;
                    memory.push(json!({"path":format!(".agents/memory/{n}"),"revision":revision(text.as_bytes()),"summary":summary.chars().take(512).collect::<String>(),"confirmed":true}));
                }
            }
        }
    }
    let policy_source = match project {
        Some(p) if p.exists("policy.json")? => Some(".agents/policy.json"),
        _ => None,
    };
    let mut value = json!({"kind":"manifest","skills":skills.into_values().collect::<Vec<_>>(),"mcp":mcp.into_values().collect::<Vec<_>>(),"memory":memory,"warnings":warnings,"policy":policy,"provenance":{"skills":".agents/skills","mcp":".agents/mcp.json","policy":policy_source,"resolution":"project-first","fallback":global.map(|_|"$HOME/.agents"),"mcpAuthority":"configuration-only; native permissions still required"}});
    let mut truncated = 0;
    while serde_json::to_vec(&value).map_err(|e| e.to_string())?.len() > MANIFEST_LIMIT - 128 {
        let mut removed = false;
        for key in ["skills", "mcp"] {
            if let Some(entries) = value[key].as_array_mut() {
                if let Some(index) = entries
                    .iter()
                    .rposition(|entry| entry["source"] == "global")
                {
                    entries.remove(index);
                    removed = true;
                    truncated += 1;
                    break;
                }
            }
        }
        if !removed {
            return Err("project manifest exceeds 128 KiB".into());
        }
    }
    if truncated > 0 {
        if let Some(warnings) = value["warnings"].as_array_mut() {
            warnings.push(json!(format!(
                "Global entries omitted for context budget: {truncated}"
            )));
        }
    }
    Ok(value)
}
fn global() -> Result<Option<Directory>> {
    let Some(home) = std::env::var_os("HOME") else {
        return Ok(None);
    };
    let root = PathBuf::from(home)
        .canonicalize()
        .map_err(|e| e.to_string())?;
    let home = Directory::root(&root)?;
    if home.exists(".agents")? {
        Ok(Some(home.child(".agents", false)?))
    } else {
        Ok(None)
    }
}
fn context(project: Option<&Directory>, policy: &Policy) -> Result<Value> {
    match global() {
        Ok(Some(global)) => resolve(project, Some(&global), policy),
        Ok(None) => resolve(project, None, policy),
        Err(_) => {
            let mut value = resolve(project, None, policy)?;
            value["warnings"] =
                json!(["Global fallback unavailable or invalid; project context retained"]);
            Ok(value)
        }
    }
}
pub fn empty() -> Result<Value> {
    context(None, &Policy::default())
}
pub fn load(agents: &Directory, policy: &Policy) -> Result<Value> {
    context(Some(agents), policy)
}

/// Resolve only a currently discoverable global Skill file, never a caller-supplied home path.
pub fn read_global(project: Option<&Directory>, policy: &Policy, path: &str) -> Result<Value> {
    let global = global()?.ok_or("global Skill fallback unavailable")?;
    read_global_from(project, &global, policy, path)
}
pub(super) fn read_global_from(
    project: Option<&Directory>,
    global: &Directory,
    policy: &Policy,
    path: &str,
) -> Result<Value> {
    if !policy.permits(Capability::Read, true) {
        return Err("project read policy denied".into());
    }
    let relative = path
        .strip_prefix("global-skill://")
        .ok_or("invalid global Skill reference")?;
    let (name, resource) = relative
        .split_once('/')
        .ok_or("global Skill resource required")?;
    skill::name(name)?;
    super::fs::components(resource)?;
    let manifest = resolve(project, Some(global), policy)?;
    let approved = manifest["skills"]
        .as_array()
        .and_then(|skills| {
            skills.iter().find(|entry| {
                entry["name"].as_str() == Some(name)
                    && entry["source"].as_str() == Some("global")
                    && entry["files"].as_array().is_some_and(|files| {
                        files.iter().any(|file| file["path"].as_str() == Some(path))
                    })
            })
        })
        .ok_or("global Skill file is not approved by the current project manifest")?;
    let directory = global.subdir(&format!("skills/{name}"), false)?;
    let instruction = directory.read("SKILL.md")?;
    let bundle = revision(instruction.as_bytes());
    if approved["revision"].as_str() != Some(bundle.as_str()) {
        return Err("global Skill changed during read; refresh manifest".into());
    }
    let content = if resource == "SKILL.md" {
        instruction
    } else {
        directory.read_path(resource)?
    };
    // A project entry appearing while the fallback was read must take precedence too.
    if let Some(project) = project {
        if project.exists("skills")? && project.child("skills", false)?.exists(name)? {
            return Err("project Skill overrides global fallback".into());
        }
    }
    let content_revision = revision(content.as_bytes());
    Ok(
        json!({"kind":"file","path":path,"revision":if resource=="SKILL.md"{bundle}else{content_revision.clone()},"contentRevision":content_revision,"content":content}),
    )
}
