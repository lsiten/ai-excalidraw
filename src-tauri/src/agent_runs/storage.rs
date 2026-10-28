use super::*;
pub(super) fn safe(path: &Path) -> Result<()> {
    for ancestor in path.ancestors() {
        match fs::symlink_metadata(ancestor) {
            Ok(m) if m.file_type().is_symlink() => return Err("symlink rejected".into()),
            Ok(_) => {}
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(_) => return Err("path inspection failed".into()),
        }
    }
    Ok(())
}
pub(super) fn directory(data: &Path, project: Option<&str>) -> Result<PathBuf> {
    let root = match project {
        Some(id) => crate::workspace::project_root(data, id)?.join(".ai-excalidraw"),
        None => data.to_owned(),
    };
    let dir = root.join("runs");
    safe(&dir)?;
    #[cfg(unix)]
    unix_io::directory(&dir, true)?;
    #[cfg(not(unix))]
    fs::create_dir_all(&dir).map_err(|_| "create directory")?;
    safe(&dir)?;
    Ok(dir)
}
pub(super) fn save(path: &Path, run: &Run) -> Result<()> {
    let bytes = serde_json::to_vec(run).map_err(|_| "encode")?;
    if bytes.len() > LIMIT as usize {
        return Err("run limit".into());
    }
    safe(path)?;
    #[cfg(unix)]
    {
        unix_io::write(path, &bytes)
    }
    #[cfg(not(unix))]
    {
        let parent = path.parent().ok_or("parent")?;
        let mut tmp = tempfile::NamedTempFile::new_in(parent).map_err(|_| "temporary file")?;
        tmp.write_all(&bytes).map_err(|_| "write")?;
        tmp.as_file().sync_all().map_err(|_| "sync")?;
        safe(path)?;
        tmp.persist(path).map_err(|_| "replace")?;
        fs::File::open(parent)
            .and_then(|f| f.sync_all())
            .map_err(|_| "directory sync")?;
        Ok(())
    }
}
pub(super) fn load(path: &Path) -> Result<Run> {
    safe(path)?;
    #[cfg(unix)]
    let f = unix_io::read(path)?;
    #[cfg(not(unix))]
    let f = fs::File::open(path).map_err(|_| "run not found")?;
    let mut bytes = Vec::new();
    f.take(LIMIT + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| "read")?;
    if bytes.len() > LIMIT as usize {
        return Err("run limit".into());
    }
    let run: Run = serde_json::from_slice(&bytes).map_err(|_| "invalid run")?;
    if path
        .file_stem()
        .and_then(|s| s.to_str())
        .and_then(|s| Uuid::parse_str(s).ok())
        != Some(run.header.id)
    {
        return Err("run filename identity mismatch".into());
    }
    for (i, e) in run.events.iter().enumerate() {
        if e.run_id != run.header.id || e.seq != i as u64 + 1 {
            return Err("invalid journal".into());
        }
    }
    Ok(run)
}
