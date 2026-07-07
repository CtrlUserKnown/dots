# Prompt 10 — App Config Manager & `.personal` Import/Export

## Before writing any code

1. Read `~/development/dots/_python_backup/dots.py` — `run_profile_view()`, `_collect_personal_config()`, `_validate_personal_config()`, `apply_personal_config()`, `_fetch_github_raw()`.
2. Read `~/development/dots/dots-rs/src/config/personal.rs` — confirm the `.personal/` stubs are in place (prompt 02).
3. Read `~/development/dots/src/zsh/zsh/dots.py` lines 165–172 (`EDIT_CONFIGS`) to understand which config files are exposed.
4. State your plan: the app config JSON schema, what "apply" means for each app, how import is non-intrusive, and how GitHub import works.
5. **Wait for the user to confirm before writing any code.**

---

## Objective

Let users export their current setup as a portable `personal.json` file, import it on a new system, and have settings + tool preferences restored without overwriting anything the user has manually customized.

---

## `personal.json` schema (version 2, extended from Python version 1)

```json
{
  "version": "2",
  "generated": "2026-07-07T12:00:00",
  "dots_version": "1.6.0",
  "settings": {
    "update_check": true,
    "greeting": true,
    "update_frequency": 1440,
    "developer_mode": false
  },
  "theme": "Catppuccin Mocha",
  "packages": {
    "optional": ["herdr", "btop", "lazygit"],
    "dev": ["gh", "go", "cmake"]
  },
  "apps": {
    "ghostty": { "theme": "Catppuccin Mocha" },
    "neovim": {},
    "opencode": {}
  }
}
```

**Migration:** if `version == "1"` (Python format), parse it and silently upgrade to version 2.

---

## App config registry

Define a static registry of known apps and what "apply" means for each:

```rust
pub struct AppEntry {
    pub name:         &'static str,
    pub config_path:  fn() -> PathBuf,  // e.g. ~/.dots/src/ghostty/config
    pub apply:        fn(value: &Value) -> anyhow::Result<()>,
}
```

| App | Apply logic |
|-----|-------------|
| `ghostty` | If `theme` key is set, call `tui::theme::set_ghostty_theme` |
| `neovim` | No config managed by dots — note as "managed externally" |
| `opencode` | No config managed by dots |
| `herdr` | No config managed by dots (user has own herdr config per instructions) |

The rule: **dots only modifies files inside `~/.dots/src/`**. It never writes to `~/.config/nvim/`, user's herdr config, etc. If an app's config is outside `~/.dots/src/`, dots notes it as "not managed" but does not touch it.

---

## Non-intrusive apply

"Apply" means:
1. Merge settings from `personal.json` into `settings.toml` — **only keys that exist in the file, and only if they are valid**.
2. Apply known app configs (e.g. set ghostty theme).
3. Return a list of packages not yet installed on this system.
4. **Never overwrite `rc.zsh`, `.aliases`, `.functions`, or any zsh config file.** If the user has modified those files, those modifications are preserved.

---

## TUI Profile screen

Accessible from main menu as "Profile". Mirrors the Python `run_profile_view`.

```
─────────────── profile ──────────────────
  ✓  ~/.personal/config.toml
     generated 2026-07-07T12:00:00  ·  3 packages  ·  dots 1.6.0

  g  generate / update config from current system
  i  import from a local file
  G  import from GitHub  (user/repo/path/to/file.json)

  › generate / update config from current system
──── g gen  i import file  G import GitHub  q back ────
```

### Generate (`g`)
Call `collect_personal_config()` → write to `~/.personal/personal.json` → flash success.

### Import file (`i`)
Inline input prompt for file path. Validate → apply → install missing packages (via `packages::install_deps` from prompt 12).

### Import GitHub (`G`)
Inline input prompt: `user/repo/path/to/file.json`. Fetch via HTTPS using `ureq`:
```
https://raw.githubusercontent.com/{user}/{repo}/{main|master}/{path}
```
Try `main` first, then `master`. Parse as JSON → validate → apply.

---

## CLI interface

```
dots --setting-file-generate [path]   generate personal.json
dots --import <path>                  import from file
dots --import --git user/repo/path    import from GitHub
```

---

## Error handling to cover

| Scenario | Expected behavior |
|----------|-------------------|
| `personal.json` version is unknown | `Err("unsupported version '3' — update dots")` |
| `packages` field is not an object | `Err("'packages' must be an object")` |
| File not found on import | `Err("file not found: {path}")` |
| GitHub fetch fails (no internet) | `Err("could not fetch from GitHub: {cause}")` |
| `apply` modifies a file the user has manually edited | **Never allowed** — dots only manages `~/.dots/src/` files |
| `personal.json` is missing keys | Fill in defaults, not an error |

---

## Testing — three passes

**Pass 1 — generate/validate roundtrip:**
```rust
#[test]
fn generate_and_validate() {
    let tmp = tempdir().unwrap();
    let path = tmp.path().join("personal.json");
    generate_personal_config(&path).unwrap();
    let data: Value = serde_json::from_str(&fs::read_to_string(&path).unwrap()).unwrap();
    assert!(validate_personal_config(&data).is_ok());
}
```

**Pass 2 — version 1 migration:**
Write a `personal.json` with `"version": "1"` (Python format). Load it. Assert `version` in memory is `"2"` and all fields parsed correctly.

**Pass 3 — apply is non-intrusive:**
Manually edit `~/.dots/src/zsh/zsh/rc.zsh` to add a comment. Apply a `personal.json`. Assert the comment is still there.

---

## Completion criteria

- [ ] Generate, import-file, and import-GitHub all work in TUI and CLI
- [ ] Version 1 → version 2 migration works
- [ ] Apply never modifies zsh config files
- [ ] All three tests pass
