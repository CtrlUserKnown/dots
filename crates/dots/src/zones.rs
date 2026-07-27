//! User-definable dashboard zones.
//!
//! The dashboard used to be a fixed grid: five built-in panes in a hard-coded
//! order, with plugin panes appended to the end. A zone layout replaces that
//! with a two-level model the user owns:
//!
//! * A **zone** is a region of the dashboard — it takes a column `span` and a
//!   row-height `weight` in the top-level grid, and lays its own widgets out in
//!   `columns` of its own. Give it a `title` and it draws a labelled border
//!   around its widgets; leave the title off and the zone is invisible chrome,
//!   just a region.
//! * A **widget** is one tile inside a zone, named by id. Built-ins are
//!   [`BUILTIN_WIDGETS`]; anything else is a plugin pane id.
//!
//! The layout lives in `~/.dots/layout.toml`. With no file, [`Layout::default`]
//! reproduces the previous dashboard exactly — one untitled catch-all zone
//! holding the five built-ins in their original order — so the feature is
//! invisible until someone opts in.
//!
//! Plugins participate from both sides: `ui.pane{ zone = "…" }` places a pane
//! into a zone, and `ui.zone{ … }` declares a zone that the user's file hasn't.
//! Resolution ([`resolve`]) turns a layout plus the session's plugin panes into
//! the concrete draw order, reporting anything it could not place rather than
//! silently dropping it.

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

use crate::config::settings::dots_dir;

/// Widget ids the dashboard knows how to draw itself. Anything else in a zone's
/// `widgets` list is looked up among the loaded plugin panes.
pub const BUILTIN_WIDGETS: [&str; 5] = ["symlinks", "tools", "configs", "plugins", "network"];

/// Default number of columns in the top-level zone grid.
pub const DEFAULT_COLS: u16 = 2;

// ── the layout file ───────────────────────────────────────────────────────────

/// One region of the dashboard.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Zone {
    /// Stable id — what `ui.pane{ zone = … }` targets.
    pub id: String,
    /// Draws a labelled border around the zone. Omit for an invisible region.
    pub title: Option<String>,
    /// Columns spanned in the top-level grid.
    pub span: u16,
    /// Row-height weight relative to other zones.
    pub weight: u16,
    /// Columns the zone's own widgets are laid out in.
    pub columns: u16,
    /// Widget ids, in draw order: built-ins and/or plugin pane ids.
    pub widgets: Vec<String>,
    /// Plugin panes that name no zone land here. The first such zone wins.
    pub catch_all: bool,
}

impl Default for Zone {
    fn default() -> Self {
        Self {
            id: String::new(),
            title: None,
            span: 1,
            weight: 1,
            columns: 1,
            widgets: Vec::new(),
            catch_all: false,
        }
    }
}

impl Zone {
    /// A zone with sensible geometry, for plugin-declared zones and tests.
    pub fn new(id: impl Into<String>) -> Self {
        Self { id: id.into(), ..Default::default() }
    }
}

/// The whole dashboard layout.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Layout {
    /// Columns in the top-level grid that zones are placed into.
    pub columns: u16,
    #[serde(rename = "zones")]
    pub zones: Vec<Zone>,
}

impl Default for Layout {
    /// The pre-zones dashboard: one invisible catch-all zone holding the five
    /// built-ins across two columns. Rendering this must be indistinguishable
    /// from the old fixed grid — [`tests::default_layout_matches_legacy_grid`]
    /// pins that.
    fn default() -> Self {
        Self {
            columns: 1,
            zones: vec![Zone {
                id: "main".into(),
                title: None,
                span: 1,
                weight: 1,
                columns: DEFAULT_COLS,
                widgets: BUILTIN_WIDGETS.iter().map(|s| s.to_string()).collect(),
                catch_all: true,
            }],
        }
    }
}

/// Where the layout file lives.
pub fn layout_path() -> PathBuf {
    dots_dir().join("layout.toml")
}

/// Prepended by [`Layout::save`] so the scaffolded file explains itself.
const HEADER: &str = "\
# dots dashboard layout.
#
# `columns` is how many columns zones are arranged in; each zone's `span` is how
# many of those it takes, and `weight` is its height relative to other zones.
# Within a zone, `columns` arranges its own widgets the same way.
#
# `widgets` lists tiles by id, in draw order. The built-ins are:
#   symlinks, tools, configs, plugins, network
# A plugin pane joins by its own id — either listed here, or by setting
# `zone = \"<id>\"` in its `ui.pane{}` call. Panes that do neither land in the
# zone marked `catch_all`.
#
# Give a zone a `title` to draw a labelled border around it; leave it out and
# the zone is an invisible region. Delete this file to go back to the default.

";

