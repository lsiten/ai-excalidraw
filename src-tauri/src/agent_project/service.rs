use super::{
    digest::{revision, tree_revision},
    fs::{Directory, Tree, FILE_LIMIT},
    manifest, skill,
    transaction::{self, Contents, Write},
    Capability, Operation, Policy, Result,
};
use serde_json::{json, Value};
use std::path::Path;

pub(super) fn dispatch(root: &Path, operation: Operation) -> Result<Value> {
    let project = Directory::root(root)?;
    project.lock()?;
    match &operation {
        Operation::WriteSkill {
            authorized: false, ..
        }
        | Operation::WritePolicy {
            authorized: false, ..
        }
        | Operation::WriteMemory {
            authorized: false, ..
        }
        | Operation::WriteMemory {
            confirmed: false, ..
        } => return Err("explicit authorization and memory confirmation required".into()),
        _ => (),
    }
    if let Operation::WriteMemory { content, .. } = &operation {
        super::memory::validate(content)?;
    }
    let writing = matches!(
        &operation,
        Operation::WriteSkill { .. }
            | Operation::WriteMemory { .. }
            | Operation::WritePolicy { .. }
    );
    // Merely reading context must not create project files.
    if !project.exists(".agents")? && !writing {
        return match operation {
            Operation::Manifest => manifest::empty(),
            Operation::Read { path } if path.starts_with("global-skill://") => {
                manifest::read_global(None, &Policy::default(), &path)
            }
            _ => Err("project agent file not found".into()),
        };
    }
    let agents = project.child(".agents", writing)?;
    let policy: Policy = if agents.exists("policy.json")? {
        serde_json::from_str(&agents.read("policy.json")?)
            .map_err(|e| format!("invalid project policy: {e}"))?
    } else {
        Policy::default()
    };
    policy.validate()?;
    let (cap, authorized) = match &operation {
        Operation::Manifest | Operation::Read { .. } => (Capability::Read, true),
        Operation::WriteSkill { authorized, .. } => (Capability::WriteSkill, *authorized),
        Operation::WriteMemory {
            authorized,
            confirmed,
            ..
        } => (Capability::WriteMemory, *authorized && *confirmed),
        Operation::WritePolicy { authorized, .. } => (Capability::WritePolicy, *authorized),
    };
    if !policy.permits(cap, authorized) {
        return Err("capability denied or explicit confirmation missing".into());
    }
    let verify = || {
        let current = Directory::root(root)?;
        current.verify_child(".agents", &agents)?;
        project.verify_child(".agents", &agents)
    };
    match operation {
        Operation::Manifest => {
            let value = manifest::load(&agents, &policy)?;
            verify()?;
            Ok(value)
        }
        Operation::Read { path } => {
            if path.starts_with("global-skill://") {
                let file = manifest::read_global(Some(&agents), &policy, &path)?;
                verify()?;
                return Ok(file);
            }
            let relative = path
                .strip_prefix(".agents/")
                .ok_or("preview must be a scoped agent file")?;
            if !(relative.starts_with("skills/")
                || relative.starts_with("memory/")
                || relative == "policy.json")
            {
                return Err("preview scope denied".into());
            }
            let (content, rev) = if let Some(skill_path) = relative
                .strip_prefix("skills/")
                .and_then(|p| p.strip_suffix("/SKILL.md"))
            {
                skill::name(skill_path)?;
                let tree = agents
                    .subdir(&format!("skills/{skill_path}"), false)?
                    .snapshot()?;
                let content = tree.get("SKILL.md").ok_or("missing SKILL.md")?.clone();
                (content, tree_revision(&tree))
            } else {
                let content = agents.read_path(relative)?;
                let rev = revision(content.as_bytes());
                (content, rev)
            };
            let content_revision = revision(content.as_bytes());
            verify()?;
            Ok(
                json!({"kind":"file","path":path,"revision":rev,"contentRevision":content_revision,"content":content}),
            )
        }
        Operation::WritePolicy {
            policy,
            expected_revision,
            ..
        } => {
            policy.validate()?;
            let content = serde_json::to_string_pretty(&policy).map_err(|e| e.to_string())?;
            transaction::commit(
                &agents,
                Write {
                    name: "policy.json",
                    path: ".agents/policy.json",
                    contents: Contents::File(content),
                    expected: expected_revision,
                },
                verify,
            )
        }
        Operation::WriteMemory {
            name,
            content,
            expected_revision,
            ..
        } => {
            skill::name(&name)?;
            let stored=serde_json::to_string_pretty(&json!({"version":1,"confirmed":true,"source":"explicit-user-command","content":content})).map_err(|e|e.to_string())?;
            let memory = agents.child("memory", true)?;
            transaction::commit(
                &memory,
                Write {
                    name: &format!("{name}.json"),
                    path: &format!(".agents/memory/{name}.json"),
                    contents: Contents::File(stored),
                    expected: expected_revision,
                },
                || {
                    verify()?;
                    agents.verify_child("memory", &memory)
                },
            )
        }
        Operation::WriteSkill {
            name,
            content,
            files,
            expected_revision,
            ..
        } => {
            skill::name(&name)?;
            if content.len() > FILE_LIMIT || files.len() > 63 {
                return Err("Skill payload exceeds budget".into());
            }
            let skills = agents.child("skills", true)?;
            let mut tree = if skills.exists(&name)? {
                skills.child(&name, false)?.snapshot()?
            } else {
                Tree::new()
            };
            let old_revision = if tree.is_empty() {
                None
            } else {
                Some(tree_revision(&tree))
            };
            if old_revision != expected_revision {
                return Err("revision conflict".into());
            }
            tree.insert("SKILL.md".into(), content);
            let mut unique = std::collections::BTreeSet::new();
            for file in files {
                super::fs::components(&file.path)?;
                if file.path.eq_ignore_ascii_case("SKILL.md")
                    || !unique.insert(file.path.to_lowercase())
                {
                    return Err("duplicate or reserved resource path".into());
                }
                tree.insert(file.path, file.content);
            }
            skill::validate(&name, &tree)?;
            transaction::commit(
                &skills,
                Write {
                    name: &name,
                    path: &format!(".agents/skills/{name}"),
                    contents: Contents::Skill(tree),
                    expected: expected_revision,
                },
                || {
                    verify()?;
                    agents.verify_child("skills", &skills)
                },
            )
        }
    }
}
