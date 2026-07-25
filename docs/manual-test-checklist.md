# Manual Test Checklist — vX.Y.Z

Date: __________   Tester: __________

## Setup
- [ ] Fresh `setup.sh` on macOS arm64
- [ ] Fresh `setup.sh` on macOS x86_64
- [ ] Fresh `setup.sh` on Linux (Ubuntu or Fedora)

## TUI golden paths
- [ ] `dots` opens TUI without error
- [ ] Health screen shows all core deps as ✓
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

## Edge cases
- [ ] Resize terminal mid-TUI — no panic
- [ ] Ctrl-C during SSM connection — TUI re-enters cleanly
- [ ] `dots --help` prints all subcommands
- [ ] `dots --version` prints correct version

## Regression: Python parity
For each item, verify the Rust version matches the Python version's behavior:
- [ ] Symlinks created correctly by `dots health --fix`
- [ ] `personal.json` v1 (Python format) imports correctly
- [ ] SSM sessions.json with plaintext passwords (old format) migrates to keychain
