//! The Lua plugin system.
//!
//! Users drop `*.lua` files in `~/.dots/plugins/`. Each file may register
//! dashboard panes and tweak the dashboard layout through two globals injected
//! before it runs:
//!
//! * **`ui`** — the UI package. `ui.pane{…}` adds a dashboard pane (with a
//!   `size` height-weight and `span` column-width, so a plugin can make its
//!   pane bigger); `ui.layout{ columns = N }` sets the grid's column count.
//! * **`dots`** — integration helpers. `dots.sh(cmd)` runs a shell command and
//!   returns its trimmed stdout (this is what drives `gh`, `aws`, …);
//!   `dots.env(name)` and `dots.dir()` expose the environment and the `~/.dots`
//!   path.
//!
//! Plugins are trusted like any other dotfile — there is no sandbox. A plugin
//! that fails to load is recorded as an error (surfaced by `dots plugins list`)
//! and skipped; it never crashes the TUI. The [`Lua`] state is not `Send`, so
//! the [`PluginHost`] lives on the TUI's main thread and refreshes panes on a
//! per-pane interval from the event loop.

pub mod layout;

use std::cell::{Cell, RefCell};
use std::path::PathBuf;
use std::rc::Rc;
use std::time::{Duration, Instant};

use mlua::{Function, Lua, RegistryKey, Value};

use crate::config::settings::dots_dir;

/// Default seconds between `render()` calls for a pane that doesn't set one.
const DEFAULT_REFRESH: u64 = 30;

/// Where user plugins live: `~/.dots/plugins/`.
pub fn plugins_dir() -> PathBuf {
    dots_dir().join("plugins")
}

/// Runtime state for one registered pane. `render`/`on_enter` are Lua functions
/// stashed in the registry so the host can call them later.
struct PaneState {
    id: String,
    title: String,
    span: u16,
    weight: u16,
    refresh: Duration,
    render: Option<RegistryKey>,
    on_enter: Option<RegistryKey>,
    lines: Vec<String>,
    last: Option<Instant>,
}

/// A plain, cloneable snapshot of a pane for the renderer to read (keeps the
/// Lua types off the TUI side).
#[derive(Clone, Default)]
pub struct PluginPaneView {
    pub id: String,
    pub title: String,
    pub span: u16,
    pub weight: u16,
    pub lines: Vec<String>,
}

/// What `dots plugins list` reports for one `.lua` file.
pub struct PluginInfo {
    pub name: String,
    pub pane_ids: Vec<String>,
    pub error: Option<String>,
}

/// Owns the Lua runtime and every registered pane.
pub struct PluginHost {
    lua: Lua,
    panes: Vec<PaneState>,
    columns: Option<u16>,
    infos: Vec<PluginInfo>,
}

impl PluginHost {
    /// Discover and load every plugin in [`plugins_dir`]. Never fails: load
    /// errors are captured per-file in [`PluginHost::plugins`].
    pub fn load() -> Self {
        let lua = Lua::new();
        let collector: Rc<RefCell<Vec<PaneState>>> = Rc::new(RefCell::new(Vec::new()));
        let columns: Rc<Cell<Option<u16>>> = Rc::new(Cell::new(None));

        if let Err(e) = install_api(&lua, &collector, &columns) {
            // Injecting the API should never fail; if it does, run with none.
            return Self {
                lua,
                panes: Vec::new(),
                columns: None,
                infos: vec![PluginInfo { name: "<api>".into(), pane_ids: vec![], error: Some(e.to_string()) }],
            };
        }

        let mut infos = Vec::new();
        let mut files: Vec<PathBuf> = std::fs::read_dir(plugins_dir())
            .into_iter()
            .flatten()
            .flatten()
            .map(|e| e.path())
            .filter(|p| p.extension().and_then(|s| s.to_str()) == Some("lua"))
            .collect();
        files.sort();

        for path in files {
            let name = path.file_stem().and_then(|s| s.to_str()).unwrap_or("plugin").to_string();
            let before = collector.borrow().len();
            let info = match std::fs::read_to_string(&path) {
                Ok(code) => match lua.load(&code).set_name(&name).exec() {
                    Ok(()) => {
                        let ids = collector.borrow()[before..].iter().map(|p| p.id.clone()).collect();
                        PluginInfo { name, pane_ids: ids, error: None }
                    }
                    Err(e) => PluginInfo { name, pane_ids: vec![], error: Some(e.to_string()) },
                },
                Err(e) => PluginInfo { name, pane_ids: vec![], error: Some(e.to_string()) },
            };
            infos.push(info);
        }

        let panes = std::mem::take(&mut *collector.borrow_mut());
        Self { lua, panes, columns: columns.get(), infos }
    }

