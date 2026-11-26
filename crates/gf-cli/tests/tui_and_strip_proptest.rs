//! Property-based tests (`proptest`) for managed-block algebraic inversion laws,
//! Lipgloss rectangular box invariants, and arbitrary key fuzzing of `AppModel` / `ConfigTabModel`.

use std::collections::BTreeMap;

use gf_cli::tui::{
    AppEffect, AppEvent, AppInit, AppModel, AppTab, ConfigCategory, ConfigTabModel,
    ExplorerSnapshot, Key, UninstallChoice, UninstallWizardModel, UninstallWizardOutcome,
    WatchState, render_app, render_config_tab, render_uninstall_wizard,
};
use gf_cli::ui::{ColorPolicy, visible_width};
use gf_core::{
    ArchetypeSetting, ProjectContext, ProjectSettingsState, WorkspaceArchetype, merge_agents_md,
    merge_managed_block, strip_hash_managed_block, strip_html_managed_block,
};
use gf_engine::{CacheCleanSummary, CacheReport, CheckReport, ScanScope, WatchDelta};
use proptest::prelude::*;

fn arb_key() -> impl Strategy<Value = Key> {
    prop_oneof![
        Just(Key::Up),
        Just(Key::Down),
        Just(Key::Left),
        Just(Key::Right),
        Just(Key::Home),
        Just(Key::End),
        Just(Key::Tab),
        Just(Key::BackTab),
        Just(Key::Enter),
        Just(Key::Esc),
        Just(Key::Backspace),
        Just(Key::Char(' ')),
        Just(Key::Char('x')),
        Just(Key::Char('s')),
        Just(Key::Char('u')),
        Just(Key::Char('w')),
        Just(Key::Char('p')),
        Just(Key::Char('1')),
        Just(Key::Char('2')),
        Just(Key::Char('3')),
        Just(Key::Char('/')),
        Just(Key::Char('a')),
        Just(Key::Char('r')),
        Just(Key::Char('y')),
    ]
}

fn arb_human_text() -> impl Strategy<Value = String> {
    "[a-zA-Z0-9 _./\\-éèà\n]{1,120}".prop_filter("no managed markers", |s| {
        !s.contains("garde-fou managed") && !s.trim().is_empty()
    })
}

fn arb_managed_body() -> impl Strategy<Value = String> {
    "[a-zA-Z0-9 _./\\-\n]{1,80}".prop_filter("non-empty body", |s| !s.trim().is_empty())
}

proptest! {
    #[test]
    fn html_managed_block_strip_inverts_merge(
        human in arb_human_text(),
        managed in arb_managed_body(),
    ) {
        let merged = merge_agents_md(Some(&human), &managed);
        let stripped = strip_html_managed_block(&merged);
        prop_assert!(!stripped.contains("garde-fou managed"));
        prop_assert_eq!(stripped.trim(), human.trim());
    }

    #[test]
    fn hash_managed_block_strip_inverts_merge(
        human in arb_human_text(),
        managed in arb_managed_body(),
    ) {
        let block = format!(
            "# >>> garde-fou managed >>>\n{}\n# <<< garde-fou managed <<<\n",
            managed.trim()
        );
        let merged = merge_managed_block(Some(&human), &block);
        let stripped = strip_hash_managed_block(&merged);
        prop_assert!(!stripped.contains("garde-fou managed"));
        prop_assert_eq!(stripped.trim(), human.trim());
    }

    #[test]
    fn app_model_and_config_tab_survive_arbitrary_key_sequences(
        keys in prop::collection::vec(arb_key(), 1..40),
        width in 48_usize..120_usize,
    ) {
        let snap = ExplorerSnapshot {
            report: CheckReport {
                scope: ScanScope::All,
                context: ProjectContext::default(),
                files_checked: 0,
                cache_hits: 0,
                cache_misses: 0,
                cache_fingerprint: String::from("00"),
                diagnostics: Vec::new(),
            },
            sources: BTreeMap::new(),
            elapsed_ms: 0,
            agent_markdown: String::new(),
        };
        let mut app = AppModel::new(AppInit {
            initial_tab: AppTab::Diagnostics,
            snapshot: snap,
            settings: ProjectSettingsState::default(),
            cache_report: CacheReport::default(),
        });
        let mut cfg = ConfigTabModel::new(ProjectSettingsState::default());
        let mut uw = UninstallWizardModel::new();

        for key in keys {
            app.update(AppEvent::Key(key));
            cfg.handle_key(key);
            uw.handle_key(key);
            prop_assert!(cfg.cursor() < cfg.category().len());
        }

        let app_view = render_app(&app, ColorPolicy::TrueColor, width);
        let cfg_view = render_config_tab(&cfg, ColorPolicy::Plain, width);
        let uw_view = render_uninstall_wizard(&uw, ColorPolicy::Plain, width);
        prop_assert!(!app_view.is_empty());
        prop_assert!(!cfg_view.is_empty());
        prop_assert!(!uw_view.is_empty());
        for line in cfg_view.lines() {
            if line.starts_with('╭') || line.starts_with('│') || line.starts_with('╰') {
                prop_assert!(visible_width(line) <= width);
            }
        }
    }
}

