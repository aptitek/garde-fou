//! Geometric Lipgloss alignment tests, Miette code-frame assertions, `NO_COLOR` /
//! `agent` / `sarif` non-regression proofs, and headless keyboard simulations.

use std::collections::BTreeMap;
use std::path::PathBuf;

use gf_cli::args::OutputFormat;
use gf_cli::reporter::{render_agent_markdown, render_check_report_rich};
use gf_cli::tui::{
    ExplorerAction, ExplorerMode, ExplorerModel, ExplorerSnapshot, Key, WizardModel, WizardOutcome,
    WizardStep, WizardTransition,
};
use gf_cli::ui::{
    CardStyle, CodeFrameSpec, ColorPolicy, HumanCheckMeta, PillRole, Role, SourceFileView,
    encode_base64, paint, pill, render_code_frame, rounded_card, visible_width,
};
use gf_core::{
    Diagnostic, FeatureId, FrameworkId, LanguageId, ProjectContext, RepoPath, Severity, Span,
    WorkspaceArchetype,
};
use gf_engine::{CheckReport, ScanScope};

type TestRes = Result<(), Box<dyn std::error::Error>>;

fn sample_diag(rule: &str, path: &str, line: u32) -> Result<Diagnostic, gf_core::RepoPathError> {
    Ok(Diagnostic {
        rule_id: String::from(rule),
        severity: Severity::Error,
        message: String::from("Ségrégation d'étanchéité échouée à la ligne 12"),
        remediation: String::from("Extraire la primitive dans styles/tokens.css."),
        measured: 14,
        threshold: 10,
        path: RepoPath::new(path)?,
        span: Span::new(line, line).unwrap_or(Span::FIRST),
    })
}

#[test]
fn lipgloss_rounded_card_aligns_truecolor_french_accents_and_symbols() {
    let policy = ColorPolicy::TrueColor;
    let lines = vec![
        format!(
            "{} {} Ségrégation d'étanchéité échouée à la ligne 12",
            pill("✗ GF061", PillRole::BadgeError, policy),
            pill("FORME", PillRole::BadgeDomain, policy),
        ),
        format!(
            "{} ⚡ cache 136/136 · ✓ prêt · ▸ src/app.tsx:12:5",
            paint("Statut :", Role::Accent, policy),
        ),
        String::from("Chaîne très longue avec accents éèàçêîôû tronquée proprement sans déborder"),
    ];
    let card = rounded_card(
        Some("Rapport d'intégrité — Garde-fou"),
        &lines,
        CardStyle::new(Role::Error, policy),
        68,
    );
    let widths: Vec<usize> = card.lines().map(visible_width).collect();
    assert!(widths.len() >= 5 && widths[0] <= 68);
    for w in &widths {
        assert_eq!(
            *w, widths[0],
            "mismatch in card line widths: {widths:?}\n{card}"
        );
    }
}

#[test]
fn code_frame_renders_numbered_gutter_exact_caret_and_fallbacks() {
    let src = SourceFileView::Text(String::from(
        "fn ok() {}\n    let bad = val.unwrap();\nfn end() {}\n",
    ));
    let spec = CodeFrameSpec {
        path: "src/demo.rs",
        start_line: 2,
        end_line: 3,
        source: &src,
    };
    let joined = render_code_frame(&spec, ColorPolicy::Plain, 80).join("\n");
    assert!(joined.contains("╭─▸ src/demo.rs:2:5"), "{joined}");
    assert!(joined.contains(" 1 │ fn ok() {}"), "{joined}");
    assert!(
        joined.contains(" 2 │     let bad = val.unwrap();"),
        "{joined}"
    );
    assert!(joined.contains("╰── ^^^^^^^^^^^^^^^^^^^^^^^"), "{joined}");
    assert!(joined.contains("portée lignes 2..=3"), "{joined}");

    let bin = CodeFrameSpec {
        path: "a/x.png",
        start_line: 1,
        end_line: 1,
        source: &SourceFileView::BinaryOrOmitted,
    };
    assert!(
        render_code_frame(&bin, ColorPolicy::Plain, 80)
            .join("\n")
            .contains("fichier binaire ou asset")
    );

    let oob = CodeFrameSpec {
        path: "src/demo.rs",
        start_line: 50,
        end_line: 50,
        source: &src,
    };
    assert!(
        render_code_frame(&oob, ColorPolicy::Plain, 80)
            .join("\n")
            .contains("ligne 50 hors bornes")
    );
}

