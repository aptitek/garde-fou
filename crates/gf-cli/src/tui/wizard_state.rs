//! Pure state container (`WizardModel`) and request assembly for the `garde init` wizard.

use std::collections::BTreeSet;
use std::path::PathBuf;

use gf_core::WorkspaceArchetype;
use gf_engine::InitRequest;

use super::wizard_data::{ARCHETYPE_CHOICES, FEATURE_CHOICES, StackToken, stack_choices_for};
use super::wizard_types::{ConfirmChoice, MultiInputMode, WizardStep};

/// Pure, deterministic state machine for the interactive `garde init` wizard.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WizardModel {
    pub(crate) root: PathBuf,
    pub(crate) project_name: Option<String>,
    pub(crate) run_git_init: bool,
    pub(crate) step: WizardStep,
    pub(crate) cursor: usize,
    pub(crate) selected_archetype: WorkspaceArchetype,
    pub(crate) stack_options: Vec<StackToken>,
    pub(crate) selected_stack: BTreeSet<usize>,
    pub(crate) selected_features: BTreeSet<usize>,
    pub(crate) input_mode: MultiInputMode,
    pub(crate) filter_query: String,
    pub(crate) confirm_choice: ConfirmChoice,
}

impl WizardModel {
    /// Creates a new [`WizardModel`] rooted at `root`.
    #[must_use]
    pub fn new(root: PathBuf, project_name: Option<String>, run_git_init: bool) -> Self {
        let default_arch = ARCHETYPE_CHOICES[0].archetype;
        Self {
            root,
            project_name,
            run_git_init,
            step: WizardStep::Archetype,
            cursor: 0,
            selected_archetype: default_arch,
            stack_options: stack_choices_for(default_arch),
            selected_stack: BTreeSet::new(),
            selected_features: BTreeSet::new(),
            input_mode: MultiInputMode::Selecting,
            filter_query: String::new(),
            confirm_choice: ConfirmChoice::Yes,
        }
    }

    /// Returns the current wizard step.
    #[must_use]
    pub const fn step(&self) -> WizardStep {
        self.step
    }

    /// Returns the current 0-indexed cursor position within the active visible list.
    #[must_use]
    pub const fn cursor(&self) -> usize {
        self.cursor
    }

    /// Returns the number of visible items in the current step.
    #[must_use]
    pub fn visible_len(&self) -> usize {
        match self.step {
            WizardStep::Archetype => ARCHETYPE_CHOICES.len(),
            WizardStep::Stack => self.visible_stack_indices().len(),
            WizardStep::Features => self.visible_feature_indices().len(),
            WizardStep::Confirm => 2,
        }
    }

    pub(crate) fn visible_stack_indices(&self) -> Vec<usize> {
        let q = self.filter_query.trim().to_ascii_lowercase();
        self.stack_options
            .iter()
            .enumerate()
            .filter(|(_, t)| {
                q.is_empty()
                    || t.slug().contains(&q)
                    || t.summary().to_ascii_lowercase().contains(&q)
            })
            .map(|(i, _)| i)
            .collect()
    }

    pub(crate) fn visible_feature_indices(&self) -> Vec<usize> {
        let q = self.filter_query.trim().to_ascii_lowercase();
        FEATURE_CHOICES
            .iter()
            .enumerate()
            .filter(|(_, (f, d))| {
                q.is_empty() || f.as_str().contains(&q) || d.to_ascii_lowercase().contains(&q)
            })
            .map(|(i, _)| i)
            .collect()
    }

    /// Builds the [`InitRequest`] corresponding to the user's current selections.
    #[must_use]
    pub fn build_request(&self) -> InitRequest {
        let mut languages = BTreeSet::new();
        let mut frameworks = BTreeSet::new();
        for &idx in &self.selected_stack {
            match self.stack_options.get(idx).copied() {
                Some(StackToken::Lang(l)) => {
                    languages.insert(l);
                }
                Some(StackToken::Framework(fw)) => {
                    frameworks.insert(fw);
                }
                None => {}
            }
        }
        let features = self
            .selected_features
            .iter()
            .filter_map(|&i| FEATURE_CHOICES.get(i).map(|(f, _)| *f))
            .collect();
        InitRequest {
            root: self.root.clone(),
            project_name: self.project_name.clone(),
            archetype: Some(self.selected_archetype),
            languages,
            features,
            frameworks,
            run_git_init: self.run_git_init,
        }
    }
}
