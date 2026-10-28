use super::{
    files,
    model::{Context, ToolId},
    SKILL_NAME,
};
use std::path::{Path, PathBuf};

pub(super) fn resolve(data: &Path, project_id: Option<&str>) -> Result<Option<PathBuf>, String> {
    project_id
        .map(|id| {
            let root = crate::workspace::project_root(data, id)?;
            files::safe_path(&root)?;
            if !root.is_dir() {
                return Err("项目目录不可用，请重新打开".into());
            }
            Ok(root)
        })
        .transpose()
}

pub(super) fn paths(context: &Context, id: ToolId, root: Option<&Path>) -> (PathBuf, PathBuf) {
    match root {
        Some(root) => {
            let agents = root.join(".agents");
            (
                agents.join("mcp.json"),
                agents.join("skills").join(SKILL_NAME),
            )
        }
        None => context.paths(id),
    }
}