    /// Number of registered panes.
    pub fn pane_count(&self) -> usize {
        self.panes.len()
    }

    /// The dashboard column count a plugin asked for, if any.
    pub fn columns(&self) -> Option<usize> {
        self.columns.map(|c| c.max(1) as usize)
    }

    /// Per-file load report for `dots plugins list`.
    pub fn plugins(&self) -> &[PluginInfo] {
        &self.infos
    }

    /// Re-run any pane whose refresh interval has elapsed (and every pane once
    /// on first tick). Returns true if any pane's content changed.
    pub fn tick(&mut self) -> bool {
        let lua = &self.lua;
        let mut changed = false;
        for p in &mut self.panes {
            let due = match p.last {
                None => true,
                Some(t) => t.elapsed() >= p.refresh,
            };
            if due {
                refresh_pane(lua, p);
                changed = true;
            }
        }
        changed
    }

    /// Snapshots for the renderer, in registration order.
    pub fn views(&self) -> Vec<PluginPaneView> {
        self.panes
            .iter()
            .map(|p| PluginPaneView {
                id: p.id.clone(),
                title: p.title.clone(),
                span: p.span,
                weight: p.weight,
                lines: p.lines.clone(),
            })
            .collect()
    }

    /// Run the `on_enter` handler for plugin pane `idx` (index into
    /// [`PluginHost::views`]), then force that pane to re-render. No-op if the
    /// pane has no handler.
    pub fn on_enter(&mut self, idx: usize) {
        let Some(p) = self.panes.get(idx) else { return };
        if let Some(key) = &p.on_enter {
            if let Ok(f) = self.lua.registry_value::<Function>(key) {
                let _ = f.call::<Value>(());
            }
        }
        // Reflect any side effect immediately on next tick.
        if let Some(p) = self.panes.get_mut(idx) {
            p.last = None;
        }
    }
}

/// Call a pane's `render()` and refresh its cached lines. Errors are shown in
/// the pane rather than propagated, so one bad plugin can't take down the UI.
fn refresh_pane(lua: &Lua, p: &mut PaneState) {
    p.last = Some(Instant::now());
    let Some(key) = &p.render else { return };
    let Ok(f) = lua.registry_value::<Function>(key) else { return };
    match f.call::<Value>(()) {
        Ok(v) => p.lines = value_to_lines(v),
        Err(e) => p.lines = vec!["⚠ render error".to_string(), first_line(&e.to_string())],
    }
}

/// Coerce a `render()` return value into display lines: a string is split on
/// newlines; a table is treated as an array of stringifiable values.
fn value_to_lines(v: Value) -> Vec<String> {
    match v {
        Value::String(s) => s.to_string_lossy().lines().map(str::to_string).collect(),
        Value::Table(t) => t
            .sequence_values::<Value>()
            .flatten()
            .map(|item| scalar_to_string(&item))
            .collect(),
        Value::Nil => Vec::new(),
        other => vec![scalar_to_string(&other)],
    }
}

fn scalar_to_string(v: &Value) -> String {
    match v {
        Value::String(s) => s.to_string_lossy().to_string(),
        Value::Integer(i) => i.to_string(),
        Value::Number(n) => n.to_string(),
        Value::Boolean(b) => b.to_string(),
        _ => String::new(),
    }
}

fn first_line(s: &str) -> String {
    s.lines().next().unwrap_or_default().to_string()
}

