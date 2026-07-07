# Prompt 04 — TUI Framework

## Before writing any code

1. Read `~/development/dots/_python_backup/shared.py` — focus on `init_colors()`, `safe_addstr()`, `draw_header()`, `draw_footer()`, `draw_desc()`.
2. Read `~/development/dots/dots-rs/Cargo.toml` to confirm `ratatui` and `crossterm` are listed.
3. Skim the ratatui getting-started guide: https://ratatui.rs/tutorials/hello-world/
4. Skim the ratatui widget docs: https://docs.rs/ratatui/latest/ratatui/widgets/index.html
5. State your plan: the `App` state struct, the event loop design, how the shared header/footer widgets will work, and how terminal-native colors will be used (no hardcoded RGB, only `Color::Reset` / indexed colors).
6. **Wait for the user to confirm before writing any code.**

---

## Objective

Build the ratatui skeleton that all TUI screens will plug into. This includes: the main event loop, shared layout widgets (header, footer, description bar), and the color strategy. No screens are implemented yet — only the framework.

---

## Color strategy

**Do not hardcode RGB or named colors.** Use terminal-native colors so that dots respects whatever theme the user has set in their terminal (Ghostty, iTerm2, etc.).

```rust
// Acceptable:
Style::default().fg(Color::Reset)          // terminal foreground
Style::default().bg(Color::Reset)          // terminal background
Style::default().fg(Color::Cyan)           // ANSI color 6 — terminal defines its shade
Style::default().add_modifier(Modifier::BOLD)

// Not acceptable:
Style::default().fg(Color::Rgb(196, 167, 231))  // hardcoded — breaks dark/light themes
```

The four semantic styles used throughout:
```rust
pub fn style_header() -> Style { Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD) }
pub fn style_select() -> Style { Style::default().fg(Color::Green).add_modifier(Modifier::BOLD) }
pub fn style_error()  -> Style { Style::default().fg(Color::Red).add_modifier(Modifier::BOLD) }
pub fn style_dim()    -> Style { Style::default().fg(Color::DarkGray) }
```

---

## App state

```rust
pub struct App {
    pub should_quit: bool,
    pub screen: Screen,
    pub flash: Option<(String, FlashKind)>,  // transient status message
}

pub enum Screen {
    Main,           // main menu
    Health,
    Theme,
    Settings,
    Update,
    Ssm,
    // more added in later prompts
}

pub enum FlashKind { Success, Error, Info }
```

---

## Event loop

```rust
// src/tui/app.rs
pub fn run(terminal: &mut Terminal<CrosstermBackend<Stdout>>) -> anyhow::Result<()> {
    let mut app = App::new();
    loop {
        terminal.draw(|f| ui(f, &app))?;
        if event::poll(Duration::from_millis(16))? {
            if let Event::Key(key) = event::read()? {
                handle_key(&mut app, key);
            }
        }
        if app.should_quit { break; }
    }
    Ok(())
}
```

Terminal setup/teardown must be in `main.rs`:
```rust
enable_raw_mode()?;
let mut stdout = io::stdout();
execute!(stdout, EnterAlternateScreen)?;
let backend = CrosstermBackend::new(stdout);
let mut terminal = Terminal::new(backend)?;

let result = tui::app::run(&mut terminal);

// Always restore terminal, even if run() returned Err
disable_raw_mode()?;
execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
terminal.show_cursor()?;

result?;
```

Use a `scopeguard` or manual `Drop` pattern so that even a panic restores the terminal. The user's shell must not be left in raw mode.

---

## Shared widgets (`src/tui/mod.rs`)

```rust
/// Renders the top border with centered title and right-aligned version string.
pub fn draw_header(f: &mut Frame, area: Rect, title: &str, version: &str);

/// Renders bottom border at h-3 and hint text at h-2.
pub fn draw_footer(f: &mut Frame, area: Rect, hint: &str);

/// Renders the description/flash bar at h-4.
pub fn draw_desc(f: &mut Frame, area: Rect, text: &str, flash: Option<&(String, FlashKind)>);
```

These three functions are the only shared rendering primitives. Every screen uses them for the same header/footer chrome.

---

## Minimum viable screen: main menu stub

Render a simple list in the center of the screen with placeholder items:
```
─────────────── dots ──────────── v1.5.5 ─
  1  Health
  2  Theme
  3  Settings
  4  Developer
  5  SSM

  › navigate with j/k or 1-9  enter to select  q to quit
───────────────────────────────────────────
```

Navigation: `j`/`k` or arrow keys move the cursor, `q`/`Esc` quits. Enter prints `"selected: {item}"` to stderr (stub — screens not yet implemented).

---

## Terminal size guard

If the terminal is smaller than 50×14, display a centered error message and wait for resize:
```
Terminal too small — need at least 50×14
```
Use `KEY_RESIZE` / `Event::Resize` to re-render when the terminal is resized.

---

## Error handling to cover

| Scenario | Expected behavior |
|----------|-------------------|
| `enable_raw_mode()` fails | Return `Err`, print `"could not enter raw mode: {e}"` to stderr |
| Panic inside `run()` | Terminal is restored via Drop or `std::panic::set_hook` before the panic message prints |
| `event::poll` times out | Continue the loop — this is normal (16ms render tick) |
| Terminal is resized | Re-render cleanly on next tick; no layout artifacts |

---

## Testing — three passes

**Pass 1 — unit test shared widgets don't panic:**
```rust
#[test]
fn header_no_panic() {
    let backend = TestBackend::new(80, 24);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal.draw(|f| {
        draw_header(f, f.area(), " test ", "1.0.0");
        draw_footer(f, f.area(), " q quit ");
    }).unwrap();
}
```

**Pass 2 — small terminal:**
Use `TestBackend::new(30, 8)`. The "too small" message must appear instead of the normal menu.

**Pass 3 — manual smoke test:**
Run `cargo run` in a normal terminal. Confirm:
- Main menu renders
- `j`/`k` moves the cursor
- `q` quits and returns to a clean shell prompt (raw mode restored)
- Resize the terminal mid-run — no crash, re-renders correctly

---

## Completion criteria

- [ ] `cargo run` opens a functional stub TUI
- [ ] Terminal is always restored on exit (including `q`, `Ctrl-C`, and panic)
- [ ] All three tests pass
- [ ] No hardcoded RGB colors anywhere in the TUI code
