# Prompt 07 — Settings View & Theme Picker

## Before writing any code

1. Read `~/development/dots/_python_backup/dots.py` — `run_settings_view()` and `run_theme_view()`.
2. Read `~/development/dots/dots-rs/src/config/settings.rs` — confirm `load()` and `save()` are done (prompt 02).
3. Read `~/development/dots/src/ghostty/config` to understand the theme line format (`theme = "name"`).
4. State your plan: how the settings fields map to a navigable list, how the theme picker gets its list, and how changes are saved.
5. **Wait for the user to confirm before writing any code.**

---

## Objective

Implement two TUI screens: Settings and Theme Picker. Both modify `~/.dots/settings.toml` (or `~/.personal/config.toml` for user overrides).

---

## Settings screen fields

| Label | Setting key | Type | Behavior on Enter/Space |
|-------|------------|------|------------------------|
| Check for updates | (action) | Action | Opens Update screen |
| Auto-updates | `dots.update_check` | bool | Toggle ON/OFF |
| Check interval | `dots.update_frequency` | cycle | Cycles: 60m → 6h → 12h → 24h → 3d → weekly |
| Shell greeting | `dots.greeting` | bool | Toggle ON/OFF |
| Theme | (action) | Action | Opens Theme Picker |
| Developer mode | `dots.developer_mode` | bool | Toggle ON/OFF |

Layout mirrors the Python version: label (left-aligned, 22 chars wide) + value (right of label, colored).

Save on `q` or `Esc` — same behavior as Python version.

---

## Theme picker screen

The theme list comes from `ghostty +list-themes` output (only available if Ghostty is installed). If Ghostty is not installed, show a message: `"Ghostty is not installed — theme picker requires Ghostty."` and return to settings.

```rust
pub fn list_ghostty_themes() -> Vec<String>;
// Runs: Command::new("ghostty").arg("+list-themes")
// Parses: lines like "Name (source)" → strip " (source)" suffix
// Returns empty Vec (not Err) if ghostty is absent

pub fn get_current_theme(dots_dir: &Path) -> String;
// Reads src/ghostty/config, finds line matching `theme = "..."` or `theme = ...`

pub fn set_ghostty_theme(dots_dir: &Path, name: &str) -> anyhow::Result<()>;
// Replaces the theme line in src/ghostty/config
// Also writes to settings.toml: dots.theme = name
```

Theme picker layout:
```
─────────────── theme ───────────────────
▶  Catppuccin Mocha              (active)
   Dracula
   Nord
   Rose Pine
   ...

  › current: Catppuccin Mocha
──── j/k navigate  enter apply  q back ──
```

Applying a theme:
1. Call `set_ghostty_theme`
2. Show flash: `"✓ Theme set — restart Ghostty to apply"`
3. Do NOT close the picker — let the user continue browsing

---

## herdr mode toggle (SSM settings)

Add one more field to the settings screen (only shown when herdr is installed):

| Label | Setting key | Type |
|-------|------------|------|
| herdr mode | `ssm.use_herdr` | bool |

---

## Error handling to cover

| Scenario | Expected behavior |
|----------|-------------------|
| `ghostty +list-themes` is not installed | Show message, return — no crash |
| `src/ghostty/config` does not exist | `get_current_theme` returns `""`, `set_ghostty_theme` returns `Err` with context |
| Theme line not found in ghostty config | `set_ghostty_theme` appends `theme = "name"` at the end of the file |
| `save()` fails | Show flash error; do NOT exit the screen (let user retry or quit) |
| Cycled through all update intervals, wraps to start | Works correctly |

---

## Testing — three passes

**Pass 1 — theme file read/write:**
```rust
#[test]
fn set_and_get_theme() {
    let tmp = tempdir().unwrap();
    // Write a ghostty config with "theme = \"old\""
    set_ghostty_theme(tmp.path(), "new-theme").unwrap();
    assert_eq!(get_current_theme(tmp.path()), "new-theme");
}

#[test]
fn set_theme_no_existing_line() {
    let tmp = tempdir().unwrap();
    // Write a ghostty config with NO theme line
    set_ghostty_theme(tmp.path(), "Nord").unwrap();
    let content = fs::read_to_string(config_path(tmp.path())).unwrap();
    assert!(content.contains("theme = \"Nord\""));
}
```

**Pass 2 — settings roundtrip:**
Toggle `greeting` to `false` in settings screen, quit (`q`), reopen settings screen — `greeting` must still show `OFF`.

**Pass 3 — manual smoke test:**
Run `cargo run` → Settings → toggle greeting → quit → reopen → confirm persisted. Then open Theme → pick a different theme → confirm ghostty config was updated.

---

## Completion criteria

- [ ] Settings screen renders and saves correctly
- [ ] Theme picker works when Ghostty is installed; shows graceful message when not
- [ ] Both tests pass
- [ ] `set_ghostty_theme` with no existing theme line appends the line (not silently fails)
