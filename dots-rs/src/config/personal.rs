use anyhow::Result;
use std::path::PathBuf;

use super::settings::{self, Settings};

pub fn personal_dir() -> PathBuf {
    dirs::home_dir()
        .expect("home dir must exist")
        .join(".personal")
}

pub fn aliases_path() -> PathBuf {
    personal_dir().join("aliases.zsh")
}

pub fn apps_dir() -> PathBuf {
    personal_dir().join("apps")
}

pub fn ensure_personal_dir() -> Result<()> {
    let dir = personal_dir();
    std::fs::create_dir_all(&dir)?;
    std::fs::create_dir_all(apps_dir())?;
    generate_aliases_stub()?;
    Ok(())
}

pub fn generate_aliases_stub() -> Result<()> {
    let path = aliases_path();
    if path.exists() {
        return Ok(());
    }
    let stub = "# ~/.personal/aliases.zsh — user-defined aliases\n\
                # sourced after dots default aliases\n\
                # example: alias ll='ls -lah'\n";
    std::fs::write(&path, stub)?;
    Ok(())
}

/// Load user overrides from ~/.personal/config.toml.
/// Returns a Settings with only the keys present in the file set;
/// unknown keys are silently ignored (toml #[serde(default)] handles this).
pub fn load_user_overrides() -> Result<Settings> {
    let path = personal_dir().join("config.toml");
    if !path.exists() {
        return Ok(Settings::default());
    }
    let text = std::fs::read_to_string(&path)?;
    match toml::from_str::<Settings>(&text) {
        Ok(s) => Ok(s),
        Err(e) => {
            eprintln!(
                "warning: ~/.personal/config.toml is malformed ({}); ignoring overrides",
                e
            );
            Ok(Settings::default())
        }
    }
}

/// Load base settings and merge user overrides on top.
pub fn load_merged() -> Result<Settings> {
    let base = settings::load()?;
    let overrides = load_user_overrides()?;
    Ok(settings::merge(base, overrides))
}
