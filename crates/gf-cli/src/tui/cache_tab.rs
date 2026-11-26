//! Pure Elm-style model and Lipgloss view for Tab `[3] Cache` inside `garde tui`.

use gf_engine::{CacheCleanSummary, CacheReport};

use super::keys::Key;
use crate::ui::{
    ColorPolicy, Role, format_bytes_human, paint, paint_bold, render_cache_status_human,
};

/// Action emitted by [`CacheTabModel::handle_key`].
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CacheTabAction {
    /// Re-render only.
    Redraw,
    /// Run `cache_prune` (remove stale schemas & orphans) and refresh report.
    Prune,
    /// Run `cache_clean` (delete `.garde-fou-cache/` completely) and trigger cold check.
    Clean,
}

/// Pure state container for Tab `[3] Cache`.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CacheTabModel {
    pub(crate) report: CacheReport,
    pub(crate) notice: Option<String>,
}

impl CacheTabModel {
    /// Creates a new [`CacheTabModel`] from `report`.
    #[must_use]
    pub const fn new(report: CacheReport) -> Self {
        Self {
            report,
            notice: None,
        }
    }

    /// Updates the displayed [`CacheReport`] and records a clean/prune notice.
    pub fn apply_clean_result(
        &mut self,
        op: &str,
        summary: CacheCleanSummary,
        updated: CacheReport,
    ) {
        self.report = updated;
        self.notice = Some(format!(
            "cache {op} : {} entrée(s), {} dossier(s), {} libérés",
            summary.removed_entries,
            summary.removed_dirs,
            format_bytes_human(summary.freed_bytes),
        ));
    }

    /// Processes a single [`Key`] event in Tab `[3] Cache`.
    #[must_use]
    pub const fn handle_key(&self, key: Key) -> CacheTabAction {
        match key {
            Key::Char('p') => CacheTabAction::Prune,
            Key::Char('x') => CacheTabAction::Clean,
            _ => CacheTabAction::Redraw,
        }
    }
}

/// Renders Tab `[3] Cache` as a Lipgloss card with `[p] prune` and `[x] clean` shortcuts.
#[must_use]
pub fn render_cache_tab(model: &CacheTabModel, policy: ColorPolicy, max_width: usize) -> String {
    let card = render_cache_status_human(&model.report, policy, max_width);
    let notice = model.notice.as_deref().map_or_else(String::new, |msg| {
        format!(
            "\n  {}",
            paint_bold(&format!("✓ {msg}"), Role::Success, policy)
        )
    });
    let help = paint(
        "  [p] prune (schémas anciens & orphelines) · [x] clean complet + check à froid · [Tab/1-3] onglets · [q] quitter",
        Role::Muted,
        policy,
    );
    format!("{card}{notice}\n{help}")
}
