# Prompt 08 — SSM Core (Storage, Keychain, Connection)

## Before writing any code

1. Read `~/development/dots/_python_backup/ssm.py` — focus on `load_sessions()`, `save_sessions()`, `_kr_store()`, `_kr_load()`, `_kr_delete()`, and `do_connect()`.
2. Read `~/development/dots/dots-rs/src/ssm/storage.rs` (should be a stub from prompt 01).
3. Check whether `keyring` is available on this system: `cargo add keyring --dry-run` or read Cargo.toml.
4. Read the `keyring` crate docs: https://docs.rs/keyring/latest/keyring/ — understand `Entry::new`, `set_password`, `get_password`, `delete_credential`.
5. State your plan: the `Session` struct, how sessions are stored on disk vs in keychain, what happens if keychain is unavailable, and the `do_connect` flow for herdr vs plain SSH.
6. **Wait for the user to confirm before writing any code.**

---

## Objective

Implement SSM's backend: session persistence, OS keychain integration, and SSH/herdr connection. No TUI yet — only the logic layer.

---

## Session struct

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Session {
    pub name:     String,
    pub host:     String,
    pub user:     String,
    #[serde(skip)]           // never written to disk
    pub password: String,
    pub port:     u16,
}
```

The `#[serde(skip)]` on `password` ensures it is never serialized to JSON. The disk format contains name, host, user, port only.

---

## Storage paths

```
~/.config/ssm/sessions.json   ← session list (no passwords)
~/.config/ssm/config.json     ← ssm config (use_herdr)
~/.config/ssm/known_hosts     ← ssh known_hosts file for ssm connections
```

---

## Keychain

SSM uses the OS keychain to store passwords. Service name: `"dots-ssm"`. Username: the session `name`.

```rust
pub fn kr_store(name: &str, password: &str) -> anyhow::Result<()>;
pub fn kr_load(name: &str) -> Option<String>;
pub fn kr_delete(name: &str);

// Keychain availability check — called once at startup
pub fn keychain_available() -> bool;
// Tries Entry::new("dots-ssm", "__probe__").get_password()
// Returns true if it succeeds OR returns NoStorageAccess (which means a backend exists but the entry doesn't — that's fine)
// Returns false only if the backend itself is absent (e.g. no secret-service daemon on Linux)
```

**If `keychain_available()` returns false:** SSM is not available. `dots ssm` prints:
```
SSM requires a keychain backend.
  macOS: available by default
  Linux: install and run a secret-service provider (e.g. gnome-keyring, kwallet)
```
And exits. Do not continue.

---

## Session load/save

```rust
pub fn load_sessions() -> anyhow::Result<Vec<Session>>;
```
1. Read `sessions.json`. If absent, return empty `Vec`.
2. For each session: call `kr_load(name)` and set `session.password` from the result.
3. **Migration:** if a session in the JSON has a `password` field (old plaintext format), call `kr_store(name, password)`, then re-save without the password field.

```rust
pub fn save_sessions(sessions: &[Session]) -> anyhow::Result<()>;
```
1. For each session with a non-empty password: `kr_store(name, &session.password)`.
2. Serialize sessions without passwords to JSON (the `#[serde(skip)]` handles this).
3. Write atomically (tmp → rename).

---

## Connection

```rust
pub struct ConnectConfig {
    pub use_herdr: bool,
}

pub fn do_connect(session: &Session, cfg: &ConnectConfig) -> anyhow::Result<()>;
```

**herdr mode** (`cfg.use_herdr == true`):
1. Check `which("herdr")` — if absent, return `Err("herdr is not installed. Install it or toggle off herdr mode.")`.
2. Build URL: `ssh://user@host` or `ssh://user@host:port` (if port ≠ 22).
3. `Command::new("herdr").arg("--remote").arg(&url).status()?`

**Plain SSH mode**:
```
ssh -p <port>
    -o StrictHostKeyChecking=accept-new
    -o UserKnownHostsFile=~/.config/ssm/known_hosts
    -o ConnectTimeout=5
    [-o PubkeyAuthentication=no -o PreferredAuthentications=password]  ← if password set
    user@host
```

If password is set and `sshpass` is available: use `sshpass -e ssh ...` with `SSHPASS` env var.
If password is set and `sshpass` is not available: print the install tip and run SSH anyway (user may have key auth).

`do_connect` must propagate the subprocess exit code: if SSH exits non-zero, return `Err("connection failed (exit N)")`.

---

## CLI wiring (`main.rs`)

```rust
Some(Command::Ssm { connect: Some(spec), list: false }) => {
    ssm::connect_direct(&spec)?;
}
Some(Command::Ssm { connect: None, list: true }) => {
    ssm::cli_list()?;
}
Some(Command::Ssm { connect: None, list: false }) => {
    // Open TUI — implemented in prompt 09
    println!("ssm TUI — not yet implemented");
}
```

---

## Error handling to cover

| Scenario | Expected behavior |
|----------|-------------------|
| `sessions.json` is malformed JSON | Return `Err` with context: `"sessions.json is corrupt: {path}: {e}"` |
| `kr_store` fails (keychain locked) | Return `Err` with message; do not save the session |
| SSH binary not in PATH | `Err("ssh not found in PATH")` |
| herdr not in PATH | `Err("herdr is not installed...")` |
| `sessions.json` write fails | `Err` with context — the old file is not touched (atomic write) |
| Duplicate session name on add | Caller's responsibility to check before calling `save_sessions` |

---

## Testing — three passes

**Pass 1 — save/load strips passwords from JSON:**
```rust
#[test]
fn password_not_in_json() {
    let tmp = tempdir().unwrap();
    // Override sessions path to tmp
    let sessions = vec![Session { name: "test".into(), host: "1.2.3.4".into(),
                                  user: "root".into(), password: "secret".into(), port: 22 }];
    save_sessions_to(&sessions, &tmp.path().join("sessions.json")).unwrap();
    let raw: Value = serde_json::from_str(&fs::read_to_string(...).unwrap()).unwrap();
    assert!(raw[0]["password"].is_null() || raw[0].get("password").is_none());
}
```

**Pass 2 — migration test:**
Write a `sessions.json` that contains a `password` field (old format). Call `load_sessions`. Assert that:
- Returned session has the password in memory.
- On disk, the password field is gone.

**Pass 3 — `do_connect` herdr-not-found:**
Call `do_connect` with `use_herdr = true` on a system where herdr is not in PATH (or mock `which` to return None). Assert the returned error contains `"herdr is not installed"`.

---

## Completion criteria

- [ ] `cargo run -- ssm -l` lists sessions (empty on fresh install — shows "No sessions saved")
- [ ] `cargo run -- ssm -c user@host` connects (or errors cleanly if host unreachable)
- [ ] All three tests pass
- [ ] No password appears in `sessions.json` after save
- [ ] `keychain_available()` returns correct result on this system
