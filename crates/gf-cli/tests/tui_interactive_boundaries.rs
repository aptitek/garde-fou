//! Boundary tests for `tui::keys`, `tui::wizard_*`, and `tui::explorer_*`.

use std::collections::BTreeMap;
use std::path::PathBuf;

use gf_cli::args::{InitArgs, InitModeFlags};
use gf_cli::tui::{
    ExplorerAction, ExplorerMode, ExplorerModel, ExplorerSnapshot, Key, WizardModel, WizardOutcome,
    WizardTransition, WizardTriggerEnv, parse_key_bytes, preview_init_files, render_explorer,
    render_wizard, should_launch_init_wizard, stack_choices_for,
};
use gf_cli::ui::ColorPolicy;
use gf_core::{FeatureId, LanguageId, ProjectContext, WorkspaceArchetype};
use gf_engine::{CheckReport, InitRequest, ScanScope};

#[test]
fn key_parser_recognizes_all_csi_ss3_control_and_utf8_sequences() {
    assert_eq!(parse_key_bytes(b""), None);
    assert_eq!(parse_key_bytes(b"\x02"), None);
    assert_eq!(parse_key_bytes(b"\x03"), Some((Key::CtrlC, 1)));
    assert_eq!(parse_key_bytes(b"\x01"), Some((Key::CtrlA, 1)));
    assert_eq!(parse_key_bytes(b"\x0e"), Some((Key::Down, 1)));
    assert_eq!(parse_key_bytes(b"\x10"), Some((Key::Up, 1)));
    assert_eq!(parse_key_bytes(b"\r"), Some((Key::Enter, 1)));
    assert_eq!(parse_key_bytes(b"\t"), Some((Key::Tab, 1)));
    assert_eq!(parse_key_bytes(b"\x7f"), Some((Key::Backspace, 1)));
    assert_eq!(parse_key_bytes(b"\x1b[A"), Some((Key::Up, 3)));
    assert_eq!(parse_key_bytes(b"\x1bOB"), Some((Key::Down, 3)));
    assert_eq!(parse_key_bytes(b"\x1b[C"), Some((Key::Right, 3)));
    assert_eq!(parse_key_bytes(b"\x1b[D"), Some((Key::Left, 3)));
    assert_eq!(parse_key_bytes(b"\x1b[H"), Some((Key::Home, 3)));
    assert_eq!(parse_key_bytes(b"\x1b[F"), Some((Key::End, 3)));
    assert_eq!(parse_key_bytes(b"\x1b[Z"), Some((Key::BackTab, 3)));
    assert_eq!(parse_key_bytes(b"\x1b[1~"), Some((Key::Home, 4)));
    assert_eq!(parse_key_bytes(b"\x1b[4~"), Some((Key::End, 4)));
    assert_eq!(parse_key_bytes(b"\x1b[5~"), Some((Key::PageUp, 4)));
    assert_eq!(parse_key_bytes(b"\x1b[6~"), Some((Key::PageDown, 4)));
    assert_eq!(parse_key_bytes(b"\x1b"), Some((Key::Esc, 1)));
    assert_eq!(parse_key_bytes("é".as_bytes()), Some((Key::Char('é'), 2)));
}

#[test]
fn wizard_trigger_rule_blocks_non_tty_and_ci() {
    let base = InitArgs::default();
    let tty = WizardTriggerEnv {
        is_interactive_tty: true,
        is_ci: false,
    };
    assert!(should_launch_init_wizard(&base, tty));
    assert!(!should_launch_init_wizard(
        &base,
        WizardTriggerEnv {
            is_interactive_tty: false,
            is_ci: false
        }
    ));
    assert!(!should_launch_init_wizard(
        &base,
        WizardTriggerEnv {
            is_interactive_tty: true,
            is_ci: true
        }
    ));
}

