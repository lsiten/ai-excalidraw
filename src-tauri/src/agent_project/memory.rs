//! High-confidence credential guard, not exhaustive secret detection.
use super::Result;

fn placeholder(value: &str) -> bool {
    (value.starts_with('<') && value.ends_with('>'))
        || value.starts_with("${")
        || value.starts_with("process.env.")
        || value.starts_with("os.environ")
        || matches!(
            value.trim_end_matches('}').to_ascii_lowercase().as_str(),
            "" | "redacted"
                | "[redacted]"
                | "null"
                | "none"
                | "unset"
                | "true"
                | "false"
                | "your_api_key"
                | "your_token"
        )
}

fn assigned_credential(text: &str) -> bool {
    for (position, separator) in text.char_indices() {
        if !matches!(separator, '=' | ':') {
            continue;
        }
        let before = text[..position].trim_end().trim_end_matches(['\'', '"']);
        let key = before
            .rsplit(|c: char| !c.is_ascii_alphanumeric() && c != '_' && c != '-')
            .next()
            .unwrap_or("");
        let key: String = key
            .bytes()
            .filter(|b| b.is_ascii_alphanumeric())
            .map(|b| char::from(b.to_ascii_lowercase()))
            .collect();
        let sensitive = [
            "apikey",
            "accesstoken",
            "authtoken",
            "clientsecret",
            "secretaccesskey",
            "password",
            "passwd",
        ]
        .iter()
        .any(|suffix| key.ends_with(suffix))
            || matches!(key.as_str(), "token" | "secret");
        if !sensitive {
            continue;
        }
        let value = text[position + 1..]
            .trim_start()
            .trim_start_matches(['\'', '"']);
        let value = value
            .split(|c: char| c.is_whitespace() || matches!(c, '\'' | '"' | ',' | ';'))
            .next()
            .unwrap_or("");
        if !value.is_empty() && !placeholder(value) {
            return true;
        }
    }
    false
}

pub fn validate(content: &str) -> Result<()> {
    if content.trim().is_empty() || content.len() > 8192 {
        return Err("confirmed fact must be 1..8192 UTF-8 bytes".into());
    }
    let lower = content.to_ascii_lowercase();
    let private_key = [
        "private key",
        "rsa private key",
        "ec private key",
        "dsa private key",
        "openssh private key",
        "encrypted private key",
        "pgp private key block",
    ]
    .iter()
    .any(|kind| lower.contains(&format!("-----begin {kind}-----")));
    let bearer = lower.match_indices("bearer").any(|(position, _)| {
        let before = &lower[..position];
        if before
            .chars()
            .next_back()
            .is_some_and(|c| c.is_ascii_alphanumeric() || c == '_')
        {
            return false;
        }
        let after = &content[position + 6..];
        if !after.starts_with(char::is_whitespace) {
            return false;
        }
        let token = after
            .trim_start()
            .split(|c: char| c.is_whitespace() || matches!(c, '\'' | '"' | ',' | ';'))
            .next()
            .unwrap_or("");
        if token.is_empty() || placeholder(token) {
            return false;
        }
        let header: String = before
            .chars()
            .filter(|c| !c.is_whitespace() && !matches!(c, '\'' | '"'))
            .collect();
        let header = header.ends_with("authorization:") || header.ends_with("authorization=");
        header
            || token.len() >= 24
            || (token.len() >= 8
                && token.bytes().any(|b| {
                    b.is_ascii_digit() || matches!(b, b'.' | b'_' | b'-' | b'/' | b'+' | b'=')
                }))
    });
    if private_key || bearer || assigned_credential(content) {
        // Never include the matched value in errors, receipts or logs.
        return Err(
            "memory rejected: apparent credential material; store only non-secret confirmed facts"
                .into(),
        );
    }
    Ok(())
}
