use super::{io, projects};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{collections::BTreeMap, path::Path};

#[derive(Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PresentationData {
    #[serde(default)]
    pub decks: BTreeMap<String, PresentationDeck>,
    #[serde(default)]
    pub areas: BTreeMap<String, PresentationArea>,
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PresentationDeck {
    pub order: Vec<String>,
    pub include_subfolders: bool,
}

#[derive(Clone, Copy, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PresentationArea {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

fn validate_path(root: &Path, name: &str, canvas: bool) -> io::Result<()> {
    if name.len() > 4096
        || name.chars().any(char::is_control)
        || name.split('/').any(|part| part.is_empty() || part == ".")
    {
        return Err("演示路径无效或超过 4096 字节".into());
    }
    if canvas {
        io::canvas_path(root, name)?;
    } else {
        io::relative(root, name)?;
    }
    Ok(())
}

impl PresentationData {
    pub fn validate(&self, root: &Path) -> io::Result<()> {
        if self.decks.len() > 5000 || self.areas.len() > 5000 {
            return Err("演示配置最多支持 5000 个目录和画布区域".into());
        }
        let mut remaining = 25_000_usize;
        for (folder, deck) in &self.decks {
            if !folder.is_empty() {
                validate_path(root, folder, false)?;
            }
            remaining = remaining
                .checked_sub(deck.order.len())
                .ok_or("演示排序总计不能超过 25000 项")?;
            for path in &deck.order {
                validate_path(root, path, true)?;
            }
        }
        for (canvas, area) in &self.areas {
            validate_path(root, canvas, true)?;
            if [area.x, area.y, area.width, area.height]
                .iter()
                .any(|value| !value.is_finite() || value.abs() > 1_000_000_000.0)
                || area.width <= 0.0
                || area.height <= 0.0
            {
                return Err("演示区域需要有限坐标及正宽高，绝对值不能超过十亿".into());
            }
        }
        Ok(())
    }

    pub fn move_paths(&mut self, from: &str, to: Option<&str>) {
        move_keys(&mut self.decks, from, to);
        move_keys(&mut self.areas, from, to);
        for deck in self.decks.values_mut() {
            deck.order.retain_mut(|path| {
                if is_within(path, from) {
                    if let Some(to) = to {
                        *path = format!("{to}{}", &path[from.len()..]);
                    } else {
                        return false;
                    }
                }
                true
            });
        }
    }
}

fn is_within(path: &str, from: &str) -> bool {
    path == from
        || path
            .strip_prefix(from)
            .is_some_and(|tail| tail.starts_with('/'))
}

pub(super) fn move_keys<T>(entries: &mut BTreeMap<String, T>, from: &str, to: Option<&str>) {
    let keys: Vec<_> = entries
        .keys()
        .filter(|key| is_within(key, from))
        .cloned()
        .collect();
    for key in keys {
        if let Some(value) = entries.remove(&key) {
            if let Some(to) = to {
                entries.insert(format!("{to}{}", &key[from.len()..]), value);
            }
        }
    }
}

pub fn read(root: &Path) -> io::Result<Value> {
    serde_json::to_value(projects::load_metadata(root)?.presentation).map_err(|e| e.to_string())
}

pub fn write(root: &Path, presentation: PresentationData) -> io::Result<Value> {
    presentation.validate(root)?;
    let mut metadata = projects::load_metadata(root)?;
    metadata.presentation = presentation;
    projects::save_metadata(root, &metadata)?;
    Ok(json!({"saved": true}))
}
