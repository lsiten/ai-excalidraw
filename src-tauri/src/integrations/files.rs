use super::{model::Status, PAYLOAD};
use std::{
    fs::{self, OpenOptions},
    io::Write,
    path::{Component, Path, PathBuf},
};
use uuid::Uuid;

pub fn safe_path(path: &Path) -> Result<(), String> {
    if !path.is_absolute()
        || path
            .components()
            .any(|part| matches!(part, Component::ParentDir))
    {
        return Err("安装路径必须是绝对路径，且不能包含上级目录跳转".into());
    }
    for ancestor in path.ancestors() {
        match fs::symlink_metadata(ancestor) {
            Ok(meta) if meta.file_type().is_symlink() => {
                return Err(format!("不支持符号链接路径：{}", ancestor.display()))
            }
            Ok(_) => {}
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => return Err(format!("无法检查 {}：{e}", ancestor.display())),
        }
    }
    Ok(())
}
pub fn read_optional(path: &Path) -> Result<Option<Vec<u8>>, String> {
    match fs::read(path) {
        Ok(bytes) => Ok(Some(bytes)),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(format!("无法读取 {}：{e}", path.display())),
    }
}
pub fn skill_status(path: &Path) -> Result<Status, String> {
    safe_path(path)?;
    if path.exists() && !path.is_dir() {
        return Err("Skill 路径必须为目录".into());
    }
    let mut current = true;
    let mut present = false;
    for (relative, expected) in PAYLOAD {
        let file = path.join(relative);
        safe_path(&file)?;
        let actual = read_optional(&file)?;
        if *relative == "SKILL.md" {
            present = actual.is_some();
        }
        current &= actual.as_deref() == Some(*expected);
    }
    Ok(if current {
        Status::Current
    } else if present {
        Status::Outdated
    } else {
        Status::Missing
    })
}
fn private_write(path: &Path, data: &[u8]) -> Result<(), String> {
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = options.open(path).map_err(|e| e.to_string())?;
    file.write_all(data)
        .and_then(|()| file.sync_all())
        .map_err(|e| e.to_string())
}
fn atomic_write(path: &Path, data: &[u8]) -> Result<(), String> {
    safe_path(path)?;
    let parent = path.parent().ok_or("缺少父目录")?;
    fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    safe_path(path)?;
    let temp = parent.join(format!(".ai-excalidraw-{}.tmp", Uuid::new_v4()));
    let result = private_write(&temp, data)
        .and_then(|()| fs::rename(&temp, path).map_err(|e| e.to_string()));
    if result.is_err() && temp.exists() {
        if let Err(e) = fs::remove_file(&temp) {
            return Err(format!("写入失败，且无法清理临时文件：{e}"));
        }
    }
    result
}
struct Change {
    path: PathBuf,
    original: Option<Vec<u8>>,
    backup: Option<PathBuf>,
}
#[derive(Default)]
pub struct Transaction {
    changes: Vec<Change>,
    finished: bool,
}
impl Transaction {
    pub fn write(&mut self, path: &Path, data: &[u8]) -> Result<(), String> {
        safe_path(path)?;
        let original = read_optional(path)?;
        if original.as_deref() == Some(data) {
            return Ok(());
        }
        let backup = match &original {
            Some(bytes) => {
                let backup = path.with_file_name(format!(
                    "{}.ai-excalidraw-backup-{}",
                    path.file_name().ok_or("缺少文件名")?.to_string_lossy(),
                    Uuid::new_v4()
                ));
                private_write(&backup, bytes)?;
                Some(backup)
            }
            None => None,
        };
        self.changes.push(Change {
            path: path.into(),
            original,
            backup,
        });
        atomic_write(path, data)
    }
    pub fn backups(&self) -> Vec<String> {
        self.changes
            .iter()
            .filter_map(|c| c.backup.as_ref().map(|p| p.display().to_string()))
            .collect()
    }
    pub fn commit(&mut self) {
        self.finished = true;
    }
    pub fn rollback(&mut self) -> Result<(), String> {
        let mut failures = Vec::new();
        for change in self.changes.iter().rev() {
            let result = match &change.original {
                Some(bytes) => atomic_write(&change.path, bytes),
                None => {
                    safe_path(&change.path).and_then(|()| match fs::remove_file(&change.path) {
                        Ok(()) => Ok(()),
                        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
                        Err(e) => Err(e.to_string()),
                    })
                }
            };
            if let Err(e) = result {
                failures.push(format!("{}: {e}", change.path.display()));
            }
        }
        self.finished = true;
        if failures.is_empty() {
            Ok(())
        } else {
            Err(failures.join("; "))
        }
    }
}
impl Drop for Transaction {
    fn drop(&mut self) {
        if !self.finished {
            if let Err(e) = self.rollback() {
                eprintln!("Integration rollback failed: {e}");
            }
        }
    }
}
