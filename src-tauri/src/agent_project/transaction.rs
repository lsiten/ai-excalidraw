use super::{
    digest::{revision, tree_revision},
    fs::{Directory, Tree},
    Result,
};
use serde_json::{json, Value};
pub enum Contents {
    File(String),
    Skill(Tree),
}
impl Contents {
    pub fn revision(&self) -> String {
        match self {
            Self::File(s) => revision(s.as_bytes()),
            Self::Skill(t) => tree_revision(t),
        }
    }
    fn read_like(&self, parent: &Directory, name: &str) -> Result<Self> {
        match self {
            Self::File(_) => Ok(Self::File(parent.read(name)?)),
            Self::Skill(_) => Ok(Self::Skill(parent.child(name, false)?.snapshot()?)),
        }
    }
    fn stage(&self, parent: &Directory, name: &str) -> Result<()> {
        match self {
            Self::File(s) => parent.write_new(name, s),
            Self::Skill(t) => parent.stage_tree(name, t),
        }
    }
}
pub struct Write<'a> {
    pub name: &'a str,
    pub path: &'a str,
    pub contents: Contents,
    pub expected: Option<String>,
}
/// Publication is a single OS rename/exchange. The displaced original is the backup.
/// Recheck the displaced content, catching edits that raced the preflight check.
pub fn commit(
    parent: &Directory,
    write: Write<'_>,
    verify_parent: impl Fn() -> Result<()>,
) -> Result<Value> {
    let previous = if parent.exists(write.name)? {
        Some(write.contents.read_like(parent, write.name)?.revision())
    } else {
        None
    };
    if previous != write.expected {
        return Err("revision conflict".into());
    }
    let id = uuid::Uuid::new_v4().to_string();
    let stage = format!(".backup-{id}");
    if let Err(e) = write.contents.stage(parent, &stage) {
        if parent.exists(&stage)? {
            parent.remove(&stage)?;
        }
        return Err(e);
    }
    let staged_identity = parent.identity(&stage)?;
    let preflight = (|| {
        verify_parent()?;
        let current = if parent.exists(write.name)? {
            Some(write.contents.read_like(parent, write.name)?.revision())
        } else {
            None
        };
        if current != previous {
            return Err("revision conflict before publication".into());
        }
        if write.contents.read_like(parent, &stage)?.revision() != write.contents.revision() {
            return Err("staging verification failed".into());
        }
        #[cfg(test)]
        fault("before_publish")?;
        Ok(())
    })();
    if let Err(e) = preflight {
        parent.remove(&stage)?;
        return Err(e);
    }
    if let Err(e) = parent.rename(&stage, write.name, previous.is_some()) {
        parent.remove(&stage)?;
        return Err(e);
    }
    let verified: Result<()> = (|| {
        #[cfg(test)]
        fault("after_publish")?;
        verify_parent()?;
        if parent.identity(write.name)? != staged_identity {
            return Err("concurrent replacement after publication".into());
        }
        if previous.is_some()
            && Some(write.contents.read_like(parent, &stage)?.revision()) != previous
        {
            return Err("concurrent edit captured in backup".into());
        }
        if write.contents.read_like(parent, write.name)?.revision() != write.contents.revision() {
            return Err("saved content verification failed".into());
        }
        parent.sync()?;
        Ok(())
    })();
    let parent_path = write.path.rsplit_once('/').map_or("", |(p, _)| p);
    let backup_path = format!("{parent_path}/{stage}");
    if let Err(e) = verified {
        if parent.identity(write.name)? != staged_identity {
            return Err(format!(
                "{e}; concurrent replacement preserved; recovery: {backup_path}"
            ));
        }
        // Preserve any newly edited published bytes as well; never discard an editor's changes.
        let edited = match write.contents.read_like(parent, write.name) {
            Ok(actual) => actual.revision() != write.contents.revision(),
            Err(_) => true,
        };
        if previous.is_some() {
            parent.rename(&stage, write.name, true)?;
        } else {
            parent.rename(write.name, &stage, false)?;
        }
        if edited {
            return Err(format!(
                "{e}; rolled back; concurrent content retained: {backup_path}"
            ));
        }
        parent.remove(&stage)?;
        parent.sync()?;
        return Err(format!("{e}; rolled back"));
    }
    let files=match &write.contents {
        Contents::File(content)=>vec![json!({"path":write.path,"revision":revision(content.as_bytes()),"bytes":content.len()})],
        Contents::Skill(tree)=>tree.iter().map(|(p,c)|json!({"path":format!("{}/{p}",write.path),"revision":revision(c.as_bytes()),"bytes":c.len()})).collect(),
    };
    Ok(
        json!({"kind":"artifact","artifactId":id,"path":match &write.contents{Contents::Skill(_)=>format!("{}/SKILL.md",write.path),Contents::File(_)=>write.path.to_owned()},"revision":write.contents.revision(),"backupPath":previous.map(|_|backup_path),"files":files,"saved":true,"verified":true}),
    )
}
#[cfg(test)]
thread_local! {pub static FAULT:std::cell::RefCell<Option<&'static str>>=const {std::cell::RefCell::new(None)};}
#[cfg(test)]
fn fault(point: &str) -> Result<()> {
    if CRASH_AT.with(|f| *f.borrow() == Some(point)) {
        std::process::exit(91);
    }
    if FAULT.with(|f| *f.borrow() == Some(point)) {
        Err(format!("injected {point} failure"))
    } else {
        Ok(())
    }
}

#[cfg(test)]
thread_local! {pub static CRASH_AT:std::cell::RefCell<Option<&'static str>>=const {std::cell::RefCell::new(None)};}
