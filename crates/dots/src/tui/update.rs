//! Update state, embedded as a row in the Settings popup rather than its own
//! screen. Owns the apply/re-check background work; reads the check result
//! held on [`App`] (populated by the background check the event loop kicks
//! off at startup). Mirrors the self-update UX in `ssm`.

use std::sync::mpsc::Receiver;

use crate::tui::app::App;
use crate::update::{self as update_core, UpdateInfo};

// ── state ─────────────────────────────────────────────────────────────────────

pub struct UpdateScreen {
    /// Background re-check kicked off from the settings popup (separate from
    /// the initial check the event loop runs on launch).
    recheck_rx: Option<Receiver<anyhow::Result<Option<UpdateInfo>>>>,
    apply_rx:   Option<Receiver<anyhow::Result<()>>>,
    applying:   bool,
    /// True once the swap succeeded — the row then asks for a restart.
    done:       bool,
}

impl Default for UpdateScreen {
    fn default() -> Self { Self::new() }
}

impl UpdateScreen {
    pub fn new() -> Self {
        Self { recheck_rx: None, apply_rx: None, applying: false, done: false }
    }

    /// Reset transient state when (re)entering settings. Kept for parity with
    /// the navigation flow; the underlying check result lives on `App`.
    pub fn sync_from_app(&mut self, _app: &App) {
        // A completed update stays "done" until the process restarts, so we only
        // clear the in-flight flag when nothing is running.
        if !self.applying {
            self.recheck_rx = None;
        }
    }

    /// Drain the apply/re-check channels. Called each tick from the event loop.
    /// `app` is updated in place so the dashboard summary reflects a re-check.
    pub fn pump(&mut self, app: &mut App) {
        if let Some(rx) = &self.recheck_rx {
            if let Ok(result) = rx.try_recv() {
                self.recheck_rx = None;
                match result {
                    Ok(info)  => { app.update_info = info; app.update_error = None; }
                    Err(e)    => app.update_error = Some(e.to_string()),
                }
            }
        }

        if let Some(rx) = &self.apply_rx {
            if let Ok(result) = rx.try_recv() {
                self.applying = false;
                self.apply_rx = None;
                match result {
                    Ok(())  => self.done = true,
                    Err(e)  => app.update_error = Some(format!("update failed: {e}")),
                }
            }
        }
    }

    fn start_apply(&mut self, app: &App) {
        if self.applying || self.done { return; }
        if let Some(info) = app.update_info.clone() {
            self.applying = true;
            self.apply_rx = Some(update_core::spawn_apply(info));
        }
    }

    fn start_recheck(&mut self, app: &mut App) {
        if self.applying || self.recheck_rx.is_some() { return; }
        app.update_error = None;
        // Force a check regardless of the throttle by passing a zero interval.
        self.recheck_rx = Some(update_core::spawn_check(0));
    }

    /// Do the sensible thing for the current state when the settings row is
    /// activated: apply an available update, or kick off a re-check when
    /// there's nothing to apply yet. A no-op once done, mid-flight, or when
    /// this install is managed by a package manager.
    pub fn activate(&mut self, app: &mut App) {
        if self.done || self.applying { return; }
        if app.install_source.defer_message().is_some() { return; }
        if app.update_info.is_some() {
            self.start_apply(app);
        } else {
            self.start_recheck(app);
        }
    }

    /// (status text, is_positive) for the settings row's value column.
    pub fn status(&self, app: &App) -> (String, bool) {
        if self.done {
            ("restart to finish".into(), true)
        } else if self.applying {
            ("installing…".into(), true)
        } else if let Some(msg) = app.install_source.defer_message() {
            (msg, true)
        } else if let Some(info) = &app.update_info {
            (format!("v{} available", info.latest), false)
        } else if self.recheck_rx.is_some() {
            ("checking…".into(), true)
        } else if app.update_error.is_some() {
            ("check failed".into(), false)
        } else {
            ("up to date".into(), true)
        }
    }

    /// One-line description shown under the settings rows when this one's focused.
    pub fn desc(&self, app: &App) -> String {
        if self.done {
            "restart dots to run the new version".into()
        } else if self.applying {
            "downloading and installing… this can take a moment".into()
        } else if let Some(msg) = app.install_source.defer_message() {
            format!("installed via a package manager — {msg}")
        } else if app.update_info.is_some() {
            format!("current v{} — enter to install", update_core::CURRENT)
        } else if self.recheck_rx.is_some() {
            "checking for updates…".into()
        } else if let Some(err) = &app.update_error {
            format!("✗ {err} — enter to retry")
        } else {
            format!("current v{} — enter to check again", update_core::CURRENT)
        }
    }
}