#[test]
fn plain_policy_and_automated_formats_emit_zero_ansi_bytes() -> TestRes {
    let report = CheckReport {
        scope: ScanScope::All,
        context: ProjectContext::default(),
        files_checked: 8,
        cache_hits: 7,
        cache_misses: 1,
        cache_fingerprint: String::from("0123456789abcdef"),
        diagnostics: vec![sample_diag("GF061", "styles/app.css", 12)?],
    };
    let mut sources = BTreeMap::from([(
        String::from("styles/app.css"),
        SourceFileView::Text(String::from(".btn { color: #ff00aa; }\n")),
    )]);
    let plain_meta = HumanCheckMeta {
        elapsed_ms: 9,
        lcov_tenths: Some(945),
        policy: ColorPolicy::Plain,
        max_width: 90,
    };
    let human_plain = render_check_report_rich(&report, OutputFormat::Human, &sources, &plain_meta);
    assert!(!human_plain.as_bytes().contains(&0x1B) && human_plain.contains("[GF061]"));

    let color_meta = HumanCheckMeta {
        policy: ColorPolicy::TrueColor,
        ..plain_meta
    };
    let agent_out = render_check_report_rich(&report, OutputFormat::Agent, &sources, &color_meta);
    assert!(!agent_out.as_bytes().contains(&0x1B));
    assert_eq!(agent_out, render_agent_markdown(&report));

    sources.clear();
    let sarif_out = render_check_report_rich(&report, OutputFormat::Sarif, &sources, &color_meta);
    assert!(!sarif_out.as_bytes().contains(&0x1B));
    let parsed: serde_json::Value = serde_json::from_str(&sarif_out)?;
    assert_eq!(parsed["version"], "2.1.0");
    assert_eq!(parsed["runs"][0]["results"][0]["ruleId"], "GF061");
    Ok(())
}

#[test]
fn wizard_model_simulates_keyboard_flow_and_escape_cancellation() -> TestRes {
    let mut wiz = WizardModel::new(PathBuf::from("/tmp/proj"), Some(String::from("proj")), true);
    assert_eq!(wiz.step(), WizardStep::Archetype);
    let seq = [
        Key::Down,
        Key::Down,
        Key::Enter,
        Key::Char(' '),
        Key::Down,
        Key::Char(' '),
        Key::Enter,
        Key::Char(' '),
        Key::Enter,
    ];
    for key in seq {
        assert_eq!(wiz.handle_key(key), WizardTransition::Continue);
    }
    assert_eq!(wiz.step(), WizardStep::Confirm);
    let WizardTransition::Done(WizardOutcome::Confirmed(req)) = wiz.handle_key(Key::Enter) else {
        return Err("expected Confirmed InitRequest".into());
    };
    assert_eq!(req.archetype, Some(WorkspaceArchetype::Infra));
    assert!(req.languages.contains(&LanguageId::Yaml));
    assert!(req.frameworks.contains(&FrameworkId::DockerCompose));
    assert!(req.features.contains(&FeatureId::Docker));

    let mut cancelled = WizardModel::new(PathBuf::from("/tmp/p2"), None, false);
    assert_eq!(
        cancelled.handle_key(Key::Esc),
        WizardTransition::Done(WizardOutcome::Cancelled)
    );
    Ok(())
}

#[test]
fn explorer_model_filters_selects_and_emits_osc52_base64_payload() -> TestRes {
    let report = CheckReport {
        scope: ScanScope::All,
        context: ProjectContext::default(),
        files_checked: 3,
        cache_hits: 3,
        cache_misses: 0,
        cache_fingerprint: String::from("deadbeef"),
        diagnostics: vec![
            sample_diag("GF010", "src/lib.rs", 4)?,
            sample_diag("GF060", "src/ui.tsx", 9)?,
            sample_diag("GF061", "styles/app.css", 12)?,
        ],
    };
    let agent_md = render_agent_markdown(&report);
    let mut exp = ExplorerModel::new(ExplorerSnapshot {
        report,
        sources: BTreeMap::new(),
        elapsed_ms: 4,
        agent_markdown: agent_md,
    });

    for k in [
        Key::Char('/'),
        Key::Char('G'),
        Key::Char('F'),
        Key::Char('0'),
        Key::Char('6'),
        Key::Enter,
    ] {
        assert_eq!(exp.handle_key(k), ExplorerAction::Redraw);
    }
    assert_eq!(
        (exp.mode(), exp.filter_query(), exp.filtered_indices()),
        (ExplorerMode::Normal, "GF06", vec![1, 2])
    );
    assert_eq!(exp.handle_key(Key::Down), ExplorerAction::Redraw);
    assert_eq!(
        exp.selected_diagnostic().map(|d| d.rule_id.as_str()),
        Some("GF061")
    );

    let ExplorerAction::CopyOsc52(osc) = exp.handle_key(Key::Char('y')) else {
        return Err("expected CopyOsc52".into());
    };
    let expected_b64 = encode_base64(
        b"- **[GF061]** `styles/app.css:12-12` (`measured: 14`, `max: 10`) \xe2\x80\x94 S\xc3\xa9gr\xc3\xa9gation d'\xc3\xa9tanch\xc3\xa9it\xc3\xa9 \xc3\xa9chou\xc3\xa9e \xc3\xa0 la ligne 12 | **Remediation**: Extraire la primitive dans styles/tokens.css.",
    );
    assert_eq!(osc, format!("\x1b]52;c;{expected_b64}\x07"));
    assert_eq!(exp.handle_key(Key::Char('q')), ExplorerAction::Quit);
    Ok(())
}
