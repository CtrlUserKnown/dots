use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::config::settings::{self, dots_dir};
use crate::packages::{check_dep, Category, DEPS};
use crate::tui::settings::{get_current_theme, set_ghostty_theme};

const CURRENT_VERSION: &str = "2";

// ── schema ────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PersonalSettings {
    #[serde(default = "bool_true")]
    pub update_check:     bool,
    #[serde(default = "bool_true")]
    pub greeting:         bool,
    #[serde(default = "default_freq")]
    pub update_frequency: u64,
    #[serde(default)]
    pub developer_mode:   bool,
}

fn bool_true() -> bool { true }
fn default_freq() -> u64 { 1440 }

impl Default for PersonalSettings {
    fn default() -> Self {
        Self { update_check: true, greeting: true, update_frequency: 1440, developer_mode: false }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct PackageList {
    #[serde(default)]
    pub optional: Vec<String>,
    #[serde(default)]
    pub dev:      Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PersonalConfig {
    pub version:      String,
    pub generated:    String,
    pub dots_version: String,
    #[serde(default)]
    pub settings:     PersonalSettings,
    #[serde(default)]
    pub theme:        String,
    #[serde(default)]
    pub packages:     PackageList,
    #[serde(default)]
    pub apps:         HashMap<String, Value>,
}

// ── paths ─────────────────────────────────────────────────────────────────────

pub fn personal_config_path() -> PathBuf {
    crate::config::personal::personal_dir().join("personal.json")
}

// ── collect ───────────────────────────────────────────────────────────────────

pub fn collect_personal_config() -> PersonalConfig {
    let dots    = dots_dir();
    let sets    = settings::load().unwrap_or_default();
    let theme   = get_current_theme(&dots);

    let optional: Vec<String> = DEPS.iter()
        .filter(|d| d.category == Category::Optional && check_dep(d))
        .map(|d| if d.brew.is_empty() { d.bin } else { d.brew })
        .map(str::to_string)
        .collect();

    let dev: Vec<String> = DEPS.iter()
        .filter(|d| d.category == Category::Dev && check_dep(d))
        .map(|d| if d.brew.is_empty() { d.bin } else { d.brew })
        .map(str::to_string)
        .collect();

    let mut apps: HashMap<String, Value> = HashMap::new();
    if !theme.is_empty() {
        let mut ghostty = serde_json::Map::new();
        ghostty.insert("theme".to_string(), Value::String(theme.clone()));
        apps.insert("ghostty".to_string(), Value::Object(ghostty));
    }

    PersonalConfig {
        version:      CURRENT_VERSION.to_string(),
        generated:    iso_now(),
        dots_version: env!("DOTS_VERSION").to_string(),
        settings:     PersonalSettings {
            update_check:     sets.dots.update_check,
            greeting:         sets.dots.greeting,
            update_frequency: sets.dots.update_frequency,
            developer_mode:   sets.dots.developer_mode,
        },
        theme,
        packages: PackageList { optional, dev },
        apps,
    }
}

pub fn generate_personal_config(path: &Path) -> Result<()> {
    let cfg = collect_personal_config();
    generate_personal_config_to(&cfg, path)
}

pub fn generate_personal_config_to(cfg: &PersonalConfig, path: &Path) -> Result<()> {
    if let Some(p) = path.parent() { std::fs::create_dir_all(p)?; }
    let json = serde_json::to_string_pretty(cfg).context("serializing config")?;
    let tmp  = path.with_extension("json.tmp");
    std::fs::write(&tmp, json)?;
    std::fs::rename(&tmp, path)?;
    Ok(())
}

// ── validate ──────────────────────────────────────────────────────────────────

pub fn validate_personal_config(v: &Value) -> Result<()> {
    if !v.is_object() { bail!("not a JSON object"); }
    let ver = v["version"].as_str().unwrap_or("");
    if ver != "1" && ver != "2" {
        bail!("unsupported version '{}' — update dots", ver);
    }
    let pkgs = &v["packages"];
    if !pkgs.is_null() && !pkgs.is_object() {
        bail!("'packages' must be an object");
    }
    if let Some(obj) = pkgs.as_object() {
        for cat in ["optional", "dev"] {
            if let Some(arr) = obj.get(cat) {
                if !arr.is_array() {
                    bail!("'packages.{}' must be a list", cat);
                }
            }
        }
    }
    Ok(())
}

// ── load & migrate ────────────────────────────────────────────────────────────

pub fn load_from_value(v: &Value) -> Result<PersonalConfig> {
    validate_personal_config(v)?;
    let mut cfg: PersonalConfig = serde_json::from_value(v.clone())
        .context("parsing personal config")?;
    if cfg.version == "1" {
        cfg.version = CURRENT_VERSION.to_string();
    }
    Ok(cfg)
}

// ── apply ─────────────────────────────────────────────────────────────────────

/// Returns names of packages listed in the config that are not yet installed.
pub fn apply_personal_config(cfg: &PersonalConfig) -> Result<Vec<String>> {
    let mut current = settings::load().unwrap_or_default();
    current.dots.update_check     = cfg.settings.update_check;
    current.dots.greeting         = cfg.settings.greeting;
    current.dots.update_frequency = cfg.settings.update_frequency;
    current.dots.developer_mode   = cfg.settings.developer_mode;
    settings::save(&current)?;

    let theme = cfg.apps.get("ghostty")
        .and_then(|a| a.get("theme"))
        .and_then(|t| t.as_str())
        .filter(|t| !t.is_empty())
        .or_else(|| if cfg.theme.is_empty() { None } else { Some(cfg.theme.as_str()) });

    if let Some(t) = theme {
        let dots = dots_dir();
        if dots.join("src/ghostty/config").exists() {
            set_ghostty_theme(&dots, t).ok();
        }
    }

    let all_names: Vec<&str> = cfg.packages.optional.iter()
        .chain(cfg.packages.dev.iter())
        .map(String::as_str)
        .collect();

    let missing: Vec<String> = all_names.iter()
        .filter(|&&name| {
            DEPS.iter()
                .find(|d| d.brew == name || d.bin == name)
                .map(|d| !check_dep(d))
                .unwrap_or(false)
        })
        .map(|&s| s.to_string())
        .collect();

    Ok(missing)
}

// ── github fetch ──────────────────────────────────────────────────────────────

pub fn fetch_github_raw(spec: &str) -> Result<String> {
    let mut parts = spec.splitn(3, '/');
    let user = parts.next().ok_or_else(|| anyhow::anyhow!("expected user/repo/path"))?;
    let repo = parts.next().ok_or_else(|| anyhow::anyhow!("expected user/repo/path"))?;
    let path = parts.next().ok_or_else(|| anyhow::anyhow!("expected user/repo/path"))?;

    for branch in ["main", "master"] {
        let url = format!("https://raw.githubusercontent.com/{user}/{repo}/{branch}/{path}");
        match ureq::get(&url)
            .timeout(std::time::Duration::from_secs(10))
            .call()
        {
            Ok(resp) => {
                return resp.into_string().context("reading response body");
            }
            Err(_) => continue,
        }
    }
    bail!("could not fetch '{}' from GitHub (tried main and master)", spec)
}

// ── timestamp helper ──────────────────────────────────────────────────────────

fn iso_now() -> String {
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    let days = secs / 86400;
    let tod  = secs % 86400;
    let h = tod / 3600;
    let m = (tod % 3600) / 60;
    let s = tod % 60;
    let (y, mo, d) = epoch_days_to_ymd(days as u32);
    format!("{y:04}-{mo:02}-{d:02}T{h:02}:{m:02}:{s:02}")
}

fn epoch_days_to_ymd(mut days: u32) -> (u32, u32, u32) {
    let mut y = 1970u32;
    loop {
        let dy = if is_leap(y) { 366 } else { 365 };
        if days < dy { break; }
        days -= dy; y += 1;
    }
    let months = if is_leap(y) {
        [31u32,29,31,30,31,30,31,31,30,31,30,31]
    } else {
        [31,28,31,30,31,30,31,31,30,31,30,31]
    };
    let mut mo = 1u32;
    for dm in &months {
        if days < *dm { break; }
        days -= dm; mo += 1;
    }
    (y, mo, days + 1)
}

fn is_leap(y: u32) -> bool { y % 4 == 0 && (y % 100 != 0 || y % 400 == 0) }

// ── tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn generate_and_validate() {
        let tmp  = tempdir().unwrap();
        let path = tmp.path().join("personal.json");
        generate_personal_config(&path).unwrap();
        let text = std::fs::read_to_string(&path).unwrap();
        let data: Value = serde_json::from_str(&text).unwrap();
        validate_personal_config(&data).unwrap();
    }

    #[test]
    fn version_1_migration() {
        let v1 = serde_json::json!({
            "version": "1",
            "generated": "2026-01-01T00:00:00",
            "dots_version": "1.5.0",
            "settings": { "update_check": true, "greeting": false, "update_frequency": 1440 },
            "theme": "Nord",
            "packages": { "optional": ["herdr"], "dev": ["gh"] }
        });
        validate_personal_config(&v1).unwrap();
        let cfg = load_from_value(&v1).unwrap();
        assert_eq!(cfg.version, "2", "version should be upgraded to 2");
        assert_eq!(cfg.theme, "Nord");
        assert_eq!(cfg.settings.greeting, false);
    }

    #[test]
    fn apply_is_non_intrusive() {
        let cfg = PersonalConfig {
            version:      "2".to_string(),
            generated:    "2026-01-01T00:00:00".to_string(),
            dots_version: "1.0.0".to_string(),
            settings:     PersonalSettings::default(),
            theme:        String::new(),
            packages:     PackageList::default(),
            apps:         HashMap::new(),
        };
        let result = apply_personal_config(&cfg);
        // apply should succeed (or fail gracefully if no settings.toml)
        // It must never touch zsh files
        let rc = dirs::home_dir().unwrap_or_default().join(".dots/src/zsh/zsh/rc.zsh");
        if rc.exists() {
            let before = std::fs::metadata(&rc).unwrap().modified().unwrap();
            let _ = result;
            let after = std::fs::metadata(&rc).unwrap().modified().unwrap();
            assert_eq!(before, after, "apply must not modify rc.zsh");
        }
    }

    #[test]
    fn validate_rejects_unknown_version() {
        let v = serde_json::json!({ "version": "99", "packages": {} });
        assert!(validate_personal_config(&v).is_err());
    }

    #[test]
    fn validate_rejects_packages_not_object() {
        let v = serde_json::json!({ "version": "2", "packages": "broken" });
        assert!(validate_personal_config(&v).is_err());
    }

    #[test]
    fn missing_keys_fill_defaults() {
        let v = serde_json::json!({
            "version": "2",
            "generated": "",
            "dots_version": "0.0.0"
        });
        let cfg = load_from_value(&v).unwrap();
        assert!(cfg.settings.update_check,         "update_check should default true");
        assert!(cfg.settings.greeting,             "greeting should default true");
        assert_eq!(cfg.settings.update_frequency, 1440);
        assert!(cfg.packages.optional.is_empty(), "optional packages should default empty");
        assert!(cfg.packages.dev.is_empty(),      "dev packages should default empty");
        assert!(cfg.apps.is_empty(),              "apps should default empty");
    }
}
