//! Leaf types (`AppTab`, `WatchState`, `AppEvent`, `AppEffect`, `AppInit`) for the 3-tab cockpit.

use gf_core::ProjectSettingsState;
use gf_engine::{CacheReport, WatchDelta};

use super::explorer_types::ExplorerSnapshot;
use super::keys::Key;

/// Top-level tabs of the interactive `garde tui` cockpit.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum AppTab {
    /// Tab `[1] Diagnostics (Live)` — code frames, filter, OSC 52 copy, hot watch.
    #[default]
    Diagnostics = 0,
    /// Tab `[2] Projet & Garde-fous` — radio archetype + checkbox configurator.
    Config = 1,
    /// Tab `[3] Cache` — BLAKE3 cache status, prune, and cold clean.
    Cache = 2,
}

impl AppTab {
    /// All 3 cockpit tabs in display order.
    pub const ALL: [Self; 3] = [Self::Diagnostics, Self::Config, Self::Cache];

    /// Cycles forward (`Tab`).
    #[must_use]
    pub const fn next(self) -> Self {
        Self::ALL[(self as usize + 1) % Self::ALL.len()]
    }

    /// Cycles backward (`Shift+Tab`).
    #[must_use]
    pub const fn prev(self) -> Self {
        Self::ALL[(self as usize + Self::ALL.len() - 1) % Self::ALL.len()]
    }

    /// Tab bar display title.
    #[must_use]
    pub const fn label(self) -> &'static str {
        const LABELS: [&str; 3] = [
            "[1] Diagnostics (Live)",
            "[2] Projet & Garde-fous",
            "[3] Cache",
        ];
        LABELS[self as usize]
    }
}

/// Live filesystem watch toggle state inside `AppModel`.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum WatchState {
    /// Auto-reloads diagnostics when workspace files change.
    #[default]
    Live,
    /// Watch paused by user (`w`).
    Paused,
}

impl WatchState {
    /// Toggles between [`WatchState::Live`] and [`WatchState::Paused`].
    #[must_use]
    pub const fn toggle(self) -> Self {
        match self {
            Self::Live => Self::Paused,
            Self::Paused => Self::Live,
        }
    }
}

/// Pure event input fed into `AppModel::update`.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AppEvent {
    /// User pressed a key.
    Key(Key),
    /// Workspace filesystem watcher detected a debounced delta.
    FileChanged(WatchDelta),
    /// Periodic animation / heartbeat tick.
    Tick,
}

/// Imperative side-effect emitted by `AppModel::update` to the terminal driver.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AppEffect {
    /// Only re-render the current cockpit view.
    Redraw,
    /// Run a hot incremental check (`BLAKE3` cache) and feed the new snapshot back.
    ReloadCheck,
    /// Write an `OSC 52` clipboard escape sequence to the terminal.
    CopyOsc52(String),
    /// Persist `garde-fou.toml` losslessly + `run_sync` + re-check.
    ApplyConfig(ProjectSettingsState),
    /// Prune obsolete cache schemas & orphaned entries and refresh Tab `[3]`.
    PruneCache,
    /// Remove `.garde-fou-cache/` completely, refresh Tab `[3]`, and run a cold check.
    CleanCache,
    /// Exit the TUI cleanly.
    Quit,
}

/// Initial state bundle for `AppModel::new`.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AppInit {
    /// Starting active tab (`AppTab::Diagnostics` for `tui`, `AppTab::Config` for `config`).
    pub initial_tab: AppTab,
    /// Initial diagnostic snapshot.
    pub snapshot: ExplorerSnapshot,
    /// Initial project settings loaded from `garde-fou.toml` + disk detection.
    pub settings: ProjectSettingsState,
    /// Initial BLAKE3 cache status report.
    pub cache_report: CacheReport,
}
