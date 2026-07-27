# Manual Test Checklist — vX.Y.Z

Date: __________   Tester: __________

## Setup
- [ ] Fresh `setup.sh` on macOS arm64
- [ ] Fresh `setup.sh` on macOS x86_64
- [ ] Fresh `setup.sh` on Linux (Ubuntu or Fedora)

## TUI golden paths
- [ ] `dots` opens TUI without error
- [ ] Nav strip appears on every screen with the current one lit; narrowing the terminal drops the hints, never the active tab
- [ ] `1`–`6` jump between screens from anywhere, including the dashboard
- [ ] `[` / `]` / `tab` cycle screens and wrap at both ends
- [ ] Typing a digit into the configs add-path prompt, the alias search box, or a profile import path inserts the digit — it does **not** change screen
- [ ] Digits do nothing while the settings popup is open (its edits save on `esc`)
- [ ] `y`/`n` still answer the tools install confirmation rather than navigating
- [ ] Symlinks pane opens the Symlinks screen; Tools pane opens the Tools screen — neither shows the other's rows
- [ ] Tools screen shows all core deps with a green `●`
- [ ] `r` repairs all only on Symlinks, `i` installs all only on Tools; neither key is advertised on the other screen
- [ ] Leaving a screen and returning restores its own cursor position independently
- [ ] Settings: toggle greeting, restart, confirm persisted
- [ ] Theme picker: apply a theme, confirm ghostty config updated
- [ ] Aliases: add alias, restart shell, confirm active
- [ ] Profile: generate personal.json, inspect file
- [ ] Settings popup: check for updates without network (should show error, not crash)

## SSM golden paths
- [ ] `dots ssm` opens TUI
- [ ] Add a session, confirm in list and sessions.json
- [ ] Connect to a real SSH host (or loopback if available)
- [ ] Duplicate a session, confirm name is unique
- [ ] Delete a session, confirm removed from list and keychain

## Dashboard layout (zones)
- [ ] With no `~/.dots/layout.toml`, the dashboard looks exactly as it did before zones
- [ ] `dots layout init` writes a commented `layout.toml`; `dots layout init` again refuses without `--force`
- [ ] Edit `layout.toml` into 2+ zones — hjkl focus crosses zone borders, clicks land on the tile under the cursor
- [ ] A titled zone draws its labelled border and insets its tiles
- [ ] A plugin with `ui.zone{}` + `ui.pane{ zone = … }` gets its own region (`examples/plugins/zones.lua`)
- [ ] Listing a plugin pane in a zone's `widgets` overrides the pane's own `zone`
- [ ] A typo'd widget id is reported by `dots layout show` and the dashboard still opens
- [ ] A malformed `layout.toml` warns on stdout and falls back to the default rather than failing to start

## Edge cases
- [ ] Resize terminal mid-TUI — no panic
- [ ] Ctrl-C during SSM connection — TUI re-enters cleanly
- [ ] `dots --help` prints all subcommands
- [ ] `dots -V` prints the version; `dots --version` adds commit + resolution source
- [ ] A release tag disagreeing with `Cargo.toml` fails the `verify-version` CI job before any binary is built

## Regression: Python parity
For each item, verify the Rust version matches the Python version's behavior:
- [ ] Symlinks created correctly by `dots health --fix`
- [ ] `personal.json` v1 (Python format) imports correctly
- [ ] SSM sessions.json with plaintext passwords (old format) migrates to keychain