impl Layout {
    /// Read `~/.dots/layout.toml`. `Ok(None)` means no file exists — callers
    /// fall back to [`Layout::default`].
    ///
    /// A parse error is returned rather than swallowed, but no caller treats it
    /// as fatal: a broken layout must never lock the user out of the TUI they
    /// need in order to fix it, so the dashboard falls back to the default and
    /// says so.
    pub fn try_load() -> Result<Option<Self>> {
        let path = layout_path();
        if !path.exists() {
            return Ok(None);
        }
        let text = std::fs::read_to_string(&path)
            .with_context(|| format!("reading {}", path.display()))?;
        let layout: Layout = toml::from_str(&text)
            .with_context(|| format!("parsing {}", path.display()))?;
        Ok(Some(layout.normalized()))
    }

    /// Write this layout to `~/.dots/layout.toml`, via a temp file so a crashed
    /// write can't leave a half-parsed layout behind (same approach as
    /// `config::settings::save`).
    ///
    /// The file is written with a comment header: this is a file people edit by
    /// hand, and the field names alone don't explain what `span` means against
    /// `columns`, or where an unplaced plugin pane ends up.
    pub fn save(&self) -> Result<()> {
        let path = layout_path();
        let dir = path.parent().expect("layout path has parent");
        std::fs::create_dir_all(dir).with_context(|| format!("creating {}", dir.display()))?;

        let tmp = path.with_extension("toml.tmp");
        let text = format!("{HEADER}{}", toml::to_string_pretty(self).context("serializing layout")?);
        std::fs::write(&tmp, &text).with_context(|| format!("writing {}", tmp.display()))?;
        std::fs::rename(&tmp, &path).with_context(|| format!("renaming into {}", path.display()))?;
        Ok(())
    }

    /// Clamp geometry to usable values and drop unusable zones, so a
    /// hand-written file can't produce a zero-width grid or an unaddressable
    /// zone.
    fn normalized(mut self) -> Self {
        self.columns = self.columns.max(1);
        self.zones.retain(|z| !z.id.is_empty());
        for z in &mut self.zones {
            z.span = z.span.clamp(1, self.columns);
            z.weight = z.weight.max(1);
            z.columns = z.columns.max(1);
        }
        if self.zones.is_empty() {
            return Self::default();
        }
        self
    }

    /// Index of the zone unassigned plugin panes belong to: the first zone
    /// flagged `catch_all`, else the last zone, so a pane is never lost.
    fn catch_all(&self) -> usize {
        self.zones
            .iter()
            .position(|z| z.catch_all)
            .unwrap_or(self.zones.len().saturating_sub(1))
    }

    /// Merge zones a plugin declared. Zones the layout already defines win —
    /// the user's file is authoritative over a plugin's suggestion — so this
    /// only appends genuinely new ones.
    pub fn merge_plugin_zones(&mut self, declared: &[Zone]) {
        for z in declared {
            if z.id.is_empty() || self.zones.iter().any(|e| e.id == z.id) {
                continue;
            }
            let mut z = z.clone();
            z.span = z.span.clamp(1, self.columns);
            z.weight = z.weight.max(1);
            z.columns = z.columns.max(1);
            self.zones.push(z);
        }
    }
}

// ── resolution ────────────────────────────────────────────────────────────────

/// One tile, resolved to something the renderer can draw directly.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Widget {
    /// A built-in pane, named by its entry in [`BUILTIN_WIDGETS`].
    Builtin(&'static str),
    /// Index into the plugin panes passed to [`resolve`].
    Plugin(usize),
}

/// A zone with its widgets resolved and its geometry clamped.
#[derive(Debug, Clone)]
pub struct ResolvedZone {
    pub id: String,
    pub title: Option<String>,
    pub span: u16,
    pub weight: u16,
    pub columns: u16,
    pub widgets: Vec<Widget>,
}

/// The dashboard, ready to draw.
#[derive(Debug, Clone, Default)]
pub struct Resolved {
    pub columns: u16,
    pub zones: Vec<ResolvedZone>,
    /// Anything the layout asked for that could not be placed — an unknown
    /// widget id, or a pane naming a zone that does not exist. Surfaced by
    /// `dots layout show` instead of failing.
    pub warnings: Vec<String>,
}

impl Resolved {
    /// Every widget in draw order (zone by zone), parallel to the rects the
    /// renderer computes. This flattening is what lets focus and hit-testing
    /// stay a single index across zones.
    pub fn flat(&self) -> Vec<&Widget> {
        self.zones.iter().flat_map(|z| z.widgets.iter()).collect()
    }

