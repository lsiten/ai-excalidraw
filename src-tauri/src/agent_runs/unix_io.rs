//! Descriptor-relative journal IO; every directory component is opened without following links.
use super::*;
use rustix::fs::{self as sys, Mode, OFlags};
use std::path::Component;
fn error(_: impl std::fmt::Display) -> String {
    "secure filesystem operation failed".into()
}
pub(super) fn directory(path: &Path, create: bool) -> Result<fs::File> {
    if !path.is_absolute() {
        return Err("absolute storage path required".into());
    }
    let mut dir = fs::File::from(
        sys::open(
            "/",
            OFlags::RDONLY | OFlags::DIRECTORY | OFlags::CLOEXEC,
            Mode::empty(),
        )
        .map_err(error)?,
    );
    for part in path.components() {
        match part {
            Component::RootDir => {}
            Component::Normal(name) => {
                if create {
                    match sys::mkdirat(&dir, name, Mode::RUSR | Mode::WUSR | Mode::XUSR) {
                        Ok(()) | Err(rustix::io::Errno::EXIST) => {}
                        Err(e) => return Err(error(e)),
                    }
                }
                dir = fs::File::from(
                    sys::openat(
                        &dir,
                        name,
                        OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
                        Mode::empty(),
                    )
                    .map_err(error)?,
                );
            }
            _ => return Err("invalid storage component".into()),
        }
    }
    Ok(dir)
}
pub(super) fn read(path: &Path) -> Result<fs::File> {
    let parent = directory(path.parent().ok_or("parent")?, false)?;
    let file = fs::File::from(
        sys::openat(
            &parent,
            path.file_name().ok_or("filename")?,
            OFlags::RDONLY | OFlags::NOFOLLOW | OFlags::CLOEXEC | OFlags::NONBLOCK,
            Mode::empty(),
        )
        .map_err(error)?,
    );
    if !file.metadata().map_err(error)?.is_file() {
        return Err("regular file required".into());
    }
    Ok(file)
}
pub(super) fn lock(path: &Path) -> Result<fs::File> {
    let dir = directory(path, false)?;
    // Lock the directory inode, avoiding replaceable lock-file names.
    sys::flock(&dir, sys::FlockOperation::LockExclusive).map_err(error)?;
    Ok(dir)
}
pub(super) fn write(path: &Path, bytes: &[u8]) -> Result<()> {
    let dir = directory(path.parent().ok_or("parent")?, false)?;
    let temp = format!(".{}.tmp", Uuid::new_v4());
    let mut file = fs::File::from(
        sys::openat(
            &dir,
            temp.as_str(),
            OFlags::WRONLY | OFlags::CREATE | OFlags::EXCL | OFlags::NOFOLLOW | OFlags::CLOEXEC,
            Mode::RUSR | Mode::WUSR,
        )
        .map_err(error)?,
    );
    let result = (|| {
        file.write_all(bytes).map_err(error)?;
        file.sync_all().map_err(error)?;
        sys::renameat(
            &dir,
            temp.as_str(),
            &dir,
            path.file_name().ok_or("filename")?,
        )
        .map_err(error)?;
        dir.sync_all().map_err(error)
    })();
    if result.is_err() {
        match sys::unlinkat(&dir, temp.as_str(), sys::AtFlags::empty()) {
            Ok(()) | Err(rustix::io::Errno::NOENT) => {}
            Err(_) => return Err("write failed; temporary cleanup failed".into()),
        }
    }
    result
}
