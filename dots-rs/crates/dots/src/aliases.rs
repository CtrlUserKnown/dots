use std::path::Path;
use anyhow::{bail, Context, Result};

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum AliasSource { BuiltIn, User }

#[derive(Debug, Clone)]
pub struct Alias {
    pub name:   String,
    pub value:  String,
    pub source: AliasSource,
}

// ── parsing ───────────────────────────────────────────────────────────────────

/// Parse a zsh aliases file into a flat list of Alias structs.
/// Source is always BuiltIn; callers override it for user aliases.
pub fn parse_alias_file(path: &Path) -> Vec<Alias> {
    if !path.exists() { return Vec::new(); }
    let Ok(text) = std::fs::read_to_string(path) else { return Vec::new() };
    let mut out = Vec::new();
    for line in text.lines() {
        let line = line.trim();
        if line.starts_with('#') || line.is_empty() { continue; }
        let Some(rest) = line.strip_prefix("alias ") else { continue };
        // Skip suffix/global aliases (alias -s, alias -g)
        if rest.starts_with('-') { continue; }
        let Some(eq) = rest.find('=') else { continue };
        let name = rest[..eq].trim().to_string();
        if name.is_empty() { continue; }
        // Only allow [a-zA-Z0-9_-] in alias names
        if !name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-') {
            continue;
        }
        let val_raw = rest[eq + 1..].trim();
        let value = strip_value(val_raw);
        out.push(Alias { name, value, source: AliasSource::BuiltIn });
    }
    out
}

fn strip_value(val_raw: &str) -> String {
    // Handle 'value' and "value" forms (find the matching closing quote)
    if let Some(c) = val_raw.chars().next() {
        if c == '\'' || c == '"' {
            if let Some(end) = val_raw[1..].find(c) {
                return val_raw[1..1 + end].to_string();
            }
        }
    }
    // No quotes: take up to `  #` trailing comment or end of line
    if let Some(pos) = val_raw.find("  #") {
        val_raw[..pos].trim().to_string()
    } else {
        val_raw.to_string()
    }
}

// ── load combined list ────────────────────────────────────────────────────────

pub fn load_all_aliases(dots_dir: &Path, personal_dir: &Path) -> Vec<Alias> {
    let builtin_path = dots_dir.join("src/zsh/zsh/.aliases");
    let user_path    = personal_dir.join("aliases.zsh");
    let mut all      = parse_alias_file(&builtin_path);
    let mut user     = parse_alias_file(&user_path);
    for a in &mut user { a.source = AliasSource::User; }
    all.extend(user);
    all
}

// ── user alias mutations ──────────────────────────────────────────────────────

pub fn add_user_alias(personal_dir: &Path, name: &str, value: &str) -> Result<()> {
    validate_alias_name(name)?;
    if value.is_empty() { bail!("alias value must not be empty"); }

    let path = personal_dir.join("aliases.zsh");

    if path.exists() {
        let existing = parse_alias_file(&path);
        if existing.iter().any(|a| a.name == name) {
            bail!("alias '{}' already exists in your personal aliases", name);
        }
    }

    let new_line = format!("alias {}='{}'\n", name, value);
    let content = if path.exists() {
        let mut base = std::fs::read_to_string(&path).context("reading aliases.zsh")?;
        if !base.ends_with('\n') { base.push('\n'); }
        base + &new_line
    } else {
        format!("# personal aliases — managed by dots\n{}", new_line)
    };

    write_atomic(&path, &content)
}

pub fn remove_user_alias(personal_dir: &Path, name: &str) -> Result<()> {
    let path = personal_dir.join("aliases.zsh");
    if !path.exists() {
        bail!("personal aliases file not found");
    }
    let text = std::fs::read_to_string(&path).context("reading aliases.zsh")?;
    let new_text: String = text.lines()
        .filter(|l| !l.trim().starts_with(&format!("alias {}=", name)))
        .map(|l| format!("{}\n", l))
        .collect();
    write_atomic(&path, &new_text)
}

pub fn edit_user_alias(personal_dir: &Path, name: &str, new_value: &str) -> Result<()> {
    validate_alias_name(name)?;
    if new_value.is_empty() { bail!("alias value must not be empty"); }

    let path = personal_dir.join("aliases.zsh");
    if !path.exists() {
        bail!("personal aliases file not found");
    }
    let text  = std::fs::read_to_string(&path).context("reading aliases.zsh")?;
    let mut found = false;
    let new_text: String = text.lines()
        .map(|l| {
            if !found && l.trim().starts_with(&format!("alias {}=", name)) {
                found = true;
                format!("alias {}='{}'\n", name, new_value)
            } else {
                format!("{}\n", l)
            }
        })
        .collect();
    if !found {
        bail!("alias '{}' not found in personal aliases", name);
    }
    write_atomic(&path, &new_text)
}

