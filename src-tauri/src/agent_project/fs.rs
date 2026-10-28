//! Descriptor-relative IO: never follow links, including intermediate components.
use super::Result;
use rustix::fs::{self, AtFlags, Dir, FileType, FlockOperation, Mode, OFlags, RenameFlags};
use std::{
    collections::BTreeMap,
    fs::File,
    io::{Read, Write},
    path::Path,
};
pub const FILE_LIMIT: usize = 262_144;
pub const TOTAL_LIMIT: usize = 2_097_152;
pub const FILE_COUNT: usize = 64;
pub type Tree = BTreeMap<String, String>;
pub struct Directory(pub File);
fn error(e: impl std::fmt::Display) -> String {
    e.to_string()
}
pub fn components(path: &str) -> Result<Vec<&str>> {
    if path.is_empty() || path.len() > 1024 || path.contains(['\\', '\0', ':']) {
        return Err("invalid relative path".into());
    }
    let parts: Vec<_> = path.split('/').collect();
    if parts.len() > 16
        || parts
            .iter()
            .any(|p| p.is_empty() || *p == "." || *p == ".." || p.starts_with('.') || p.len() > 128)
    {
        return Err("unsafe relative path".into());
    }
    Ok(parts)
}
impl Directory {
    pub fn root(path: &Path) -> Result<Self> {
        let canonical = path.canonicalize().map_err(error)?;
        if canonical != path {
            return Err("project root changed".into());
        }
        let mut directory = Self(File::from(
            fs::open(
                "/",
                OFlags::RDONLY | OFlags::DIRECTORY | OFlags::CLOEXEC,
                Mode::empty(),
            )
            .map_err(error)?,
        ));
        for part in path.components() {
            if let std::path::Component::Normal(part) = part {
                directory = directory.child(part.to_str().ok_or("non UTF-8 root")?, false)?;
            }
        }
        Ok(directory)
    }
    pub fn identity(&self, name: &str) -> Result<(i128, u64)> {
        let s = fs::statat(&self.0, name, AtFlags::SYMLINK_NOFOLLOW).map_err(error)?;
        Ok((i128::from(s.st_dev), s.st_ino))
    }
    pub fn verify_child(&self, name: &str, child: &Self) -> Result<()> {
        let actual = fs::fstat(&child.0).map_err(error)?;
        if self.identity(name)? != (i128::from(actual.st_dev), actual.st_ino) {
            return Err("project directory replaced during operation".into());
        }
        Ok(())
    }
    pub fn lock(&self) -> Result<()> {
        fs::flock(&self.0, FlockOperation::NonBlockingLockExclusive)
            .map_err(|e| format!("project busy: {e}"))
    }
    pub fn child(&self, name: &str, create: bool) -> Result<Self> {
        if create {
            match fs::mkdirat(&self.0, name, Mode::RUSR | Mode::WUSR | Mode::XUSR) {
                Ok(()) | Err(rustix::io::Errno::EXIST) => (),
                Err(e) => return Err(error(e)),
            }
        }
        Ok(Self(File::from(
            fs::openat(
                &self.0,
                name,
                OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
                Mode::empty(),
            )
            .map_err(error)?,
        )))
    }
    pub fn subdir(&self, path: &str, create: bool) -> Result<Self> {
        let mut current = Self(self.0.try_clone().map_err(error)?);
        for part in components(path)? {
            current = current.child(part, create)?;
        }
        Ok(current)
    }
    pub fn exists(&self, name: &str) -> Result<bool> {
        match fs::statat(&self.0, name, AtFlags::SYMLINK_NOFOLLOW) {
            Ok(_) => Ok(true),
            Err(rustix::io::Errno::NOENT) => Ok(false),
            Err(e) => Err(error(e)),
        }
    }
    pub fn read(&self, name: &str) -> Result<String> {
        let fd = fs::openat(
            &self.0,
            name,
            OFlags::RDONLY | OFlags::NOFOLLOW | OFlags::NONBLOCK | OFlags::CLOEXEC,
            Mode::empty(),
        )
        .map_err(error)?;
        let stat = fs::fstat(&fd).map_err(error)?;
        if FileType::from_raw_mode(stat.st_mode) != FileType::RegularFile
            || stat.st_nlink != 1
            || stat.st_size < 0
            || u64::try_from(stat.st_size).map_err(error)? > FILE_LIMIT as u64
        {
            return Err("not a bounded, unlinked regular file".into());
        }
        let mut result = String::new();
        File::from(fd)
            .take((FILE_LIMIT + 1) as u64)
            .read_to_string(&mut result)
            .map_err(error)?;
        if result.len() > FILE_LIMIT {
            return Err("file too large".into());
        }
        Ok(result)
    }
    pub fn read_path(&self, path: &str) -> Result<String> {
        let parts = components(path)?;
        let mut directory = Self(self.0.try_clone().map_err(error)?);
        for part in &parts[..parts.len() - 1] {
            directory = directory.child(part, false)?;
        }
        directory.read(parts[parts.len() - 1])
    }
    pub fn write_new(&self, name: &str, content: &str) -> Result<()> {
        let mut file = File::from(
            fs::openat(
                &self.0,
                name,
                OFlags::WRONLY | OFlags::CREATE | OFlags::EXCL | OFlags::NOFOLLOW | OFlags::CLOEXEC,
                Mode::RUSR | Mode::WUSR,
            )
            .map_err(error)?,
        );
        file.write_all(content.as_bytes()).map_err(error)?;
        file.sync_all().map_err(error)
    }
    pub fn entries(&self) -> Result<Vec<String>> {
        let mut result = Vec::new();
        for entry in Dir::read_from(&self.0).map_err(error)? {
            let entry = entry.map_err(error)?;
            let name = entry.file_name().to_str().map_err(error)?;
            if name == "." || name == ".." {
                continue;
            }
            if result.len() >= 256 {
                return Err("directory entry limit exceeded".into());
            }
            result.push(name.to_owned());
        }
        result.sort();
        Ok(result)
    }
    pub fn snapshot(&self) -> Result<Tree> {
        let mut tree = Tree::new();
        self.collect("", &mut tree)?;
        Ok(tree)
    }
    fn collect(&self, prefix: &str, tree: &mut Tree) -> Result<()> {
        for name in self.entries()? {
            let path = format!("{prefix}{name}");
            components(&path)?;
            let stat = fs::statat(&self.0, &name, AtFlags::SYMLINK_NOFOLLOW).map_err(error)?;
            match FileType::from_raw_mode(stat.st_mode) {
                FileType::Directory => self
                    .child(&name, false)?
                    .collect(&format!("{path}/"), tree)?,
                FileType::RegularFile => {
                    tree.insert(path, self.read(&name)?);
                }
                _ => return Err("symlink or special resource rejected".into()),
            }
            validate_tree(tree)?;
        }
        Ok(())
    }
    /// Metadata-only inventory: never opens resource content while listing a global Skill.
    pub fn inventory(&self) -> Result<BTreeMap<String, usize>> {
        let mut files = BTreeMap::new();
        self.inventory_into("", &mut files)?;
        Ok(files)
    }
    fn inventory_into(&self, prefix: &str, files: &mut BTreeMap<String, usize>) -> Result<()> {
        for name in self.entries()? {
            let path = format!("{prefix}{name}");
            components(&path)?;
            let stat = fs::statat(&self.0, &name, AtFlags::SYMLINK_NOFOLLOW).map_err(error)?;
            match FileType::from_raw_mode(stat.st_mode) {
                FileType::Directory => self
                    .child(&name, false)?
                    .inventory_into(&format!("{path}/"), files)?,
                FileType::RegularFile => {
                    let size = usize::try_from(stat.st_size).map_err(error)?;
                    if stat.st_nlink != 1 || size > FILE_LIMIT {
                        return Err("unsafe or oversized resource".into());
                    }
                    files.insert(path, size);
                }
                _ => return Err("unsafe resource type".into()),
            }
            if files.len() > FILE_COUNT || files.values().sum::<usize>() > TOTAL_LIMIT {
                return Err("resource inventory budget exceeded".into());
            }
        }
        Ok(())
    }
    pub fn stage_tree(&self, name: &str, tree: &Tree) -> Result<()> {
        fs::mkdirat(&self.0, name, Mode::RUSR | Mode::WUSR | Mode::XUSR).map_err(error)?;
        let staged = self.child(name, false)?;
        for (path, content) in tree {
            let parts = components(path)?;
            let mut parent = Self(staged.0.try_clone().map_err(error)?);
            for part in &parts[..parts.len() - 1] {
                parent = parent.child(part, true)?;
            }
            parent.write_new(parts[parts.len() - 1], content)?;
            parent.sync()?;
        }
        staged.sync()
    }
    pub fn rename(&self, from: &str, to: &str, exchange: bool) -> Result<()> {
        fs::renameat_with(
            &self.0,
            from,
            &self.0,
            to,
            if exchange {
                RenameFlags::EXCHANGE
            } else {
                RenameFlags::NOREPLACE
            },
        )
        .map_err(error)?;
        Ok(())
    }
    pub fn remove(&self, name: &str) -> Result<()> {
        let stat = fs::statat(&self.0, name, AtFlags::SYMLINK_NOFOLLOW).map_err(error)?;
        if FileType::from_raw_mode(stat.st_mode) == FileType::Directory {
            let dir = self.child(name, false)?;
            for child in dir.entries()? {
                dir.remove(&child)?;
            }
            fs::unlinkat(&self.0, name, AtFlags::REMOVEDIR).map_err(error)
        } else {
            fs::unlinkat(&self.0, name, AtFlags::empty()).map_err(error)
        }
    }
    pub fn sync(&self) -> Result<()> {
        self.0.sync_all().map_err(error)
    }
}
pub fn validate_tree(tree: &Tree) -> Result<()> {
    if tree.len() > FILE_COUNT
        || tree.values().any(|v| v.len() > FILE_LIMIT)
        || tree.values().map(String::len).sum::<usize>() > TOTAL_LIMIT
    {
        return Err("Skill resource budget exceeded".into());
    }
    let mut folded = std::collections::BTreeSet::new();
    for key in tree.keys() {
        components(key)?;
        if !folded.insert(key.to_lowercase()) {
            return Err("case-insensitive path collision".into());
        }
    }
    Ok(())
}
