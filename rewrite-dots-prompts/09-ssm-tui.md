# Prompt 09 — SSM TUI Screen

## Before writing any code

1. Read `~/development/dots/_python_backup/ssm.py` — `run_tui()`, `run_form()`, `run_search()`, `run_help_view()`.
2. Read `~/development/dots/dots-rs/src/ssm/storage.rs` — confirm `load_sessions()`, `save_sessions()`, `do_connect()` are done (prompt 08).
3. Read `~/development/dots/dots-rs/src/tui/mod.rs` — confirm `draw_header`, `draw_footer`, `draw_desc` are available.
4. Read the `dots` man page at `~/development/dots/src/man/man1/dots.1` and `ssm.1` for the help text reference.
5. State your plan: the TUI state machine, which screens are separate structs, how the form handles field editing inline, and where help links to.
6. **Wait for the user to confirm before writing any code.**

---

## Objective

Build the SSM ratatui TUI — the interactive session manager. The user opens it with `dots ssm` (or the `ssm` alias). It replaces the Python curses implementation.

---

## TUI state

```rust
pub enum SsmScreen {
    List,
    Form(FormState),       // add or edit
    Search(SearchState),
    Help,
    ConfirmDelete(usize),  // index of session to delete
}

pub struct SsmApp {
    pub sessions:      Vec<Session>,
    pub cfg:           ConnectConfig,
    pub idx:           usize,            // selected row
    pub screen:        SsmScreen,
    pub flash:         Option<(String, FlashKind)>,
    pub filter_active: bool,
    pub visible:       Vec<usize>,       // indices into sessions (filtered view)
    pub count_buf:     String,           // vim-style numeric prefix
    pub pending_g:     bool,             // waiting for second 'g' in 'gg'
}
```

---

## List screen layout

```
─── ssh sessions ────────────────── [herdr ON] ─
  NAME                 HOST/IP                  USER         PORT
  ──────────────────────────────────────────────────────────────
▶ prod-server          192.168.1.10             root         22
  staging              staging.example.com      deploy       2222
  ...

  › root@192.168.1.10:22
── j/k nav  gg/G top/bot  enter connect  a add  e edit  D dup  d del  y yank  / search  h herdr  ? help  q quit ──
```

---

## Key bindings (list screen)

| Key | Action |
|-----|--------|
| `j` / `↓` | Down |
| `k` / `↑` | Up |
| `gg` | Jump to top |
| `G` | Jump to bottom (or row N with count prefix) |
| `Ctrl-d` / `Ctrl-u` | Half page down/up |
| `Ctrl-f` / `Ctrl-b` | Full page down/up |
| `Enter` | Connect to selected session |
| `a` | Open add form |
| `e` | Open edit form (pre-filled) |
| `D` | Duplicate selected session |
| `d` | Confirm delete inline (show "Delete 'name'? y/n" in flash area) |
| `y` | Copy `user@host:port` to clipboard (pbcopy on macOS; xclip/xsel on Linux) |
| `/` | Open search |
| `h` | Toggle herdr mode; flash result |
| `?` | Open help screen |
| `u` | Open update screen (reuse from prompt 06) |
| `q` / `Esc` | Quit (if filter active: clear filter first) |
| `1`–`9` | Numeric prefix for count operations |

---

## Add/Edit form

Inline form with 5 fields (same as Python version):
```
Name       [ my-server              ]
Host/IP    [ 192.168.1.10           ]
User       [ root                   ]
Password   [ ••••••••               ]
Port       [ 22                     ]
```

- `Tab` / `↓` / `Enter` advances to next field
- `↑` goes to previous field
- `Esc` cancels
- `Enter` on last field validates and saves
- Password field: mask with `•`

Validation:
- Name and Host/IP are required; flash error if empty
- Port must be a valid u16; flash error if not
- On add: name must be unique; flash error if duplicate

---

## Search screen

Incremental filter on name and host. `Esc` clears filter and returns to list. `Enter` confirms filter and shows filtered list (indicated by `[filtered]` in header).

---

## Help screen

```
─────────────── help ─────────────────────

  navigation
  j / k  ↑↓         move up / down
  gg                 jump to top
  G                  jump to bottom
  ^d / ^u            half-page down / up
  ...

  actions
  enter              connect
  a                  add session
  e                  edit session
  D                  duplicate
  d                  delete
  y                  copy connection string

  other
  h                  toggle herdr mode
  u                  check for updates
  ?                  this screen
  q  esc             quit / back

  documentation
  man ssm            full man page
  https://github.com/CtrlUserKnown/dotfiles/wiki

──────────────── q back ──────────────────
```

The GitHub wiki URL and man page reference are hardcoded strings — no HTTP calls in the help screen.

---

## Clipboard

```rust
pub fn yank(text: &str) -> anyhow::Result<()>;
// macOS: pipe to `pbcopy`
// Linux: try `xclip -selection clipboard`, fallback to `xsel --clipboard --input`
// If neither available: return Err("clipboard tool not found — install xclip or xsel")
```

---

## Auto-reload

When `sessions.json` changes on disk (modified externally), reload sessions automatically. Check `sessions.json` mtime once per render tick and reload if it changed. Flash `"↺ Sessions reloaded"` when this happens.

---

## Error handling to cover

| Scenario | Expected behavior |
|----------|-------------------|
| `do_connect` returns `Err` | Leave SSM TUI, print error, `"Press Enter to return…"`, re-enter TUI |
| `save_sessions` fails | Flash error; session is NOT added to the in-memory list |
| Clipboard tool absent | Flash `"✗ pbcopy / xclip not available"` |
| `sessions.json` deleted externally | Reload returns empty list; no crash |
| Form: port field contains letters | Flash `"✗ Port must be a number"` |
| Terminal too small | Graceful partial render; session rows clipped, no panic |

---

## Testing — three passes

**Pass 1 — form validation unit test:**
```rust
#[test]
fn form_rejects_empty_name() {
    let result = validate_form("", "192.168.1.1", "root", "", "22");
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("Name"));
}

#[test]
fn form_rejects_bad_port() {
    let result = validate_form("s", "h", "u", "", "abc");
    assert!(result.is_err());
}
```

**Pass 2 — search filter:**
```rust
#[test]
fn search_filters_by_name() {
    let sessions = vec![
        Session { name: "prod".into(), host: "1.1.1.1".into(), .. },
        Session { name: "staging".into(), host: "2.2.2.2".into(), .. },
    ];
    let result = filter_sessions(&sessions, "prod");
    assert_eq!(result.len(), 1);
    assert_eq!(result[0].name, "prod");
}
```

**Pass 3 — manual smoke test:**
Run `cargo run -- ssm`. Confirm:
- List renders with correct columns
- `a` opens form, `Esc` cancels without saving
- Add a session, confirm it appears in list and `sessions.json`
- Press `y` on a session, confirm the string is in clipboard
- Press `?` to see help
- Press `q` to quit

---

## Completion criteria

- [ ] All key bindings work as described
- [ ] Passwords never appear in `sessions.json`
- [ ] Help screen shows GitHub wiki URL and man page reference
- [ ] Both unit tests pass
- [ ] No panic when sessions list is empty
- [ ] `cargo run -- ssm` opens TUI cleanly
