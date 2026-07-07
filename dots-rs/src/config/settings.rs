use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct DotsSettings {
    pub update_check: bool,
    pub update_frequency: u64,
    pub greeting: bool,
    pub developer_mode: bool,
    pub theme: String,
}

impl Default for DotsSettings {
    fn default() -> Self {
        Self {
            update_check: true,
            update_frequency: 1440,
            greeting: true,
            developer_mode: false,
            theme: String::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct SsmSettings {
    pub use_herdr: bool,
}

impl Default for SsmSettings {
    fn default() -> Self {
        Self { use_herdr: true }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Settings {
    #[serde(default)]
    pub dots: DotsSettings,
    #[serde(default)]
    pub ssm: SsmSettings,
}

pub fn dots_dir() -> PathBuf {
    dirs::home_dir()
        .expect("home dir must exist")
        .join(".dots")
}

pub fn settings_path() -> PathBuf {
    dots_dir().join("settings.toml")
}

pub fn load() -> Result<Settings> {
    let path = settings_path();
    if !path.exists() {
        return Ok(Settings::default());
    }
    let text = std::fs::read_to_string(&path)
        .with_context(|| format!("reading {}", path.display()))?;
    // Unknown keys are ignored via deny_unknown_fields absence; missing keys fall back to Default.
    match toml::from_str::<Settings>(&text) {
        Ok(s) => Ok(s),
        Err(e) => {
            eprintln!("warning: settings.toml is malformed ({}); using defaults", e);
            Ok(Settings::default())
        }
    }
}

pub fn save(s: &Settings) -> Result<()> {
    let path = settings_path();
    let dir = path.parent().expect("settings path has parent");
    std::fs::create_dir_all(dir)
        .with_context(|| format!("creating {}", dir.display()))?;

    let tmp = path.with_extension("toml.tmp");
    let text = toml::to_string_pretty(s).context("serializing settings")?;

    // Overwrite any leftover tmp from a previous crashed write.
    std::fs::write(&tmp, &text)
        .with_context(|| format!("failed to save settings: {}: writing tmp", path.display()))?;
    std::fs::rename(&tmp, &path)
        .with_context(|| format!("failed to save settings: {}: renaming tmp", path.display()))?;

    Ok(())
}

/// Merge `overlay` on top of `base` — overlay values win.
pub fn merge(base: Settings, overlay: Settings) -> Settings {
    Settings {
        dots: DotsSettings {
            update_check: overlay.dots.update_check,
            update_frequency: overlay.dots.update_frequency,
            greeting: overlay.dots.greeting,
            developer_mode: overlay.dots.developer_mode,
            theme: if overlay.dots.theme.is_empty() {
                base.dots.theme
            } else {
                overlay.dots.theme
            },
        },
        ssm: SsmSettings {
            use_herdr: overlay.ssm.use_herdr,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn roundtrip() {
        let tmp = tempdir().unwrap();
        let path = tmp.path().join("settings.toml");

        let mut s = Settings::default();
        s.dots.greeting = false;
        s.dots.update_frequency = 360;
        s.ssm.use_herdr = false;

        let text = toml::to_string_pretty(&s).unwrap();
        std::fs::write(&path, &text).unwrap();

        let loaded: Settings = toml::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
        assert!(!loaded.dots.greeting);
        assert_eq!(loaded.dots.update_frequency, 360);
        assert!(!loaded.ssm.use_herdr);
    }

    #[test]
    fn missing_file_returns_defaults() {
        let defaults = Settings::default();
        // Simulate missing file path via direct deserialization of empty string.
        let loaded: Settings = toml::from_str("").unwrap();
        assert_eq!(loaded.dots.greeting, defaults.dots.greeting);
        assert_eq!(loaded.dots.update_check, defaults.dots.update_check);
        assert_eq!(loaded.dots.update_frequency, defaults.dots.update_frequency);
        assert_eq!(loaded.ssm.use_herdr, defaults.ssm.use_herdr);
    }

    #[test]
    fn merge_user_wins() {
        let mut base = Settings::default();
        base.dots.greeting = false;

        let mut overlay = Settings::default();
        overlay.dots.greeting = true;

        let result = merge(base, overlay);
        assert!(result.dots.greeting);
    }

    #[test]
    fn invalid_toml_returns_err() {
        let result: Result<Settings, _> = toml::from_str("NOT VALID TOML }{{{");
        assert!(result.is_err());
    }
}
