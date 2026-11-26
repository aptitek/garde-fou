//! Pure Gum-style 3-choice selector and confirmation wizard for interactive `garde uninstall`.

use super::keys::Key;
use crate::ui::{CardStyle, ColorPolicy, PillRole, Role, paint, paint_bold, pill, rounded_card};

/// Scope selected by the user in interactive `garde uninstall`.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum UninstallChoice {
    /// Remove Garde-fou from the current project (`--project`).
    #[default]
    Project = 0,
    /// Uninstall the `garde` CLI binary itself (`--self`).
    SelfBinary = 1,
    /// Remove both project configuration and CLI binary (`--project --self`).
    Both = 2,
}

impl UninstallChoice {
    /// All 3 selectable scopes in display order.
    pub const ALL: [Self; 3] = [Self::Project, Self::SelfBinary, Self::Both];

    /// Display label shown in the Gum radio selector.
    #[must_use]
    pub const fn label(self) -> &'static str {
        const LABELS: [&str; 3] = [
            "Désinstaller du projet uniquement (garde-fou.toml, garde.lock, blocs gérés)",
            "Désinstaller le binaire `garde` uniquement (mise / binaire courant)",
            "Désinstallation complète (Projet + Binaire `garde`)",
        ];
        LABELS[self as usize]
    }
}

/// Active step of [`UninstallWizardModel`].
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum UninstallWizardStep {
    /// Choosing scope (`Project`, `SelfBinary`, `Both`).
    SelectScope,
    /// Confirming destructive execution (`[Oui / Non]`).
    Confirm,
}

/// Transition returned by [`UninstallWizardModel::handle_key`].
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum UninstallWizardOutcome {
    /// Stay in wizard and redraw.
    Continue,
    /// User confirmed the selected [`UninstallChoice`].
    Confirmed(UninstallChoice),
    /// User cancelled (`Esc`, `q`, `Ctrl+C`, or `Non`).
    Cancelled,
}

/// Pure, deterministic state machine for interactive `garde uninstall`.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UninstallWizardModel {
    step: UninstallWizardStep,
    cursor: usize,
    confirm_yes: bool,
}

impl Default for UninstallWizardModel {
    fn default() -> Self {
        Self::new()
    }
}

impl UninstallWizardModel {
    /// Creates a new [`UninstallWizardModel`] starting on scope selection.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            step: UninstallWizardStep::SelectScope,
            cursor: 0,
            confirm_yes: false,
        }
    }

    /// Returns the currently highlighted [`UninstallChoice`].
    #[must_use]
    pub const fn selected_choice(&self) -> UninstallChoice {
        UninstallChoice::ALL[self.cursor]
    }

    /// Returns the active [`UninstallWizardStep`].
    #[must_use]
    pub const fn step(&self) -> UninstallWizardStep {
        self.step
    }

    /// Processes a single [`Key`] event and returns [`UninstallWizardOutcome`].
    pub fn handle_key(&mut self, key: Key) -> UninstallWizardOutcome {
        if matches!(key, Key::CtrlC | Key::Char('q')) {
            return UninstallWizardOutcome::Cancelled;
        }
        match self.step {
            UninstallWizardStep::SelectScope => self.handle_scope_key(key),
            UninstallWizardStep::Confirm => self.handle_confirm_key(key),
        }
    }

    fn handle_scope_key(&mut self, key: Key) -> UninstallWizardOutcome {
        let last = UninstallChoice::ALL.len() - 1;
        match key {
            Key::Esc => return UninstallWizardOutcome::Cancelled,
            Key::Up | Key::Char('k') => self.cursor = self.cursor.saturating_sub(1),
            Key::Down | Key::Char('j') => self.cursor = (self.cursor + 1).min(last),
            Key::Enter | Key::Char(' ') => {
                self.step = UninstallWizardStep::Confirm;
                self.confirm_yes = false;
            }
            _ => {}
        }
        UninstallWizardOutcome::Continue
    }

    const fn handle_confirm_key(&mut self, key: Key) -> UninstallWizardOutcome {
        match key {
            Key::Esc | Key::Backspace => {
                self.step = UninstallWizardStep::SelectScope;
                UninstallWizardOutcome::Continue
            }
            Key::Left | Key::Right | Key::Tab | Key::Char('h' | 'l' | ' ') => {
                self.confirm_yes = !self.confirm_yes;
                UninstallWizardOutcome::Continue
            }
            Key::Char('o' | 'O' | 'y' | 'Y') => {
                UninstallWizardOutcome::Confirmed(self.selected_choice())
            }
            Key::Char('n' | 'N') => UninstallWizardOutcome::Cancelled,
            Key::Enter => {
                if self.confirm_yes {
                    UninstallWizardOutcome::Confirmed(self.selected_choice())
                } else {
                    UninstallWizardOutcome::Cancelled
                }
            }
            _ => UninstallWizardOutcome::Continue,
        }
    }
}

fn render_scope_lines(cursor: usize, policy: ColorPolicy) -> Vec<String> {
    let mut lines = vec![
        paint_bold("Que souhaitez-vous désinstaller ?", Role::Accent, policy),
        String::new(),
    ];
    for (i, choice) in UninstallChoice::ALL.iter().enumerate() {
        let focused = i == cursor;
        let ptr = if focused {
            paint_bold("▸", Role::Accent, policy)
        } else {
            String::from(" ")
        };
        let dot = if focused {
            paint_bold("◉", Role::Warning, policy)
        } else {
            paint("○", Role::Muted, policy)
        };
        let txt = if focused {
            paint_bold(choice.label(), Role::Accent, policy)
        } else {
            String::from(choice.label())
        };
        lines.push(format!("{ptr} {dot} {txt}"));
    }
    lines
}

fn render_confirm_lines(model: &UninstallWizardModel, policy: ColorPolicy) -> Vec<String> {
    let yes_role = if model.confirm_yes {
        PillRole::BadgeError
    } else {
        PillRole::BadgeMuted
    };
    let no_role = if model.confirm_yes {
        PillRole::BadgeMuted
    } else {
        PillRole::BadgeAccent
    };
    let yes = pill("Oui, désinstaller", yes_role, policy);
    let no = pill("Non, annuler", no_role, policy);
    vec![
        paint_bold(
            "Confirmer la désinstallation chirurgicale :",
            Role::Warning,
            policy,
        ),
        format!("  • {}", model.selected_choice().label()),
        String::new(),
        format!("  {yes}   {no}"),
    ]
}

/// Renders [`UninstallWizardModel`] as a Lipgloss Gum card.
#[must_use]
pub fn render_uninstall_wizard(
    model: &UninstallWizardModel,
    policy: ColorPolicy,
    max_width: usize,
) -> String {
    let lines = match model.step {
        UninstallWizardStep::SelectScope => render_scope_lines(model.cursor, policy),
        UninstallWizardStep::Confirm => render_confirm_lines(model, policy),
    };
    let card = rounded_card(
        Some("garde uninstall — assistant interactif"),
        &lines,
        CardStyle::new(Role::Warning, policy),
        max_width,
    );
    let help = paint(
        "  [↑/↓ j/k] choisir · [←/→] Oui/Non · [Entrée] valider · [Échap/q] annuler",
        Role::Muted,
        policy,
    );
    format!("{card}\n{help}")
}
