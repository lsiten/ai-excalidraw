use super::{
    fs::{components, validate_tree, Tree},
    Result,
};
pub fn name(value: &str) -> Result<()> {
    if value.is_empty()
        || value.len() > 64
        || value.starts_with('-')
        || value.ends_with('-')
        || value.contains("--")
        || !value
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
    {
        return Err("invalid Skill/memory name".into());
    }
    Ok(())
}
/// Supported frontmatter: scalar name and scalar or block description. Reject ambiguous YAML.
pub fn metadata(text: &str) -> Result<(String, String)> {
    let text = text.replace("\r\n", "\n");
    let rest = text.strip_prefix("---\n").ok_or("frontmatter required")?;
    let (header, _) = rest
        .split_once("\n---\n")
        .ok_or("frontmatter terminator required")?;
    if header.len() > 8192 {
        return Err("frontmatter too large".into());
    }
    let mut fields = std::collections::BTreeMap::new();
    let lines: Vec<_> = header.lines().collect();
    let mut i = 0;
    while i < lines.len() {
        let line = lines[i];
        i += 1;
        if line.trim().is_empty() || line.starts_with('#') {
            continue;
        }
        let (key, raw) = line.split_once(':').ok_or("invalid frontmatter field")?;
        if key.trim() != key
            || key.is_empty()
            || !key.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-')
        {
            return Err("invalid frontmatter key".into());
        }
        // Optional Skill metadata may contain nested YAML; only name/description are interpreted.
        if key != "name" && key != "description" {
            while i < lines.len() && (lines[i].starts_with([' ', '\t']) || lines[i].is_empty()) {
                i += 1;
            }
            continue;
        }
        let raw = raw.trim();
        let value = if matches!(raw, "|" | ">" | "|-" | ">-") {
            let mut parts = Vec::new();
            while i < lines.len() && (lines[i].starts_with(' ') || lines[i].is_empty()) {
                parts.push(lines[i].trim());
                i += 1;
            }
            parts.join(if raw.starts_with('>') { " " } else { "\n" })
        } else if raw.starts_with('"') {
            serde_json::from_str::<String>(raw).map_err(|_| "invalid quoted frontmatter scalar")?
        } else if raw.starts_with('\'') {
            raw.strip_prefix('\'')
                .and_then(|s| s.strip_suffix('\''))
                .ok_or("invalid quoted scalar")?
                .replace("''", "'")
        } else {
            if raw.starts_with(['[', '{', '&', '*', '!']) || raw.contains(": ") {
                return Err("unsupported frontmatter scalar".into());
            }
            raw.split(" #").next().unwrap_or("").trim().to_owned()
        };
        if fields.insert(key.to_owned(), value).is_some() {
            return Err("duplicate frontmatter field".into());
        }
    }
    let n = fields.remove("name").ok_or("name required")?;
    name(&n)?;
    let d = fields
        .remove("description")
        .filter(|s| !s.trim().is_empty())
        .ok_or("description required")?;
    if d.len() > 2048 {
        return Err("description too long".into());
    }
    Ok((n, d))
}
fn reference(reference: &str, tree: &Tree) -> Result<()> {
    let reference = reference.trim().trim_matches(['<', '>']);
    if reference.starts_with('#')
        || reference.starts_with("https://")
        || reference.starts_with("http://")
        || reference.starts_with("mailto:")
    {
        return Ok(());
    }
    let path = reference.split(['#', '?']).next().unwrap_or("");
    let path = path.strip_prefix("./").unwrap_or(path);
    components(path)?;
    if !tree.contains_key(path) {
        return Err(format!("missing referenced resource: {path}"));
    }
    Ok(())
}
pub fn validate(n: &str, tree: &Tree) -> Result<()> {
    validate_tree(tree)?;
    let text = tree.get("SKILL.md").ok_or("SKILL.md required")?;
    if metadata(text)?.0 != n {
        return Err("frontmatter name mismatch".into());
    }
    for fragment in text.split("](").skip(1) {
        let (target, _) = fragment
            .split_once(')')
            .ok_or("unterminated resource reference")?;
        let target = target.split(" \"").next().unwrap_or(target);
        reference(target, tree)?;
    }
    for line in text.lines() {
        if line.trim_start().starts_with('[') {
            if let Some((_, target)) = line.split_once("]:") {
                reference(target.trim().split(" \"").next().unwrap_or(""), tree)?;
            }
        }
    }
    for (i, span) in text.split('`').enumerate() {
        if i % 2 == 1
            && !span.contains(char::is_whitespace)
            && ["references/", "scripts/", "assets/"]
                .iter()
                .any(|p| span.starts_with(p))
        {
            reference(span, tree)?;
        }
    }
    Ok(())
}
