//! Boundary tests for `ui::policy`, `ui::ansi`, `ui::theme`, `ui::box_layout`, and `ui::ops_report`.

use std::collections::BTreeMap;
use std::path::PathBuf;

use gf_cli::args::ColorChoice;
use gf_cli::ui::{
    ColorPolicy, HumanCheckMeta, Role, TermCapabilities, TreeSymbol, char_display_width,
    encode_base64, gauge_bar, join_horizontal_top, kv_row, paint, paint_bold,
    render_commit_msg_human, render_human_check, render_init_human, render_lock_human,
    render_sync_human, resolve_color_policy, spinner_frame, status_tree, strip_ansi,
    truncate_visible, visible_width,
};
use gf_core::{Diagnostic, ProjectContext, RepoPath, Severity, Span, Validation};
use gf_engine::{CheckReport, FileSyncAction, InitReport, ScanScope, SyncReport};

fn base_tty_caps() -> TermCapabilities {
    TermCapabilities {
        is_tty: true,
        is_ci: false,
        no_color: None,
        term: Some(String::from("xterm-256color")),
        clicolor_force: None,
        colorterm: None,
    }
}

#[test]
fn color_policy_handles_never_always_and_truecolor_variants() {
    let tty_caps = base_tty_caps();
    assert_eq!(
        resolve_color_policy(ColorChoice::Never, &tty_caps),
        ColorPolicy::Plain
    );
    assert_eq!(
        resolve_color_policy(ColorChoice::Auto, &tty_caps),
        ColorPolicy::Ansi16
    );

    let tc_caps = TermCapabilities {
        colorterm: Some(String::from("truecolor")),
        ..tty_caps.clone()
    };
    assert_eq!(
        resolve_color_policy(ColorChoice::Always, &tc_caps),
        ColorPolicy::TrueColor
    );

    let b24_caps = TermCapabilities {
        colorterm: Some(String::from("24bit")),
        ..tty_caps
    };
    assert_eq!(
        resolve_color_policy(ColorChoice::Auto, &b24_caps),
        ColorPolicy::TrueColor
    );
}

#[test]
fn color_policy_handles_no_color_and_clicolor_force() {
    let c = base_tty_caps();
    let no_col = TermCapabilities {
        no_color: Some(String::from("1")),
        clicolor_force: Some(String::from("1")),
        ..c.clone()
    };
    assert_eq!(
        resolve_color_policy(ColorChoice::Auto, &no_col),
        ColorPolicy::Plain
    );
    let empty_nc = TermCapabilities {
        no_color: Some(String::new()),
        ..c.clone()
    };
    assert_eq!(
        resolve_color_policy(ColorChoice::Auto, &empty_nc),
        ColorPolicy::Ansi16
    );
    let forced = TermCapabilities {
        is_tty: false,
        is_ci: true,
        clicolor_force: Some(String::from("1")),
        ..c.clone()
    };
    assert_eq!(
        resolve_color_policy(ColorChoice::Auto, &forced),
        ColorPolicy::Ansi16
    );
    let zero = TermCapabilities {
        is_tty: false,
        clicolor_force: Some(String::from("0")),
        ..c
    };
    assert_eq!(
        resolve_color_policy(ColorChoice::Auto, &zero),
        ColorPolicy::Plain
    );
}

#[test]
fn color_policy_disables_on_dumb_term_and_ci() {
    let c = base_tty_caps();
    assert_eq!(
        resolve_color_policy(
            ColorChoice::Auto,
            &TermCapabilities {
                term: Some(String::from("dumb")),
                ..c.clone()
            }
        ),
        ColorPolicy::Plain
    );
    assert_eq!(
        resolve_color_policy(ColorChoice::Auto, &TermCapabilities { is_ci: true, ..c }),
        ColorPolicy::Plain
    );
}

#[test]
fn ansi_widths_truncation_and_base64_cover_edge_boundaries() {
    assert_eq!(char_display_width('\x00'), 0);
    assert_eq!(char_display_width('\x1f'), 0);
    assert_eq!(char_display_width(' '), 1);
    assert_eq!(char_display_width('\x7f'), 0);
    assert_eq!(char_display_width('\u{009f}'), 0);
    assert_eq!(char_display_width('\u{00a0}'), 1);
    assert_eq!(char_display_width('\u{0301}'), 0);
    assert_eq!(char_display_width('⚡'), 2);
    assert_eq!(char_display_width('✓'), 1);

    assert_eq!(
        strip_ansi("\x1b]8;;https://x\x1b\\lien\x1b]8;;\x1b\\"),
        "lien"
    );
    assert_eq!(strip_ansi("\x1b]52;c;YWJj\x07ok\x1bM\x1b"), "ok");
    assert_eq!(truncate_visible("abcdef", 0), "");
    assert_eq!(truncate_visible("abcd", 4), "abcd");
    assert_eq!(truncate_visible("abcde", 4), "abc…");
    let trunc_ansi = truncate_visible(&paint("abcdef", Role::Error, ColorPolicy::Ansi16), 4);
    assert!(trunc_ansi.ends_with("…\x1b[0m") && visible_width(&trunc_ansi) == 4);
    assert_eq!(encode_base64(b""), "");
    assert_eq!(encode_base64(b"f"), "Zg==");
    assert_eq!(encode_base64(b"fo"), "Zm8=");
    assert_eq!(encode_base64(b"foo"), "Zm9v");
}

