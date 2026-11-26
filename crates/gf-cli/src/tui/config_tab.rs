//! Pure Elm-style state machine (`ConfigTabModel`) for Tab `[2] Projet & Garde-fous`
//! and standalone `garde config [PATH]`.

use gf_core::{GovernanceFlag, ProjectSettingsState};

use super::config_tab_catalog::{
    ALL_ARCHETYPE_SETTINGS, ALL_FEATURES, ALL_FRAMEWORKS, ALL_LANGUAGES, ConfigCategory,
};
use super::keys::Key;

/// Action returned by [`ConfigTabModel::handle_key`].
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ConfigTabAction {
    /// Only redraw the config tab.
    Redraw,
    /// Persist `garde-fou.toml` losslessly via `toml_edit`, sync `AGENTS.md`/`garde.lock`, and re-check.
    SaveAndSync(ProjectSettingsState),
    /// Reverted all unsaved draft changes back to baseline.
    Reverted,
}

/// Pure, deterministic state machine for the interactive checkbox configurator.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ConfigTabModel {
    pub(crate) baseline: ProjectSettingsState,
    pub(crate) draft: ProjectSettingsState,
    pub(crate) category: ConfigCategory,
    pub(crate) cursor: usize,
    pub(crate) notice: Option<String>,
}

impl ConfigTabModel {
    /// Creates a new [`ConfigTabModel`] initialized from `settings`.
    #[must_use]
    pub fn new(settings: ProjectSettingsState) -> Self {
        Self {
            baseline: settings.clone(),
            draft: settings,
            category: ConfigCategory::Archetype,
            cursor: 0,
            notice: None,
        }
    }

    /// Returns the active [`ConfigCategory`].
    #[must_use]
    pub const fn category(&self) -> ConfigCategory {
        self.category
    }

    /// Returns the current 0-indexed row cursor within `self.category()`.
    #[must_use]
    pub const fn cursor(&self) -> usize {
        self.cursor
    }

    /// Returns the live draft [`ProjectSettingsState`].
    #[must_use]
    pub const fn draft(&self) -> &ProjectSettingsState {
        &self.draft
    }

    /// Returns the number of unsaved changes relative to `baseline`.
    #[must_use]
    pub fn unsaved_changes(&self) -> usize {
        self.draft.diff_count(&self.baseline)
    }

    /// Commits `saved` as the new baseline after `apply_project_settings` succeeds.
    pub fn mark_saved(&mut self, saved: ProjectSettingsState) {
        self.baseline = saved.clone();
        self.draft = saved;
        self.notice = Some(String::from(
            "Configuration enregistrée et synchronisée (garde-fou.toml + AGENTS.md + garde.lock)",
        ));
    }

    /// Processes a single [`Key`] event and returns a [`ConfigTabAction`].
    pub fn handle_key(&mut self, key: Key) -> ConfigTabAction {
        let len = self.category.len();
        match key {
            Key::Left | Key::Char('h') => {
                self.category = self.category.prev();
                self.cursor = 0;
            }
            Key::Right | Key::Char('l') => {
                self.category = self.category.next();
                self.cursor = 0;
            }
            Key::Up | Key::Char('k') => self.cursor = self.cursor.saturating_sub(1),
            Key::Down | Key::Char('j') => {
                self.cursor = (self.cursor + 1).min(len.saturating_sub(1));
            }
            Key::Home | Key::Char('g') => self.cursor = 0,
            Key::End | Key::Char('G') => self.cursor = len.saturating_sub(1),
            Key::Char(' ' | 'x') => toggle_current_item(self),
            Key::Char('s') | Key::Enter => return ConfigTabAction::SaveAndSync(self.draft.clone()),
            Key::Char('u') | Key::Esc => {
                self.draft = self.baseline.clone();
                self.notice = Some(String::from("Modifications annulées"));
                return ConfigTabAction::Reverted;
            }
            _ => {}
        }
        ConfigTabAction::Redraw
    }
}

fn toggle_indexed<T: Ord + Copy>(
    set: &mut std::collections::BTreeSet<T>,
    items: &[T],
    cursor: usize,
) {
    if let Some(&item) = items.get(cursor)
        && !set.remove(&item)
    {
        set.insert(item);
    }
}

fn toggle_current_item(m: &mut ConfigTabModel) {
    match m.category {
        ConfigCategory::Archetype => {
            if let Some(&setting) = ALL_ARCHETYPE_SETTINGS.get(m.cursor) {
                m.draft.archetype = setting;
            }
        }
        ConfigCategory::Languages => {
            toggle_indexed(&mut m.draft.languages, &ALL_LANGUAGES, m.cursor);
        }
        ConfigCategory::Frameworks => {
            toggle_indexed(&mut m.draft.frameworks, &ALL_FRAMEWORKS, m.cursor);
        }
        ConfigCategory::Features => toggle_indexed(&mut m.draft.features, &ALL_FEATURES, m.cursor),
        ConfigCategory::Governance => {
            if let Some(&flag) = GovernanceFlag::ALL.get(m.cursor) {
                m.draft.governance.toggle(flag);
            }
        }
    }
}