#[test]
fn wizard_trigger_rule_respects_yes_interactive_and_explicit_flags() {
    let base = InitArgs::default();
    let tty = WizardTriggerEnv {
        is_interactive_tty: true,
        is_ci: false,
    };
    assert!(!should_launch_init_wizard(
        &InitArgs {
            offline: true,
            ..base.clone()
        },
        tty
    ));
    assert!(!should_launch_init_wizard(
        &InitArgs {
            no_git: true,
            ..base.clone()
        },
        tty
    ));
    let yes_mode = InitModeFlags {
        interactive: false,
        yes: true,
    };
    assert!(!should_launch_init_wizard(
        &InitArgs {
            mode: yes_mode,
            ..base.clone()
        },
        tty
    ));
    let force_i = InitModeFlags {
        interactive: true,
        yes: false,
    };
    assert!(should_launch_init_wizard(
        &InitArgs {
            offline: true,
            mode: force_i,
            ..base
        },
        tty
    ));
}

#[test]
fn wizard_archetype_catalogs_and_views_exercise_all_steps() {
    for arch in WorkspaceArchetype::ALL {
        let choices = stack_choices_for(arch);
        assert_ne!(choices.len(), 0);
        for tok in choices {
            assert_ne!(tok.slug(), "");
            assert_ne!(tok.summary(), "");
        }
        let mut req = InitRequest::for_path(PathBuf::from("/tmp/x"));
        req.archetype = Some(arch);
        req.languages.insert(LanguageId::Rust);
        req.features.insert(FeatureId::Ui);
        assert!(preview_init_files(&req).len() >= 6);
    }

    let mut wiz = WizardModel::new(PathBuf::from("/tmp/w"), None, true);
    assert!(render_wizard(&wiz, ColorPolicy::Plain, 80).contains("ARCHÉTYPE"));
    wiz.handle_key(Key::End);
    wiz.handle_key(Key::Home);
    wiz.handle_key(Key::Enter);
    wiz.handle_key(Key::Char('a'));
    assert_ne!(wiz.build_request().languages.len(), 0);
    wiz.handle_key(Key::Char('a'));
    assert_eq!(wiz.build_request().languages.len(), 0);
    wiz.handle_key(Key::Char('/'));
    wiz.handle_key(Key::Char('z'));
    assert!(render_wizard(&wiz, ColorPolicy::Plain, 80).contains("FILTRE"));
    wiz.handle_key(Key::Backspace);
    wiz.handle_key(Key::Esc);
    wiz.handle_key(Key::Enter);
    assert!(render_wizard(&wiz, ColorPolicy::Plain, 80).contains("FEATURES"));
    wiz.handle_key(Key::Enter);
    assert!(render_wizard(&wiz, ColorPolicy::Plain, 80).contains("Confirmation"));
    wiz.handle_key(Key::Right);
    assert_eq!(
        wiz.handle_key(Key::Enter),
        WizardTransition::Done(WizardOutcome::Cancelled)
    );
}

#[test]
fn explorer_clean_reload_and_agent_preview_modes_behave_cleanly() {
    let clean_rep = CheckReport {
        scope: ScanScope::All,
        context: ProjectContext::default(),
        files_checked: 2,
        cache_hits: 2,
        cache_misses: 0,
        cache_fingerprint: String::from("aa"),
        diagnostics: Vec::new(),
    };
    let mut exp = ExplorerModel::new(ExplorerSnapshot {
        report: clean_rep.clone(),
        sources: BTreeMap::new(),
        elapsed_ms: 2,
        agent_markdown: String::from("## Garde-fou Report"),
    });
    assert!(exp.is_clean());
    assert!(render_explorer(&exp, ColorPolicy::Plain, 90).contains("0 Violation"));
    assert_eq!(exp.handle_key(Key::Char('r')), ExplorerAction::Reload);
    exp.replace_snapshot(ExplorerSnapshot {
        report: clean_rep,
        sources: BTreeMap::new(),
        elapsed_ms: 3,
        agent_markdown: String::from("## Garde-fou Report"),
    });
    exp.handle_key(Key::Char('a'));
    assert_eq!(exp.mode(), ExplorerMode::AgentPreview);
    assert!(render_explorer(&exp, ColorPolicy::Plain, 90).contains("Aperçu Markdown Agent"));
    assert!(matches!(
        exp.handle_key(Key::Char('y')),
        ExplorerAction::CopyOsc52(_)
    ));
    exp.handle_key(Key::Esc);
    assert_eq!(exp.mode(), ExplorerMode::Normal);
    assert_eq!(exp.handle_key(Key::CtrlC), ExplorerAction::Quit);
}
