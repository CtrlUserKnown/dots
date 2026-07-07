# Prompt 11 — Alias System

## Before writing any code

1. Read `~/development/dots/_python_backup/dots.py` — `run_alias_view()` if it exists, and `PERSONAL_ALIASES_PATH`.
2. Read `~/development/dots/src/zsh/zsh/.aliases` in full — understand which aliases are baked-in defaults and which are expected to be user-extensible.
3. Read `~/development/dots/src/zsh/zsh/rc.zsh` — find where `.aliases` and `~/.personal/aliases.zsh` are sourced.
4. Read `~/development/dots/dots-rs/src/config/personal.rs` — confirm `.personal/` path helpers are in place (prompt 02).
5. State your plan: how defaults are sourced from the zsh file (read-only), how user aliases are stored, and how the TUI add-alias form works.
6. **Wait for the user to confirm before writing any code.**

---

## Objective

Let users view and extend the alias system from the TUI — browsing built-in aliases and adding their own to `~/.personal/aliases.zsh`. Never modify the built-in `.aliases` file.

---

## Two tiers of aliases

| Tier | Source file | Editable by dots? |
|------|-------------|-------------------|
| Built-in | `~/.dots/src/zsh/zsh/.aliases` | **No** — dots never modifies this file |
| User | `~/.personal/aliases.zsh` | **Yes** — dots reads and appends |

Both files are sourced by `rc.zsh` so both are active in the shell.

---

## Alias struct

```rust
#[derive(Debug, Clone)]
pub struct Alias {
    pub name:   String,    // e.g. "la"
    pub value:  String,    // e.g. "ls -la"
    pub source: AliasSource,
}

pub enum AliasSource {
    BuiltIn,
    User,
}
```

---

## Parsing aliases from a zsh file

Parse the file into `Vec<Alias>`. Recognize two forms:

```zsh
alias la='ls -la'
alias la="ls -la"
```

Ignore comment lines (`#`), blank lines, suffix aliases (`alias -s`), and any line that does not start with `alias ` (after stripping leading whitespace). Do not evaluate shell syntax — treat the right-hand side as a raw string.

```rust
pub fn parse_alias_file(path: &Path) -> Vec<Alias>;
// Returns empty Vec if the file does not exist — not an error
```

---

## Adding a user alias

```rust
pub fn add_user_alias(
    personal_dir: &Path,
    name: &str,
    value: &str,
) -> anyhow::Result<()>;
```

1. Parse `~/.personal/aliases.zsh` to check for a name collision with existing user aliases (not built-ins). If a collision exists, return `Err("alias '{name}' already exists in your personal aliases")`.
2. Append to `~/.personal/aliases.zsh` atomically (read → append in memory → tmp → rename):
   ```zsh
   alias name='value'
   ```
3. If `~/.personal/aliases.zsh` does not exist, create it with a header comment:
   ```zsh
   # personal aliases — managed by dots
   alias name='value'
   ```

Never modify `~/.dots/src/zsh/zsh/.aliases`.

---

## Removing / editing a user alias

```rust
pub fn remove_user_alias(personal_dir: &Path, name: &str) -> anyhow::Result<()>;
pub fn edit_user_alias(personal_dir: &Path, name: &str, new_value: &str) -> anyhow::Result<()>;
```

Both rewrite `~/.personal/aliases.zsh` atomically. Built-in aliases cannot be removed or edited — return `Err("cannot modify built-in alias '{name}'")` if attempted.

---

## TUI Alias screen

Accessible from main menu as "Aliases".

```
─────────── aliases ─────────────────────
  built-in                         user
  ──────────────────────────────────────
  la        ls -la                 •
  lla       ls -la                 •
  gst       git status
  gco       git checkout
  ...

▶ la        ls -la                 (user)

  a add  e edit  d delete (user only)  q back
──── j/k navigate  a add  / search  q back ────
```

- Built-in aliases shown with no indicator.
- User aliases shown with a `(user)` badge.
- `e` and `d` are only active on user aliases. If the user presses them on a built-in, flash `"✗ Cannot modify built-in aliases"`.
- `a` opens an inline add form.

### Add form

```
Name   [ la         ]
Value  [ ls -la     ]
(Enter to save, Esc to cancel)
```

Validation:
- Name must be non-empty, no spaces, no shell special characters (only `[a-zA-Z0-9_-]`).
- Value must be non-empty.
- Name collision with existing aliases (built-in or user) → flash error, do not save.

---

## CLI interface

```
dots aliases list             print all aliases (built-in + user)
dots aliases add NAME VALUE   add a user alias
dots aliases remove NAME      remove a user alias
```

---

## Error handling to cover

| Scenario | Expected behavior |
|----------|-------------------|
| `~/.dots/src/zsh/zsh/.aliases` absent | Built-in list shows empty; no crash |
| `~/.personal/aliases.zsh` absent | User list shows empty; created on first add |
| Name contains a space | `Err("alias name must not contain spaces")` |
| Edit/delete on a built-in | `Err("cannot modify built-in alias")` |
| Alias name collision (user vs built-in) | Flash error; do not save |
| File write fails | Err with context; old file untouched |

---

## Testing — three passes

**Pass 1 — parse roundtrip:**
```rust
#[test]
fn parse_aliases() {
    let content = "alias la='ls -la'\nalias gst='git status'\n";
    let tmp = write_tmp(content);
    let aliases = parse_alias_file(&tmp);
    assert_eq!(aliases.len(), 2);
    assert_eq!(aliases[0].name, "la");
    assert_eq!(aliases[0].value, "ls -la");
}
```

**Pass 2 — add/remove user alias:**
```rust
#[test]
fn add_and_remove_user_alias() {
    let tmp = tempdir().unwrap();
    add_user_alias(tmp.path(), "foo", "echo foo").unwrap();
    let aliases = parse_alias_file(&tmp.path().join("aliases.zsh"));
    assert!(aliases.iter().any(|a| a.name == "foo"));
    remove_user_alias(tmp.path(), "foo").unwrap();
    let aliases = parse_alias_file(&tmp.path().join("aliases.zsh"));
    assert!(!aliases.iter().any(|a| a.name == "foo"));
}
```

**Pass 3 — manual smoke test:**
Run `cargo run` → Aliases. Confirm:
- Built-in aliases from `.aliases` are listed.
- Press `a`, add a new alias, confirm it appears in list.
- Verify `~/.personal/aliases.zsh` contains the new alias.
- Press `d` to delete the alias, confirm it is removed.
- Restart the shell and confirm `source ~/.personal/aliases.zsh` exposes the alias.

---

## Completion criteria

- [ ] Built-in aliases display correctly from `.aliases` file
- [ ] User aliases can be added, edited, and removed
- [ ] Built-in aliases are never modified
- [ ] `~/.personal/aliases.zsh` is created if absent on first add
- [ ] All three tests pass
