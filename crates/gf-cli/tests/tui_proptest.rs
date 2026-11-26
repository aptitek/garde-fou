//! Property-based tests (`proptest`) verifying Lipgloss ANSI invariance,
//! rectangular card geometry (`max_width` in `20..=140`), and TUI key fuzzing bounds.

use std::collections::BTreeMap;
use std::path::PathBuf;

use gf_cli::tui::{ExplorerModel, ExplorerSnapshot, Key, WizardModel};
use gf_cli::ui::{
    CardStyle, ColorPolicy, PillRole, Role, paint, paint_bold, pill, rounded_card, strip_ansi,
    visible_width,
};
use gf_core::{Diagnostic, ProjectContext, RepoPath, Severity, Span};
use gf_engine::{CheckReport, ScanScope};
use proptest::prelude::*;

fn printable_text_strategy() -> impl Strategy<Value = String> {
    prop::collection::vec(
        prop_oneof![
            any::<char>().prop_filter("no ESC or newline", |c| {
                *c != '\x1b' && *c != '\n' && *c != '\r' && !c.is_control()
            }),
            Just('é'),
            Just('à'),
            Just('⚡'),
            Just('漢'),
            Just('✓'),
        ],
        0..48,
    )
    .prop_map(|chars| chars.into_iter().collect())
}

fn policy_strategy() -> impl Strategy<Value = ColorPolicy> {
    prop_oneof![
        Just(ColorPolicy::Plain),
        Just(ColorPolicy::Ansi16),
        Just(ColorPolicy::TrueColor),
    ]
}

fn role_strategy() -> impl Strategy<Value = Role> {
    prop_oneof![
        Just(Role::Accent),
        Just(Role::Success),
        Just(Role::Error),
        Just(Role::Warning),
        Just(Role::Muted),
        Just(Role::SurfaceBorder),
        Just(Role::CodeLine),
    ]
}

fn pill_role_strategy() -> impl Strategy<Value = PillRole> {
    prop_oneof![
        Just(PillRole::BadgeOk),
        Just(PillRole::BadgeError),
        Just(PillRole::BadgeWarn),
        Just(PillRole::BadgeAccent),
        Just(PillRole::BadgeDomain),
        Just(PillRole::BadgeMuted),
    ]
}

fn key_strategy() -> impl Strategy<Value = Key> {
    prop_oneof![
        Just(Key::Up),
        Just(Key::Down),
        Just(Key::Left),
        Just(Key::Right),
        Just(Key::PageUp),
        Just(Key::PageDown),
        Just(Key::Home),
        Just(Key::End),
        Just(Key::Enter),
        Just(Key::Esc),
        Just(Key::Tab),
        Just(Key::BackTab),
        Just(Key::Backspace),
        Just(Key::CtrlA),
        prop::sample::select(vec![
            ' ', 'x', 'a', '/', 'j', 'k', 'y', 'n', 'q', 'r', 'G', 'f'
        ])
        .prop_map(Key::Char),
    ]
}

fn make_explorer_with_count(count: usize) -> ExplorerModel {
    let mut diagnostics = Vec::with_capacity(count);
    for i in 0..count {
        if let Ok(path) = RepoPath::new(&format!("src/mod_{i}.rs")) {
            diagnostics.push(Diagnostic {
                rule_id: format!("GF0{:02}", i % 83),
                severity: Severity::Error,
                message: format!("violation {i}"),
                remediation: String::from("refactor"),
                measured: 12,
                threshold: 10,
                path,
                span: Span::FIRST,
            });
        }
    }
    ExplorerModel::new(ExplorerSnapshot {
        report: CheckReport {
            scope: ScanScope::All,
            context: ProjectContext::default(),
            files_checked: count,
            cache_hits: 0,
            cache_misses: count,
            cache_fingerprint: String::from("00"),
            diagnostics,
        },
        sources: BTreeMap::new(),
        elapsed_ms: 1,
        agent_markdown: String::from("## Garde-fou Report"),
    })
}

proptest! {
    #[test]
    fn ansi_paint_and_pill_preserve_exact_visible_width_and_stripped_text(
        text in printable_text_strategy(),
        role in role_strategy(),
        pill_role in pill_role_strategy(),
        policy in policy_strategy(),
    ) {
        let base_w = visible_width(&text);

        let painted = paint(&text, role, policy);
        prop_assert_eq!(visible_width(&painted), base_w);
        prop_assert_eq!(strip_ansi(&painted), text.as_str());

        let bolded = paint_bold(&text, role, policy);
        prop_assert_eq!(visible_width(&bolded), base_w);
        prop_assert_eq!(strip_ansi(&bolded), text.as_str());

        let badged = pill(&text, pill_role, policy);
        prop_assert_eq!(visible_width(&badged), base_w + 2);
        let stripped_pill = strip_ansi(&badged);
        let expected = if policy == ColorPolicy::Plain {
            format!("[{text}]")
        } else {
            format!(" {text} ")
        };
        prop_assert_eq!(stripped_pill, expected);
    }

    #[test]
    fn rounded_card_obeys_strict_rectangular_width_invariant(
        title in prop::option::of(printable_text_strategy()),
        lines in prop::collection::vec(printable_text_strategy(), 0..10),
        role in role_strategy(),
        policy in policy_strategy(),
        max_width in 20_usize..=140,
    ) {
        let rendered = rounded_card(
            title.as_deref(),
            &lines,
            CardStyle::new(role, policy),
            max_width,
        );
        let row_widths: Vec<usize> = rendered.lines().map(visible_width).collect();
        prop_assert!(row_widths.len() >= 2);
        let expected_w = row_widths[0];
        prop_assert!(expected_w <= max_width);
        for w in row_widths {
            prop_assert_eq!(w, expected_w);
        }
    }

    #[test]
    fn fuzzing_wizard_and_explorer_keys_keeps_cursor_in_valid_bounds(
        keys in prop::collection::vec(key_strategy(), 1..60),
        diag_count in 0_usize..=15,
    ) {
        let mut wiz = WizardModel::new(PathBuf::from("/tmp/fuzz"), None, false);
        for &k in &keys {
            let _ = wiz.handle_key(k);
            let w_len = wiz.visible_len();
            if w_len == 0 {
                prop_assert_eq!(wiz.cursor(), 0);
            } else {
                prop_assert!(wiz.cursor() < w_len);
            }
        }

        let mut exp = make_explorer_with_count(diag_count);
        for &k in &keys {
            let _ = exp.handle_key(k);
            let e_len = exp.filtered_indices().len();
            if e_len == 0 {
                prop_assert_eq!(exp.cursor(), 0);
            } else {
                prop_assert!(exp.cursor() < e_len);
            }
        }
    }
}

#[test]
fn visible_width_ignores_ansi_sgr_and_osc_escape_sequences() {
    assert_eq!(visible_width("\x1b[38;2;125;86;244méèà\x1b[0m"), 3);
    assert_eq!(
        visible_width("\x1b]8;;https://example.com\x07lien\x1b]8;;\x07"),
        4
    );
}
