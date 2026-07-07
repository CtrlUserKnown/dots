# Prompt 05 — Health View

## Before writing any code

1. Read `~/development/dots/_python_backup/dots.py` — focus on `run_health_view()` and the DEPS list (lines ~54–96).
2. Read `~/development/dots/dots-rs/src/symlinks.rs` — confirm `check()` and `get_symlinks()` are done (prompt 03).
3. Read `~/development/dots/dots-rs/src/tui/mod.rs` — confirm `draw_header`, `draw_footer`, `draw_desc` are available (prompt 04).
4. State your plan: how the display list is built (sections + items), how navigation and inline-fix work, and how the deps list will be structured in Rust.
5. **Wait for the user to confirm before writing any code.**

---

## Objective

Implement the Health screen — a scrollable list showing symlink status, required/optional tool status, and shell plugin status. The user can navigate to any broken item and press `Enter` to fix it, or press `r` to repair all broken symlinks at once.

---

## Deps list

Port the Python `DEPS` list to Rust as a static array. Define a `Dep` struct:

```rust
pub struct Dep {
    pub bin:      &'static str,   // binary name for which()
    pub brew:     &'static str,   // homebrew formula
    pub dnf:      &'static str,   // fedora dnf package
    pub apt:      &'static str,   // debian/ubuntu apt package
    pub desc:     &'static str,
    pub category: Category,
    pub tap:      &'static str,   // brew tap (empty if none)
    pub cask:     bool,
}

pub enum Category { Required, Optional, Dev }
```

Include at minimum (mirror the Python list):
- Required: `git`, `eza`, `bat`, `fd`, `fzf`, `fastfetch`, `zoxide`
- Optional: `herdr`, `btop`, `lazygit`, `yazi`, `carapace`, `nvim`

The health view only displays `Required` and `Optional` deps (Dev is in the package installer, prompt 12).

---

## Display model

Build a flat `Vec<DisplayRow>`:

```rust
enum DisplayRow {
    Section(String),                   // bold dim separator label
    SymlinkItem { symlink: Symlink, status: SymlinkStatus },
    DepItem     { dep: &'static Dep, installed: bool },
    PluginItem  { name: &'static str, desc: &'static str, installed: bool },
}
```

Navigation only lands on `SymlinkItem`, `DepItem`, `PluginItem` — sections are skipped.

---

## Shell plugins

Replicate the Python `PLUGINS` list as a static array:

```rust
pub struct Plugin {
    pub name:  &'static str,
    pub desc:  &'static str,
    pub paths: &'static [&'static str],  // check any of these paths for existence
}
```

Plugins:
- `zsh-autosuggestions` — `/opt/homebrew/share/zsh-autosuggestions/zsh-autosuggestions.zsh` or `/usr/share/...`
- `zsh-syntax-highlighting` — similar
- `zsh-history-substring-search` — `~/.config/zsh/plugins/zsh-history-substring-search` or homebrew path
- `fzf-tab` — `~/.config/zsh/plugins/fzf-tab`

---

## Layout

```
─────────────────── health ──────────────
  symlinks
  ✓  ~/.config/zsh            → ~/.dots/src/zsh/zsh
▶ ✗  ~/.config/bat            BROKEN
  ✓  ~/.zshrc                 → ~/.dots/src/zsh/.zshrc

  tools
  ✓  git           [req]   version control
  ✓  eza           [req]   modern ls replacement
  ✗  fzf           [req]   fuzzy finder
  ...

  plugins
  ✓  zsh-autosuggestions      fish-like suggestions
  ✗  fzf-tab                  fuzzy-search tab completions

  › → ~/.dots/src/bat          (description of selected)
─── enter fix  r repair all  i install missing  q back ──
```

The `▶` cursor marks the selected item. Scroll offset keeps the selected item in view.

---

## Key bindings

| Key | Action |
|-----|--------|
| `j` / `↓` | Move down |
| `k` / `↑` | Move up |
| `Enter` | Fix selected item (repair symlink or install dep via package manager) |
| `r` | Repair all broken symlinks |
| `i` | Install all missing required/optional deps (pass dep list to `packages::install_deps`) |
| `q` / `Esc` | Back to main menu |

---

## Inline fix behavior

- **Symlink fix:** call `symlinks::repair()`; show flash `"✓ Repaired"` or `"✗ Repair failed"`
- **Dep install:** call `packages::install_one(dep)` (stub from packages.rs — implemented in prompt 12); for now, print `"install not yet implemented"` as a flash
- **Plugin fix:** show flash `"install plugin manually — see docs"` (plugins are cloned via setup.sh, not managed inline)

---

## Error handling to cover

| Scenario | Expected behavior |
|----------|-------------------|
| `get_symlinks()` returns empty (no `~/.dots`) | Show one-line warning in symlinks section: "~/.dots not found" |
| `which()` subprocess fails | Treat dep as not installed; do not crash |
| Terminal too small for the list | Show as many rows as fit; scroll still works |
| Repair fails (permissions) | Flash error with the `anyhow` error string |

---

## Testing — three passes

**Pass 1 — display list has correct structure:**
```rust
#[test]
fn display_list_has_sections() {
    let rows = build_display_rows();
    let sections: Vec<_> = rows.iter().filter(|r| matches!(r, DisplayRow::Section(_))).collect();
    assert!(sections.len() >= 3); // symlinks, tools, plugins
}
```

**Pass 2 — navigation skips sections:**
Write a test that simulates pressing `j` from a section row and confirms the cursor lands on the next item, not the section itself.

**Pass 3 — manual smoke test:**
Run `cargo run` → open Health. Confirm:
- All symlinks and tools for your system show the correct status
- Pressing `j`/`k` scrolls through items
- Pressing `r` on a healthy system shows `"0 repaired, X already OK"`
- Pressing `q` returns to the main menu without crashing

---

## Completion criteria

- [ ] Health screen renders correctly in a real terminal
- [ ] All three tests pass
- [ ] `cargo run -- health` runs health check in non-TUI mode (prints a text report to stdout) — this is used by CI/setup scripts
- [ ] No panics when `~/.dots` is absent
