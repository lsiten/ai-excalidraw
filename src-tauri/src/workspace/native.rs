use super::io::Result;
use std::process::Command;

pub fn apple_script(script: &str, args: &[&str]) -> Result<String> {
    let output = Command::new("/usr/bin/osascript")
        .arg("-e")
        .arg(script)
        .arg("--")
        .args(args)
        .output()
        .map_err(|e| e.to_string())?;
    if !output.status.success() {
        let error = String::from_utf8_lossy(&output.stderr);
        if error.contains("-128") {
            return Err("CANCELLED".into());
        }
        return Err(error.into_owned());
    }
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_owned())
}
pub fn pick(kind: &str) -> Result<String> {
    let script = match kind {
        "directory" => "POSIX path of (choose folder with prompt \"选择 AI Excalidraw 项目目录\")",
        "file" => "POSIX path of (choose file with prompt \"打开 .excalidraw 画布\")",
        "save" => "POSIX path of (choose file name with prompt \"导出画布\" default name \"画布.excalidraw\")",
        _ => return Err("未知的文件选择方式".into()),
    };
    match apple_script(script, &[]) {
        Err(error) if error == "CANCELLED" => Ok(String::new()),
        result => result,
    }
}
pub fn trash(path: &str) -> Result<()> {
    apple_script(
        "on run argv\ntell application \"Finder\" to delete (POSIX file (item 1 of argv) as alias)\nreturn \"trashed\"\nend run",
        &[path],
    )
    .map(|_| ())
}
pub fn reveal(path: &str) -> Result<()> {
    let status = Command::new("/usr/bin/open")
        .arg("-R")
        .arg(path)
        .status()
        .map_err(|e| e.to_string())?;
    if status.success() {
        Ok(())
    } else {
        Err("无法在 Finder 中显示".into())
    }
}
