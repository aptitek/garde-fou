//! Pure Bubble Tea-inspired 3-tab cockpit state machine (`AppModel`) for `garde tui`,
//! `garde check -i`, and `garde config [PATH]`.

use gf_core::ProjectSettingsState;
use gf_engine::{CacheCleanSummary, CacheReport};

use super::app_types::{AppEffect, AppEvent, AppInit, AppTab, WatchState};
use super::cache_tab::{CacheTabAction, CacheTabModel};
use super::config_tab::{ConfigTabAction, ConfigTabModel};
use super::explorer_model::ExplorerModel;
use super::explorer_types::{ExplorerAction, ExplorerMode, ExplorerSnapshot};
use super::keys::Key;

/// Pure, deterministic 3-tab cockpit state machine.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AppModel {
    pub(crate) active_tab: AppTab,
    pub(crate) watch_state: WatchState,
    pub(crate) explorer: ExplorerModel,
    pub(crate) config: ConfigTabModel,
    pub(crate) cache: CacheTabModel,
    pub(crate) last_watch_summary: Option<String>,
    pub(crate) tick_count: u64,
}

impl AppModel {
    /// Constructs a new [`AppModel`] from `init`.
    #[must_use]
    pub fn new(init: AppInit) -> Self {
        Self {
            active_tab: init.initial_tab,
            watch_state: WatchState::Live,
            explorer: ExplorerModel::new(init.snapshot),
            config: ConfigTabModel::new(init.settings),
            cache: CacheTabModel::new(init.cache_report),
            last_watch_summary: None,
            tick_count: 0,
        }
    }

    /// Returns the currently focused [`AppTab`].
    #[must_use]
    pub const fn active_tab(&self) -> AppTab {
        self.active_tab
    }

    /// Returns the live watch status ([`WatchState::Live`] or [`WatchState::Paused`]).
    #[must_use]
    pub const fn watch_state(&self) -> WatchState {
        self.watch_state
    }

    /// Returns `true` when the latest check snapshot has zero violations.
    #[must_use]
    pub const fn is_clean(&self) -> bool {
        self.explorer.is_clean()
    }

    /// Borrows the inner [`ConfigTabModel`] (Tab `[2]`).
    #[must_use]
    pub const fn config(&self) -> &ConfigTabModel {
        &self.config
    }

    /// Replaces the diagnostic snapshot and cache report after a reload.
    pub fn apply_reload(&mut self, snapshot: ExplorerSnapshot, cache_report: CacheReport) {
        self.explorer.replace_snapshot(snapshot);
        self.cache.report = cache_report;
    }

    /// Commits saved settings after `apply_project_settings` completes.
    pub fn apply_saved_config(&mut self, saved: ProjectSettingsState) {
        self.config.mark_saved(saved);
    }

    /// Updates Tab `[3] Cache` after a prune or clean operation.
    pub fn apply_cache_op(&mut self, op: &str, summary: CacheCleanSummary, updated: CacheReport) {
        self.cache.apply_clean_result(op, summary, updated);
    }

    /// Pure Elm update transition `(model, event) -> AppEffect`.
    pub fn update(&mut self, event: AppEvent) -> AppEffect {
        match event {
            AppEvent::Key(key) => handle_app_key(self, key),
            AppEvent::FileChanged(delta) => {
                if self.watch_state == WatchState::Live {
                    self.last_watch_summary = Some(delta.summary_short());
                    AppEffect::ReloadCheck
                } else {
                    AppEffect::Redraw
                }
            }
            AppEvent::Tick => {
                self.tick_count = self.tick_count.wrapping_add(1);
                AppEffect::Redraw
            }
        }
    }
}

fn handle_app_key(m: &mut AppModel, key: Key) -> AppEffect {
    if key == Key::CtrlC {
        return AppEffect::Quit;
    }
    if m.active_tab == AppTab::Diagnostics && m.explorer.mode() == ExplorerMode::Filtering {
        return map_explorer_action(m.explorer.handle_key(key));
    }
    if let Some(effect) = handle_global_key(m, key) {
        return effect;
    }
    match m.active_tab {
        AppTab::Diagnostics => map_explorer_action(m.explorer.handle_key(key)),
        AppTab::Config => map_config_action(m.config.handle_key(key)),
        AppTab::Cache => map_cache_action(m.cache.handle_key(key)),
    }
}

const fn handle_global_key(m: &mut AppModel, key: Key) -> Option<AppEffect> {
    match key {
        Key::Tab => m.active_tab = m.active_tab.next(),
        Key::BackTab => m.active_tab = m.active_tab.prev(),
        Key::Char('1') => m.active_tab = AppTab::Diagnostics,
        Key::Char('2') => m.active_tab = AppTab::Config,
        Key::Char('3') => m.active_tab = AppTab::Cache,
        Key::Char('w') => m.watch_state = m.watch_state.toggle(),
        Key::Char('q') => return Some(AppEffect::Quit),
        _ => return None,
    }
    Some(AppEffect::Redraw)
}

fn map_explorer_action(action: ExplorerAction) -> AppEffect {
    match action {
        ExplorerAction::Redraw => AppEffect::Redraw,
        ExplorerAction::Reload => AppEffect::ReloadCheck,
        ExplorerAction::CopyOsc52(seq) => AppEffect::CopyOsc52(seq),
        ExplorerAction::Quit => AppEffect::Quit,
    }
}

fn map_config_action(action: ConfigTabAction) -> AppEffect {
    match action {
        ConfigTabAction::Redraw | ConfigTabAction::Reverted => AppEffect::Redraw,
        ConfigTabAction::SaveAndSync(state) => AppEffect::ApplyConfig(state),
    }
}

const fn map_cache_action(action: CacheTabAction) -> AppEffect {
    match action {
        CacheTabAction::Redraw => AppEffect::Redraw,
        CacheTabAction::Prune => AppEffect::PruneCache,
        CacheTabAction::Clean => AppEffect::CleanCache,
    }
}