// ── helpers ───────────────────────────────────────────────────────────────────

fn validate_alias_name(name: &str) -> Result<()> {
    if name.is_empty() { bail!("alias name must not be empty"); }
    if name.contains(' ') { bail!("alias name must not contain spaces"); }
    if !name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-') {
        bail!("alias name must contain only [a-zA-Z0-9_-]");
    }
    Ok(())
}

fn write_atomic(path: &Path, content: &str) -> Result<()> {
    let tmp = path.with_extension("zsh.tmp");
    std::fs::write(&tmp, content).context("writing tmp file")?;
    std::fs::rename(&tmp, path).context("renaming tmp")?;
    Ok(())
}

// ── tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::PathBuf;
    use tempfile::tempdir;

    fn write_tmp(content: &str) -> PathBuf {
        let tmp = tempdir().unwrap();
        let p   = tmp.keep().join("aliases.zsh");
        fs::write(&p, content).unwrap();
        p
    }

    #[test]
    fn parse_aliases() {
        let p = write_tmp("alias la='ls -la'\nalias gst='git status'\n");
        let aliases = parse_alias_file(&p);
        assert_eq!(aliases.len(), 2);
        assert_eq!(aliases[0].name,  "la");
        assert_eq!(aliases[0].value, "ls -la");
        assert_eq!(aliases[1].name,  "gst");
        assert_eq!(aliases[1].value, "git status");
    }

    #[test]
    fn parse_double_quote_alias() {
        let p = write_tmp("alias la=\"ls -la\"  # list all\n");
        let aliases = parse_alias_file(&p);
        assert_eq!(aliases.len(), 1);
        assert_eq!(aliases[0].value, "ls -la");
    }

    #[test]
    fn parse_skips_suffix_aliases() {
        let p = write_tmp("alias -s md='$EDITOR'\nalias foo='bar'\n");
        let aliases = parse_alias_file(&p);
        assert_eq!(aliases.len(), 1);
        assert_eq!(aliases[0].name, "foo");
    }

    #[test]
    fn parse_skips_comments_and_blanks() {
        let p = write_tmp("# section\n\nalias ok='yes'\n");
        let aliases = parse_alias_file(&p);
        assert_eq!(aliases.len(), 1);
    }

    #[test]
    fn missing_file_returns_empty() {
        let aliases = parse_alias_file(Path::new("/nonexistent/path"));
        assert!(aliases.is_empty());
    }

    #[test]
    fn add_and_remove_user_alias() {
        let tmp  = tempdir().unwrap();
        let path = tmp.path().join("aliases.zsh");

        add_user_alias(tmp.path(), "foo", "echo foo").unwrap();
        let aliases = parse_alias_file(&path);
        assert!(aliases.iter().any(|a| a.name == "foo"), "alias not found after add");

        remove_user_alias(tmp.path(), "foo").unwrap();
        let aliases = parse_alias_file(&path);
        assert!(!aliases.iter().any(|a| a.name == "foo"), "alias still present after remove");
    }

    #[test]
    fn add_duplicate_fails() {
        let tmp = tempdir().unwrap();
        add_user_alias(tmp.path(), "dup", "first").unwrap();
        assert!(add_user_alias(tmp.path(), "dup", "second").is_err());
    }

    #[test]
    fn edit_user_alias() {
        let tmp = tempdir().unwrap();
        add_user_alias(tmp.path(), "myalias", "old value").unwrap();
        super::edit_user_alias(tmp.path(), "myalias", "new value").unwrap();
        let aliases = parse_alias_file(&tmp.path().join("aliases.zsh"));
        let a = aliases.iter().find(|a| a.name == "myalias").unwrap();
        assert_eq!(a.value, "new value");
    }

    #[test]
    fn invalid_name_with_space_rejected() {
        let tmp = tempdir().unwrap();
        assert!(add_user_alias(tmp.path(), "bad name", "val").is_err());
    }

    #[test]
    fn invalid_name_with_special_char_rejected() {
        let tmp = tempdir().unwrap();
        assert!(add_user_alias(tmp.path(), "bad!name", "val").is_err());
    }
}