fn empty_snapshot() -> ExplorerSnapshot {
    ExplorerSnapshot {
        report: CheckReport {
            scope: ScanScope::All,
            context: ProjectContext::default(),
            files_checked: 1,
            cache_hits: 1,
            cache_misses: 0,
            cache_fingerprint: String::from("aa"),
            diagnostics: Vec::new(),
        },
        sources: BTreeMap::new(),
        elapsed_ms: 1,
        agent_markdown: String::from("# Report"),
    }
}

#[test]
fn app_model_three_tabs_and_watch_toggle_flow() {
    let settings = ProjectSettingsState {
        archetype: ArchetypeSetting::Explicit(WorkspaceArchetype::Software),
        ..ProjectSettingsState::default()
    };
    let mut app = AppModel::new(AppInit {
        initial_tab: AppTab::Diagnostics,
        snapshot: empty_snapshot(),
        settings,
        cache_report: CacheReport::default(),
    });
    let delta = WatchDelta {
        changed: vec![String::from("src/lib.rs")],
        removed: Vec::new(),
        config_changed: false,
    };
    assert_eq!(
        app.update(AppEvent::FileChanged(delta.clone())),
        AppEffect::ReloadCheck
    );
    app.update(AppEvent::Key(Key::Char('w')));
    assert_eq!(app.watch_state(), WatchState::Paused);
    assert_eq!(app.update(AppEvent::FileChanged(delta)), AppEffect::Redraw);
    app.update(AppEvent::Key(Key::Tab));
    assert_eq!(app.active_tab(), AppTab::Config);
    assert_eq!(app.config().category(), ConfigCategory::Archetype);
    app.update(AppEvent::Key(Key::Char('3')));
    assert_eq!(app.active_tab(), AppTab::Cache);
    assert_eq!(
        app.update(AppEvent::Key(Key::Char('p'))),
        AppEffect::PruneCache
    );
    assert_eq!(
        app.update(AppEvent::Key(Key::Char('x'))),
        AppEffect::CleanCache
    );
    app.apply_cache_op(
        "prune",
        CacheCleanSummary::default(),
        CacheReport::default(),
    );
    assert!(render_app(&app, ColorPolicy::Plain, 90).contains("Cache BLAKE3"));
}

#[test]
fn uninstall_wizard_selects_and_confirms_scope() {
    let mut uw = UninstallWizardModel::new();
    uw.handle_key(Key::Down);
    assert_eq!(uw.selected_choice(), UninstallChoice::SelfBinary);
    uw.handle_key(Key::Enter);
    assert!(render_uninstall_wizard(&uw, ColorPolicy::Plain, 80).contains("Confirmer"));
    uw.handle_key(Key::Right);
    assert_eq!(
        uw.handle_key(Key::Enter),
        UninstallWizardOutcome::Confirmed(UninstallChoice::SelfBinary)
    );
}
