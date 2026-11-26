//! Pure Gum & Bubble Tea TUI state machines (`WizardModel`, `ExplorerModel`, `ConfigTabModel`,
//! `CacheTabModel`, `AppModel`, `UninstallWizardModel`) and the isolated `terminal` TTY driver.

pub mod app_model;
pub mod app_types;
pub mod app_view;
pub mod cache_tab;
pub mod config_tab;
pub mod config_tab_catalog;
pub mod config_tab_view;
pub mod explorer_model;
pub mod explorer_types;
pub mod explorer_view;
pub mod keys;
pub mod terminal;
pub mod uninstall_wizard;
pub mod wizard_data;
pub mod wizard_model;
pub mod wizard_preview;
pub mod wizard_state;
pub mod wizard_types;
pub mod wizard_view;

pub use self::{
    app_model::AppModel,
    app_types::{AppEffect, AppEvent, AppInit, AppTab, WatchState},
    app_view::render_app,
    cache_tab::{CacheTabAction, CacheTabModel, render_cache_tab},
    config_tab::{ConfigTabAction, ConfigTabModel},
    config_tab_catalog::{
        ALL_ARCHETYPE_SETTINGS, ALL_FEATURES, ALL_FRAMEWORKS, ALL_LANGUAGES, ConfigCategory,
    },
    config_tab_view::render_config_tab,
    explorer_model::ExplorerModel,
    explorer_types::{ExplorerAction, ExplorerMode, ExplorerSnapshot},
    explorer_view::render_explorer,
    keys::{Key, parse_key_bytes},
    terminal::{detect_term_width, run_wizard_interactive},
    uninstall_wizard::{
        UninstallChoice, UninstallWizardModel, UninstallWizardOutcome, UninstallWizardStep,
        render_uninstall_wizard,
    },
    wizard_data::{ARCHETYPE_CHOICES, FEATURE_CHOICES, StackToken, stack_choices_for},
    wizard_preview::preview_init_files,
    wizard_state::WizardModel,
    wizard_types::{
        WizardOutcome, WizardStep, WizardTransition, WizardTriggerEnv, should_launch_init_wizard,
    },
    wizard_view::render_wizard,
};