    /// Total widget count across all zones.
    pub fn len(&self) -> usize {
        self.zones.iter().map(|z| z.widgets.len()).sum()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

/// What [`resolve`] needs to know about one plugin pane: its id and the zone it
/// asked for (`None` = wherever the catch-all is).
pub struct PanePlacement<'a> {
    pub id: &'a str,
    pub zone: Option<&'a str>,
}

/// Turn a layout plus this session's plugin panes into concrete draw order.
///
/// Widget ids in a zone's `widgets` list are matched first, so a user who lists
/// a plugin pane explicitly controls exactly where it sits. Plugin panes not
/// listed anywhere are appended to the zone they named, or to the catch-all.
pub fn resolve(layout: &Layout, panes: &[PanePlacement<'_>]) -> Resolved {
    let mut warnings = Vec::new();
    let mut placed = vec![false; panes.len()];

    // Pass 1: everything the layout names explicitly, in the order it names it.
    let mut zones: Vec<ResolvedZone> = layout
        .zones
        .iter()
        .map(|z| {
            let mut widgets = Vec::new();
            for id in &z.widgets {
                if let Some(b) = BUILTIN_WIDGETS.iter().find(|b| *b == id) {
                    widgets.push(Widget::Builtin(b));
                } else if let Some(i) = panes.iter().position(|p| p.id == id) {
                    placed[i] = true;
                    widgets.push(Widget::Plugin(i));
                } else {
                    warnings.push(format!(
                        "zone '{}': unknown widget '{id}' — not a built-in, and no plugin pane has that id",
                        z.id
                    ));
                }
            }
            ResolvedZone {
                id: z.id.clone(),
                title: z.title.clone(),
                span: z.span.clamp(1, layout.columns.max(1)),
                weight: z.weight.max(1),
                columns: z.columns.max(1),
                widgets,
            }
        })
        .collect();

    if zones.is_empty() {
        return Resolved { columns: layout.columns.max(1), zones, warnings };
    }

    // Pass 2: plugin panes nobody listed, appended to the zone they asked for.
    let fallback = layout.catch_all();
    for (i, pane) in panes.iter().enumerate() {
        if placed[i] {
            continue;
        }
        let target = match pane.zone {
            None => fallback,
            Some(name) => match zones.iter().position(|z| z.id == name) {
                Some(z) => z,
                None => {
                    warnings.push(format!(
                        "pane '{}' asked for zone '{name}', which no zone defines — placed in '{}'",
                        pane.id, zones[fallback].id
                    ));
                    fallback
                }
            },
        };
        zones[target].widgets.push(Widget::Plugin(i));
    }

    Resolved { columns: layout.columns.max(1), zones, warnings }
}

// ── tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn panes<'a>(spec: &[(&'a str, Option<&'a str>)]) -> Vec<PanePlacement<'a>> {
        spec.iter().map(|(id, zone)| PanePlacement { id, zone: *zone }).collect()
    }

    #[test]
    fn default_layout_matches_legacy_grid() {
        let r = resolve(&Layout::default(), &[]);
        assert_eq!(r.zones.len(), 1);
        assert_eq!(r.zones[0].columns, 2, "legacy dashboard was two columns");
        assert!(r.zones[0].title.is_none(), "legacy dashboard had no zone chrome");
        assert_eq!(
            r.zones[0].widgets,
            BUILTIN_WIDGETS.iter().map(|b| Widget::Builtin(b)).collect::<Vec<_>>(),
            "built-ins must keep their original order"
        );
        assert!(r.warnings.is_empty());
    }

    #[test]
    fn unlisted_plugin_panes_go_to_the_catch_all() {
        let r = resolve(&Layout::default(), &panes(&[("gh", None), ("aws", None)]));
        assert_eq!(r.len(), BUILTIN_WIDGETS.len() + 2);
        assert_eq!(r.zones[0].widgets.last(), Some(&Widget::Plugin(1)));
        assert!(r.warnings.is_empty());
    }

    #[test]
    fn a_pane_can_target_a_zone() {
        let layout = Layout {
            columns: 2,
            zones: vec![
                Zone { id: "top".into(), span: 2, widgets: vec!["symlinks".into()], ..Zone::new("top") },
                Zone { id: "side".into(), catch_all: true, ..Zone::new("side") },
            ],
        };
        let r = resolve(&layout, &panes(&[("gh", Some("top")), ("aws", None)]));
        assert_eq!(r.zones[0].widgets, vec![Widget::Builtin("symlinks"), Widget::Plugin(0)]);
        assert_eq!(r.zones[1].widgets, vec![Widget::Plugin(1)]);
        assert!(r.warnings.is_empty());
    }

