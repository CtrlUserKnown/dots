# Prompt 02 — Configuration System

## Before writing any code

1. Read `~/development/dots/dots-rs/src/main.rs` to confirm prompt 01 is complete.
2. Read `~/development/dots/src/zsh/zsh/dots.py` lines 176–194 (the `_DEFAULT_SETTINGS` block and `load_settings` / `save_settings` functions) to understand the current settings model.
3. Read `~/development/dots/_python_backup/dots.py` lines 1152–1211 (personal config: `_collect_personal_config`, `_validate_personal_config`, `apply_personal_config`) to understand what gets exported.
4. Read `~/development/dots/src/zsh/.settings` if it exists to see the current settings JSON format.
5. State your plan: the TOML schema for settings, the `.personal/` directory layout, and the app config JSON format.
6. **Wait for the user to confirm before writing any code.**

---

## Objective

Implement the configuration system:
- `~/.dots/.settings` → replaced by `~/.dots/settings.toml` (dots internal settings)
- `~/.personal/` → user overrides, custom aliases, per-app configs (created on first run)
- `AppConfig` JSON → portable per-application config files importable from other systems

---

## Settings schema (`~/.dots/settings.toml`)

```toml
[dots]
version         = "1"           # settings format version
update_check    = true          # enable auto update check
update_frequency = 1440         # minutes between checks  
greeting        = true          # show fastfetch at shell start
developer_mode  = false         # track commits instead of releases
theme           = ""            # active ghostty theme name (empty = terminal default)

[ssm]
use_herdr       = true          # route connections through herdr --remote
```

Rules:
- Unknown keys must be **ignored** on load (forward-compat), not errored.
- Missing keys fall back to the defaults above.
- The file is created with defaults if it does not exist.
- Saving must be atomic: write to `settings.toml.tmp`, then rename. This prevents a corrupt file if the process is killed mid-write.

---

## `.personal/` layout (`~/.personal/`)

```
~/.personal/
├── config.toml          ← user overrides (same schema as settings.toml, merged on top)
├── aliases.zsh          ← user custom aliases, sourced after dots defaults
└── apps/                ← per-app portable config files
    ├── ghostty.json
    ├── neovim.json
    ├── herdr.json
    └── opencode.json
```

The `.personal/` directory is **never inside `~/.dots/`**. It lives at `$HOME/.personal/`.

`config.toml` uses the same schema as `settings.toml`. On load, dots reads `settings.toml` first, then merges `~/.personal/config.toml` on top. User values win. No key in `~/.personal/config.toml` can disable safety features (e.g. cannot set a key that breaks symlink management — document which keys are off-limits in a comment at the top of the generated file).

---

## App config JSON format

Each file in `~/.personal/apps/` follows this schema:

```json
{
  "version": "1",
  "app": "ghostty",
  "generated": "2026-07-07T12:00:00",
  "dots_version": "1.5.5",
  "settings": {}
}
```

The `settings` object is app-specific and opaque to dots — dots just stores and restores it. The actual config file placement is determined by a built-in mapping (see prompt 10).

---

## Code to implement (`src/config/`)

### `settings.rs`

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DotsSettings {
    pub update_check: bool,
    pub update_frequency: u64,   // minutes
    pub greeting: bool,
    pub developer_mode: bool,
    pub theme: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SsmSettings {
    pub use_herdr: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Settings {
    pub dots: DotsSettings,
    pub ssm: SsmSettings,
}

impl Default for Settings { ... }

pub fn load() -> anyhow::Result<Settings>;
pub fn save(s: &Settings) -> anyhow::Result<()>;
pub fn dots_dir() -> PathBuf;    // ~/.dots
pub fn settings_path() -> PathBuf; // ~/.dots/settings.toml
```

### `personal.rs`

```rust
pub fn personal_dir() -> PathBuf;              // ~/.personal
pub fn ensure_personal_dir() -> anyhow::Result<()>;
pub fn aliases_path() -> PathBuf;              // ~/.personal/aliases.zsh
pub fn apps_dir() -> PathBuf;                  // ~/.personal/apps/
pub fn load_user_overrides() -> anyhow::Result<Settings>; // merges on top of defaults
pub fn generate_aliases_stub() -> anyhow::Result<()>;     // creates aliases.zsh if missing
```

---

## Error handling to cover

| Scenario | Expected behavior |
|----------|-------------------|
| `settings.toml` is malformed TOML | Log warning, fall back to defaults — never crash |
| `~/.personal/` does not exist | Create it silently on first access |
| `~/.personal/config.toml` has an unknown key | Ignore the key, load the rest |
| Write fails (disk full, permissions) | Return `Err` with context: `"failed to save settings: {path}: {cause}"` |
| `settings.toml.tmp` already exists (crashed mid-write) | Overwrite it; it is a leftover from a failed previous write |

---

## Testing — three passes

**Pass 1 — load/save roundtrip:**
```rust
#[test]
fn roundtrip() {
    let tmp = tempdir().unwrap();
    // write a settings.toml into tmp, load it, mutate, save, reload — values must match
}
```

**Pass 2 — missing file:**
Write a test that loads settings when no `settings.toml` exists. Must return defaults without error.

**Pass 3 — merge:**
Write a test that loads a base `settings.toml` with `greeting = false` and merges a `~/.personal/config.toml` with `greeting = true`. Result must have `greeting = true`.

Run `cargo test config` — all three must pass before finishing.

---

## Completion criteria

- [ ] `src/config/settings.rs` and `src/config/personal.rs` implemented
- [ ] All three tests pass with `cargo test`
- [ ] `cargo build` still clean (no new warnings)
- [ ] `.personal/` is created with a stub `aliases.zsh` and `apps/` directory when `ensure_personal_dir()` is called
