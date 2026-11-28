//! Unit tests killing 100% of `cargo-mutants` in `gf-rules` (`evaluate_tuple_kiss`,
//! `check_local_notebook_rules`, and `check_workspace_academic`).

use gf_core::{
    AcademicSummary, Config, KissYagniFinding, KissYagniKind, Language, NotebookCellMetrics,
    ProjectContext, RepoPath, Span, UastDocument, WorkspaceRefItem,
};
use gf_rules::{evaluate_document, evaluate_workspace};

type Res = Result<(), Box<dyn std::error::Error>>;

fn make_doc(path: &str, lang: Language) -> Result<UastDocument, Box<dyn std::error::Error>> {
    Ok(UastDocument::new(RepoPath::new(path)?, lang, 10, 10)?)
}

fn has_rule(diags: &[gf_core::Diagnostic], rule: &str) -> bool {
    diags.iter().any(|d| d.rule_id == rule)
}

#[test]
fn tuple_kiss_exact_boundaries_kill_all_mutants() -> Res {
    let mut cfg = Config::strict_defaults();
    cfg.clean_code.max_tuple_arity = 2;
    cfg.clean_code.forbid_global_tuple_unpacking = true;

    let mut below = make_doc("src/a.rs", Language::Rust)?;
    below.kiss_yagni_findings.push(KissYagniFinding {
        kind: KissYagniKind::AnonymousTuple,
        measured: 1,
        detail: String::from("1-tuple"),
        span: Span::FIRST,
    });
    assert!(!has_rule(&evaluate_document(&below, &cfg), "GF073"));

    let mut equal = make_doc("src/a.rs", Language::Rust)?;
    equal.kiss_yagni_findings.push(KissYagniFinding {
        kind: KissYagniKind::AnonymousTuple,
        measured: 2,
        detail: String::from("2-tuple"),
        span: Span::FIRST,
    });
    assert!(!has_rule(&evaluate_document(&equal, &cfg), "GF073"));

    let mut above = make_doc("src/a.rs", Language::Rust)?;
    above.kiss_yagni_findings.push(KissYagniFinding {
        kind: KissYagniKind::AnonymousTuple,
        measured: 3,
        detail: String::from("3-tuple"),
        span: Span::FIRST,
    });
    assert!(has_rule(&evaluate_document(&above, &cfg), "GF073"));

    let mut global_unpack = make_doc("src/a.py", Language::Python)?;
    global_unpack.kiss_yagni_findings.push(KissYagniFinding {
        kind: KissYagniKind::GlobalTupleUnpacking,
        measured: 1,
        detail: String::from("a, b = pair()"),
        span: Span::FIRST,
    });
    assert!(has_rule(&evaluate_document(&global_unpack, &cfg), "GF073"));

    cfg.clean_code.forbid_global_tuple_unpacking = false;
    assert!(!has_rule(&evaluate_document(&global_unpack, &cfg), "GF073"));
    Ok(())
}

fn nb_doc(eff: u32) -> Result<UastDocument, Box<dyn std::error::Error>> {
    let mut doc = make_doc("notebooks/a.ipynb", Language::Notebook)?;
    doc.academic = Some(AcademicSummary {
        notebook_cell_lines: vec![NotebookCellMetrics {
            cell_index: 1,
            effective_lines: eff,
            span: Span::FIRST,
        }],
        ..AcademicSummary::default()
    });
    Ok(doc)
}

#[test]
fn notebook_cell_size_boundaries_and_workspace_academic_kill_all_mutants() -> Res {
    let mut cfg = Config::strict_defaults();
    cfg.limits.max_function_lines = 50;

    assert!(!has_rule(&evaluate_document(&nb_doc(49)?, &cfg), "GF080"));
    assert!(!has_rule(&evaluate_document(&nb_doc(50)?, &cfg), "GF080"));
    assert!(has_rule(&evaluate_document(&nb_doc(51)?, &cfg), "GF080"));

    let ctx = ProjectContext::default();
    let plain = make_doc("src/lib.rs", Language::Rust)?;
    assert!(evaluate_workspace(&[plain], &ctx, &cfg).is_valid());

    let mut broken_obs = make_doc("notes/index.md", Language::Markdown)?;
    broken_obs.academic = Some(AcademicSummary {
        obsidian_wikilinks: vec![WorkspaceRefItem {
            target: String::from("missing-note"),
            fragment: None,
            span: Span::FIRST,
        }],
        ..AcademicSummary::default()
    });
    let ws_errs = evaluate_workspace(&[broken_obs], &ctx, &cfg)
        .into_result()
        .err()
        .unwrap_or_default();
    assert!(has_rule(&ws_errs, "GF080"));
    Ok(())
}