/// Install the `ui` and `dots` global tables into `lua`.
fn install_api(
    lua: &Lua,
    collector: &Rc<RefCell<Vec<PaneState>>>,
    columns: &Rc<Cell<Option<u16>>>,
) -> mlua::Result<()> {
    let globals = lua.globals();

    // ── ui package ─────────────────────────────────────────────────────────
    let ui = lua.create_table()?;

    let sink = Rc::clone(collector);
    let pane = lua.create_function(move |lua, spec: mlua::Table| {
        let id: String = spec
            .get::<Option<String>>("id")?
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| format!("pane{}", sink.borrow().len() + 1));
        let title: String = spec.get::<Option<String>>("title")?.unwrap_or_else(|| id.clone());
        let span = spec.get::<Option<u16>>("span")?.unwrap_or(1).max(1);
        let weight = spec.get::<Option<u16>>("size")?.unwrap_or(1).max(1);
        let refresh = spec.get::<Option<u64>>("refresh")?.unwrap_or(DEFAULT_REFRESH).max(1);

        let render = match spec.get::<Option<Function>>("render")? {
            Some(f) => Some(lua.create_registry_value(f)?),
            None => None,
        };
        let on_enter = match spec.get::<Option<Function>>("on_enter")? {
            Some(f) => Some(lua.create_registry_value(f)?),
            None => None,
        };

        sink.borrow_mut().push(PaneState {
            id,
            title,
            span,
            weight,
            refresh: Duration::from_secs(refresh),
            render,
            on_enter,
            lines: Vec::new(),
            last: None,
        });
        Ok(())
    })?;
    ui.set("pane", pane)?;

    let cols = Rc::clone(columns);
    let layout = lua.create_function(move |_, spec: mlua::Table| {
        if let Some(n) = spec.get::<Option<u16>>("columns")? {
            cols.set(Some(n.max(1)));
        }
        Ok(())
    })?;
    ui.set("layout", layout)?;
    globals.set("ui", ui)?;

    // ── dots package ───────────────────────────────────────────────────────
    let dots = lua.create_table()?;

    let sh = lua.create_function(|_, cmd: String| {
        let out = std::process::Command::new("sh").arg("-c").arg(&cmd).output();
        Ok(match out {
            Ok(o) => String::from_utf8_lossy(&o.stdout).trim().to_string(),
            Err(_) => String::new(),
        })
    })?;
    dots.set("sh", sh)?;

    let env = lua.create_function(|_, name: String| Ok(std::env::var(name).unwrap_or_default()))?;
    dots.set("env", env)?;

    let dir = lua.create_function(|_, ()| Ok(dots_dir().to_string_lossy().to_string()))?;
    dots.set("dir", dir)?;

    globals.set("dots", dots)?;
    Ok(())
}

// ── tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    /// Build a host from Lua source directly (bypassing the plugins dir).
    fn host_from(src: &str) -> PluginHost {
        let lua = Lua::new();
        let collector: Rc<RefCell<Vec<PaneState>>> = Rc::new(RefCell::new(Vec::new()));
        let columns: Rc<Cell<Option<u16>>> = Rc::new(Cell::new(None));
        install_api(&lua, &collector, &columns).unwrap();
        let info = match lua.load(src).set_name("test").exec() {
            Ok(()) => PluginInfo { name: "test".into(), pane_ids: vec![], error: None },
            Err(e) => PluginInfo { name: "test".into(), pane_ids: vec![], error: Some(e.to_string()) },
        };
        let panes = std::mem::take(&mut *collector.borrow_mut());
        PluginHost { lua, panes, columns: columns.get(), infos: vec![info] }
    }

    #[test]
    fn registers_a_pane_and_renders_lines() {
        let mut h = host_from(
            r#"
            ui.pane{
              id = "hello",
              title = "Hello",
              size = 2,
              span = 2,
              render = function() return { "line one", "line two" } end,
            }
            "#,
        );
        assert!(h.plugins()[0].error.is_none());
        assert_eq!(h.pane_count(), 1);
        h.tick();
        let v = &h.views()[0];
        assert_eq!(v.id, "hello");
        assert_eq!(v.weight, 2);
        assert_eq!(v.span, 2);
        assert_eq!(v.lines, vec!["line one", "line two"]);
    }

    #[test]
    fn string_render_splits_on_newlines() {
        let mut h = host_from(r#"ui.pane{ id="s", render = function() return "a\nb" end }"#);
        h.tick();
        assert_eq!(h.views()[0].lines, vec!["a", "b"]);
    }

    #[test]
    fn layout_sets_columns() {
        let h = host_from("ui.layout{ columns = 3 }");
        assert_eq!(h.columns(), Some(3));
    }

    #[test]
    fn dots_sh_runs_a_command() {
        let mut h = host_from(
            r#"ui.pane{ id="e", render = function() return dots.sh("printf hi") end }"#,
        );
        h.tick();
        assert_eq!(h.views()[0].lines, vec!["hi"]);
    }

    #[test]
    fn render_error_is_contained() {
        let mut h = host_from(r#"ui.pane{ id="boom", render = function() error("nope") end }"#);
        h.tick();
        assert_eq!(h.views()[0].lines[0], "⚠ render error");
    }

    #[test]
    fn broken_plugin_is_reported_not_fatal() {
        let h = host_from("this is not valid lua @@@");
        assert!(h.plugins()[0].error.is_some());
        assert_eq!(h.pane_count(), 0);
    }

    #[test]
    fn defaults_fill_in_missing_fields() {
        let h = host_from(r#"ui.pane{ render = function() return "x" end }"#);
        let v = &h.views()[0];
        assert_eq!(v.span, 1);
        assert_eq!(v.weight, 1);
        assert_eq!(v.id, "pane1");
        assert_eq!(v.title, "pane1");
    }
}
