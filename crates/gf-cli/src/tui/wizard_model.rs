//! Pure keyboard transition handlers (`WizardModel::handle_key`) for the `garde init` wizard.

use std::collections::BTreeSet;

use gf_core::WorkspaceArchetype;

use super::keys::Key;
use super::wizard_data::{ARCHETYPE_CHOICES, stack_choices_for};
use super::wizard_state::WizardModel;
use super::wizard_types::{
    ConfirmChoice, MultiInputMode, WizardOutcome, WizardStep, WizardTransition,
};

impl WizardModel {
    /// Processes a single [`Key`] event and returns the resulting [`WizardTransition`].
    pub fn handle_key(&mut self, key: Key) -> WizardTransition {
        if key == Key::CtrlC {
            return WizardTransition::Done(WizardOutcome::Cancelled);
        }
        match self.step {
            WizardStep::Archetype => step_archetype(self, key),
            WizardStep::Stack | WizardStep::Features => step_multiselect(self, key),
            WizardStep::Confirm => step_confirm(self, key),
        }
    }
}

fn move_cursor(cursor: &mut usize, len: usize, key: Key) -> bool {
    if len == 0 {
        *cursor = 0;
        return matches!(key, Key::Up | Key::Down | Key::Home | Key::End);
    }
    match key {
        Key::Up | Key::Char('k') => *cursor = cursor.saturating_sub(1),
        Key::Down | Key::Char('j') => *cursor = (*cursor + 1).min(len - 1),
        Key::Home | Key::Char('g') => *cursor = 0,
        Key::End | Key::Char('G') => *cursor = len - 1,
        _ => return false,
    }
    true
}

fn step_archetype(m: &mut WizardModel, key: Key) -> WizardTransition {
    if key == Key::Esc {
        return WizardTransition::Done(WizardOutcome::Cancelled);
    }
    if !move_cursor(&mut m.cursor, ARCHETYPE_CHOICES.len(), key) && key == Key::Enter {
        let arch = ARCHETYPE_CHOICES
            .get(m.cursor)
            .map_or(WorkspaceArchetype::Minimal, |(a, _, _)| *a);
        m.selected_archetype = arch;
        m.stack_options = stack_choices_for(arch);
        m.selected_stack.clear();
        m.cursor = 0;
        m.filter_query.clear();
        m.input_mode = MultiInputMode::Selecting;
        m.step = WizardStep::Stack;
    }
    WizardTransition::Continue
}

fn step_multiselect(m: &mut WizardModel, key: Key) -> WizardTransition {
    if m.input_mode == MultiInputMode::Filtering {
        return step_filter(m, key);
    }
    let vis = if m.step == WizardStep::Stack {
        m.visible_stack_indices()
    } else {
        m.visible_feature_indices()
    };
    if key == Key::Esc {
        return WizardTransition::Done(WizardOutcome::Cancelled);
    }
    if !move_cursor(&mut m.cursor, vis.len(), key) {
        dispatch_multiselect_shortcut(m, &vis, key);
    }
    WizardTransition::Continue
}

fn dispatch_multiselect_shortcut(m: &mut WizardModel, vis: &[usize], key: Key) {
    match key {
        Key::Char('/') => m.input_mode = MultiInputMode::Filtering,
        Key::Char(' ' | 'x') | Key::Tab => {
            let picked = vis.get(m.cursor).copied();
            toggle_focused(active_set(m), picked);
        }
        Key::Char('a' | 'A') | Key::CtrlA => toggle_all(active_set(m), vis),
        Key::Enter => {
            m.cursor = 0;
            m.filter_query.clear();
            m.step = if m.step == WizardStep::Stack {
                WizardStep::Features
            } else {
                WizardStep::Confirm
            };
        }
        _ => {}
    }
}

fn toggle_focused(set: &mut BTreeSet<usize>, opt_idx: Option<usize>) {
    if let Some(orig) = opt_idx
        && !set.remove(&orig)
    {
        set.insert(orig);
    }
}

fn toggle_all(set: &mut BTreeSet<usize>, vis: &[usize]) {
    if !vis.is_empty() && vis.iter().all(|i| set.contains(i)) {
        for i in vis {
            set.remove(i);
        }
    } else {
        for &i in vis {
            set.insert(i);
        }
    }
}

fn active_set(m: &mut WizardModel) -> &mut BTreeSet<usize> {
    if m.step == WizardStep::Stack {
        &mut m.selected_stack
    } else {
        &mut m.selected_features
    }
}

fn step_filter(m: &mut WizardModel, key: Key) -> WizardTransition {
    match key {
        Key::Esc => {
            m.filter_query.clear();
            m.input_mode = MultiInputMode::Selecting;
        }
        Key::Enter => m.input_mode = MultiInputMode::Selecting,
        Key::Backspace => {
            m.filter_query.pop();
        }
        Key::Char(ch) => m.filter_query.push(ch),
        _ => {}
    }
    let len = m.visible_len();
    m.cursor = if len == 0 { 0 } else { m.cursor.min(len - 1) };
    WizardTransition::Continue
}

fn step_confirm(m: &mut WizardModel, key: Key) -> WizardTransition {
    match key {
        Key::Esc | Key::Char('n' | 'N') => WizardTransition::Done(WizardOutcome::Cancelled),
        Key::Char('y' | 'Y') => WizardTransition::Done(WizardOutcome::Confirmed(m.build_request())),
        Key::Left | Key::Right | Key::Char('h' | 'l') | Key::Tab | Key::BackTab => {
            m.confirm_choice = if m.confirm_choice == ConfirmChoice::Yes {
                ConfirmChoice::No
            } else {
                ConfirmChoice::Yes
            };
            m.cursor = usize::from(m.confirm_choice == ConfirmChoice::No);
            WizardTransition::Continue
        }
        Key::Enter => match m.confirm_choice {
            ConfirmChoice::Yes => {
                WizardTransition::Done(WizardOutcome::Confirmed(m.build_request()))
            }
            ConfirmChoice::No => WizardTransition::Done(WizardOutcome::Cancelled),
        },
        _ => WizardTransition::Continue,
    }
}
