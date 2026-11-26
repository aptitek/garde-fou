//! Leaf types (`WizardTriggerEnv`, `WizardStep`, `WizardOutcome`, `WizardTransition`)
//! and strict trigger rule (`should_launch_init_wizard`) for the `garde init` wizard.

use gf_engine::InitRequest;

use crate::args::InitArgs;

/// Environment signals determining whether `garde init` launches interactively.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct WizardTriggerEnv {
    /// `true` only when BOTH `stdin` and `stdout` are interactive TTYs.
    pub is_interactive_tty: bool,
    /// `true` when `CI` is set to a truthy non-zero value.
    pub is_ci: bool,
}

/// Evaluates the strict `garde init` wizard trigger rule (zero breakage of scripts/tests).
#[must_use]
pub const fn should_launch_init_wizard(args: &InitArgs, env: WizardTriggerEnv) -> bool {
    if args.mode.yes {
        return false;
    }
    if args.mode.interactive {
        return true;
    }
    let has_cfg = args.archetype.is_some()
        || !args.langs.is_empty()
        || !args.features.is_empty()
        || !args.frameworks.is_empty()
        || args.offline
        || args.no_git;
    !has_cfg && env.is_interactive_tty && !env.is_ci
}

/// Active step of the 4-screen `garde init` wizard.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WizardStep {
    /// Step 1: single-choice workspace archetype (`Choose`).
    Archetype,
    /// Step 2: archetype-tailored languages & frameworks (`MultiSelect`).
    Stack,
    /// Step 3: infrastructure & 6-domain guardrails (`MultiSelect`).
    Features,
    /// Step 4: file-tree preview and `Oui / Non` confirmation (`Confirm`).
    Confirm,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum MultiInputMode {
    Selecting,
    Filtering,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ConfirmChoice {
    Yes,
    No,
}

/// Final outcome produced when the `garde init` wizard completes or aborts.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum WizardOutcome {
    /// User confirmed initialization with the assembled [`InitRequest`].
    Confirmed(InitRequest),
    /// User cancelled (`Esc`, `Ctrl+C`, or `Non`) — zero disk writes must occur.
    Cancelled,
}

/// Step transition returned by `WizardModel::handle_key`.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum WizardTransition {
    /// Keep rendering the updated wizard model.
    Continue,
    /// Exit the wizard with `WizardOutcome`.
    Done(WizardOutcome),
}