    #[test]
    fn explicit_placement_beats_the_zone_hint() {
        // Listing a pane in a zone's widgets pins it there even though the pane
        // itself asked for somewhere else — the user's file is authoritative.
        let layout = Layout {
            columns: 2,
            zones: vec![
                Zone { id: "left".into(), widgets: vec!["gh".into()], ..Zone::new("left") },
                Zone { id: "right".into(), catch_all: true, ..Zone::new("right") },
            ],
        };
        let r = resolve(&layout, &panes(&[("gh", Some("right"))]));
        assert_eq!(r.zones[0].widgets, vec![Widget::Plugin(0)]);
        assert!(r.zones[1].widgets.is_empty());
    }

    #[test]
    fn unknown_widget_id_warns_and_is_skipped() {
        let layout = Layout {
            columns: 1,
            zones: vec![Zone { widgets: vec!["symlinks".into(), "nope".into()], ..Zone::new("main") }],
        };
        let r = resolve(&layout, &[]);
        assert_eq!(r.zones[0].widgets, vec![Widget::Builtin("symlinks")]);
        assert_eq!(r.warnings.len(), 1);
        assert!(r.warnings[0].contains("nope"));
    }

    #[test]
    fn pane_naming_a_missing_zone_falls_back_with_a_warning() {
        let r = resolve(&Layout::default(), &panes(&[("gh", Some("ghost"))]));
        assert_eq!(r.zones[0].widgets.last(), Some(&Widget::Plugin(0)));
        assert_eq!(r.warnings.len(), 1);
        assert!(r.warnings[0].contains("ghost"));
    }

    #[test]
    fn no_catch_all_falls_back_to_the_last_zone() {
        let layout = Layout {
            columns: 1,
            zones: vec![Zone::new("a"), Zone::new("b")],
        };
        let r = resolve(&layout, &panes(&[("gh", None)]));
        assert_eq!(r.zones[1].widgets, vec![Widget::Plugin(0)]);
    }

    #[test]
    fn normalize_clamps_geometry_and_drops_nameless_zones() {
        let l = Layout {
            columns: 0,
            zones: vec![
                Zone { span: 9, weight: 0, columns: 0, ..Zone::new("wide") },
                Zone::new(""),
            ],
        }
        .normalized();
        assert_eq!(l.columns, 1);
        assert_eq!(l.zones.len(), 1, "the id-less zone is dropped");
        assert_eq!(l.zones[0].span, 1, "span is clamped to the column count");
        assert_eq!(l.zones[0].weight, 1);
        assert_eq!(l.zones[0].columns, 1);
    }

    #[test]
    fn normalize_falls_back_when_every_zone_is_dropped() {
        let l = Layout { columns: 2, zones: vec![Zone::new("")] }.normalized();
        assert_eq!(l.zones.len(), 1);
        assert_eq!(l.zones[0].id, "main");
    }

    #[test]
    fn plugin_zones_never_override_the_users_own() {
        let mut l = Layout {
            columns: 2,
            zones: vec![Zone { columns: 3, ..Zone::new("side") }],
        };
        l.merge_plugin_zones(&[
            Zone { columns: 9, ..Zone::new("side") },
            Zone { span: 7, ..Zone::new("extra") },
        ]);
        assert_eq!(l.zones.len(), 2);
        assert_eq!(l.zones[0].columns, 3, "the user's zone is untouched");
        assert_eq!(l.zones[1].id, "extra");
        assert_eq!(l.zones[1].span, 2, "a plugin zone's span is clamped to the grid");
    }

    #[test]
    fn roundtrips_through_toml() {
        let text = toml::to_string_pretty(&Layout::default()).unwrap();
        let back: Layout = toml::from_str(&text).unwrap();
        assert_eq!(back.zones[0].widgets, Layout::default().zones[0].widgets);
        assert_eq!(back.zones[0].columns, DEFAULT_COLS);
    }

    #[test]
    fn a_partial_zone_table_fills_in_defaults() {
        // What a hand-written file actually looks like: an id and some widgets.
        let l: Layout = toml::from_str(
            r#"
            columns = 2
            [[zones]]
            id = "top"
            widgets = ["symlinks", "tools"]
            "#,
        )
        .unwrap();
        assert_eq!(l.zones[0].span, 1);
        assert_eq!(l.zones[0].weight, 1);
        assert!(!l.zones[0].catch_all);
        assert_eq!(resolve(&l, &[]).len(), 2);
    }

    #[test]
    fn flat_walks_zones_in_order() {
        let layout = Layout {
            columns: 2,
            zones: vec![
                Zone { widgets: vec!["symlinks".into()], ..Zone::new("a") },
                Zone { widgets: vec!["tools".into(), "network".into()], ..Zone::new("b") },
            ],
        };
        let r = resolve(&layout, &[]);
        assert_eq!(
            r.flat(),
            vec![&Widget::Builtin("symlinks"), &Widget::Builtin("tools"), &Widget::Builtin("network")]
        );
        assert_eq!(r.len(), 3);
    }
}
