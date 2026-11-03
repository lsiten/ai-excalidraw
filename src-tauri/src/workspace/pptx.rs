use super::{io, native};
use serde_json::{json, Value};
use std::path::{Path, PathBuf};

fn validate(bytes: &[u8]) -> io::Result<()> {
    if !bytes.starts_with(b"PK\x03\x04") {
        return Err("PPT 文件内容无效".into());
    }
    if bytes.len() > 64 * 1024 * 1024 {
        return Err("PPT 超过 64 MB，请拆分目录后导出".into());
    }
    Ok(())
}

fn save(selected: &Path, bytes: &[u8]) -> io::Result<PathBuf> {
    validate(bytes)?;
    let mut path = selected.to_path_buf();
    if path
        .extension()
        .is_none_or(|ext| !ext.eq_ignore_ascii_case("pptx"))
    {
        path.set_extension("pptx");
    }
    if path != selected && path.exists() {
        return Err("目标 PPT 已存在，请在保存对话框中直接选择该 .pptx 文件以确认替换".into());
    }
    io::atomic_write(&path, bytes)?;
    Ok(path)
}

pub fn export(name: &str, bytes: &[u8]) -> io::Result<Value> {
    validate(bytes)?;
    if name.is_empty() || name.contains(['/', '\\', ':', '\0']) || name.len() > 255 {
        return Err("PPT 文件名无效".into());
    }
    let selected = match native::apple_script(
        "on run argv\nreturn POSIX path of (choose file name with prompt \"导出 PowerPoint 演示文稿\" default name (item 1 of argv))\nend run",
        &[name],
    ) {
        Err(error) if error == "CANCELLED" => return Ok(json!({"cancelled": true})),
        result => result?,
    };
    Ok(json!({"path": save(Path::new(&selected), bytes)?}))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn writes_pptx_bytes_and_normalizes_extension() -> Result<(), Box<dyn std::error::Error>> {
        let directory = tempfile::tempdir()?;
        let payload = b"PK\x03\x04presentation";
        let path = save(&directory.path().join("课程.ppt"), payload)?;
        assert_eq!(path, directory.path().join("课程.pptx"));
        assert_eq!(std::fs::read(path)?, payload);
        assert_eq!(std::fs::read_dir(directory.path())?.count(), 1);
        Ok(())
    }

    #[test]
    fn invalid_content_leaves_existing_file_unchanged() -> Result<(), Box<dyn std::error::Error>> {
        let directory = tempfile::tempdir()?;
        let path = directory.path().join("existing.pptx");
        std::fs::write(&path, b"previous presentation")?;
        assert!(save(&path, b"not a presentation").is_err());
        assert_eq!(std::fs::read(path)?, b"previous presentation");
        Ok(())
    }

    #[test]
    fn extension_change_does_not_replace_an_unconfirmed_target(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let directory = tempfile::tempdir()?;
        let path = directory.path().join("existing.pptx");
        std::fs::write(&path, b"previous presentation")?;
        assert!(save(&directory.path().join("existing.ppt"), b"PK\x03\x04new").is_err());
        assert_eq!(std::fs::read(path)?, b"previous presentation");
        Ok(())
    }
}