#[test]
fn box_layout_primitives_render_gauges_trees_and_columns() {
    assert_eq!(paint("", Role::Accent, ColorPolicy::TrueColor), "");
    assert_eq!(paint_bold("", Role::Accent, ColorPolicy::TrueColor), "");
    assert_eq!(
        join_horizontal_top("AA\nBBBB", "1\n2\n3", 2),
        "AA    1\nBBBB  2\n      3"
    );
    assert_eq!(
        kv_row("blake3", "abcd", 8, ColorPolicy::Plain),
        "blake3    abcd"
    );
    assert!(gauge_bar(950, 10, ColorPolicy::Ansi16).contains("95.0%"));
    assert!(gauge_bar(750, 10, ColorPolicy::Ansi16).contains("75.0%"));
    assert!(gauge_bar(500, 2, ColorPolicy::Plain).contains("50.0%"));
    assert!(spinner_frame(0, "sync", ColorPolicy::Plain).contains("⠋ sync"));
    assert!(spinner_frame(11, "sync", ColorPolicy::Ansi16).contains("⠙"));

    let entries = [
        (TreeSymbol::Created, String::from("a")),
        (TreeSymbol::Updated, String::from("b")),
        (TreeSymbol::Intact, String::from("c")),
        (TreeSymbol::Sealed, String::from("d")),
        (TreeSymbol::Warn, String::from("e")),
    ];
    let tree = status_tree("hdr", &entries, ColorPolicy::Plain);
    assert!(tree.contains("├─ + a") && tree.contains("╰─ ! e"));
}

#[test]
fn ops_reports_render_init_and_sync_trees() {
    let clean_rep = CheckReport {
        scope: ScanScope::All,
        context: ProjectContext::default(),
        files_checked: 2,
        cache_hits: 2,
        cache_misses: 0,
        cache_fingerprint: String::from("aa"),
        diagnostics: Vec::new(),
    };
    assert!(
        render_human_check(&clean_rep, &BTreeMap::new(), &HumanCheckMeta::default())
            .contains("[✓ GARDE]")
    );
    let init_r = InitReport {
        root: PathBuf::from("/tmp/i"),
        project_name: String::from("i"),
        context: ProjectContext::default(),
        git_initialized: true,
        created_files: vec![String::from("garde.lock"), String::from("garde-fou.toml")],
        updated_files: vec![String::from(".gitignore")],
        preserved_files: vec![String::from("README.md")],
        installed_skills: vec![String::from("garde-fou")],
        warnings: vec![String::from("offline")],
    };
    assert!(render_init_human(&init_r, ColorPolicy::Plain).contains("garde.lock scellé"));
    let sync_r = SyncReport {
        context: ProjectContext::default(),
        agents_md: FileSyncAction::Updated,
        installed_skills: vec![String::from("garde-fou")],
        preserved_skills: vec![String::from("rust-review")],
        rejected_skills: vec![(String::from("bad"), String::from("audit"))],
        warnings: vec![String::from("net")],
        lock_updated: true,
    };
    assert!(render_sync_human(&sync_r, ColorPolicy::Plain).contains("garde: sync ok"));
}

#[test]
fn ops_reports_render_lock_and_commit_msg() -> Result<(), Box<dyn std::error::Error>> {
    assert!(
        render_lock_human("0123456789abcdef9999", ColorPolicy::Plain).contains("0123456789abcdef")
    );
    assert!(
        render_commit_msg_human(&Validation::Valid(()), ColorPolicy::Plain, 80)
            .contains("commit-msg ok")
    );
    let bad_c = Diagnostic {
        rule_id: String::from("GF077"),
        severity: Severity::Error,
        message: String::from("bad header"),
        remediation: String::from("use feat(scope): msg"),
        measured: 1,
        threshold: 0,
        path: RepoPath::new("COMMIT_EDITMSG")?,
        span: Span::FIRST,
    };
    assert!(
        render_commit_msg_human(&Validation::Invalid(vec![bad_c]), ColorPolicy::Plain, 80)
            .contains("GF077")
    );
    Ok(())
}
