//! Direct unit tests in `gf-rules` for `tdd_checks.rs`, `tdd_parity.rs`,
//! and `clone_checks.rs` (`GF020`, `GF076`).

use gf_core::{
    CallableDecl, CallableKind, Config, Language, ProjectContext, RepoPath, Span, TddParityGate,
    TestCaseSummary, UastDocument,
};
use gf_rules::{evaluate_document, evaluate_workspace_sorted};

type Res = Result<(), Box<dyn std::error::Error>>;

fn doc_at(path: &str, lang: Language) -> Result<UastDocument, Box<dyn std::error::Error>> {
    Ok(UastDocument::new(RepoPath::new(path)?, lang, 10, 8)?)
}

fn fn_decl(name: &str, span: Span, lines: u32, nodes: u32) -> CallableDecl {
    CallableDecl {
        name: name.into(),
        kind: CallableKind::Function,
        span,
        effective_lines: lines,
        param_count: 1,
        cyclomatic_complexity: 2,
        cognitive_complexity: 1,
        max_nesting_depth: 1,
        structural_hash: None,
        shingle_sketch: None,
        ast_node_count: nodes,
    }
}

fn tc(assertions: u32) -> TestCaseSummary {
    TestCaseSummary {
        name: "test_case".into(),
        assertion_count: assertions,
        has_tautology: false,
        has_weak_only_assertion: false,
        span: Span::FIRST,
    }
}

#[test]
fn tdd_local_cases_and_companion_stories_enforce_exact_edges() -> Res {
    let def = Config::strict_defaults();
    assert_eq!(
        evaluate_document(&doc_at("tests/notes.md", Language::Markdown)?, &def),
        Vec::new()
    );
    assert_eq!(
        evaluate_document(
            &doc_at("tests/helpers/setup.ts", Language::TypeScript)?,
            &def
        ),
        Vec::new()
    );
    assert_eq!(
        evaluate_document(&doc_at("tests/conftest.py", Language::Python)?, &def),
        Vec::new()
    );

    let mut good_test = doc_at("tests/good.test.ts", Language::TypeScript)?;
    good_test.test_cases.push(tc(1));
    assert_eq!(evaluate_document(&good_test, &def), Vec::new());
    let mut empty_case = doc_at("tests/bad.test.ts", Language::TypeScript)?;
    empty_case.test_cases.push(tc(0));
    assert_eq!(evaluate_document(&empty_case, &def)[0].rule_id, "GF076");

    verify_workspace_companions_and_stories()
}

fn verify_workspace_companions_and_stories() -> Res {
    let mut cfg = Config::strict_defaults();
    cfg.tdd.parity_gates.extend([
        TddParityGate::CompanionTest,
        TddParityGate::ComponentStories,
    ]);

    let mut uncovered = doc_at("src/calc.ts", Language::TypeScript)?;
    uncovered.callables.push(fn_decl("add", Span::FIRST, 5, 10));
    let empty_coloc = doc_at("src/calc.test.ts", Language::TypeScript)?;

    let mut by_test = doc_at("src/alpha.ts", Language::TypeScript)?;
    by_test.callables.push(fn_decl("a", Span::FIRST, 5, 10));
    let mut alpha_test = doc_at("src/alpha.test.ts", Language::TypeScript)?;
    alpha_test.test_cases.push(tc(1));

    let mut by_spec = doc_at("src/beta.ts", Language::TypeScript)?;
    by_spec.callables.push(fn_decl("b", Span::FIRST, 5, 10));
    let mut beta_spec = doc_at("src/beta.spec.ts", Language::TypeScript)?;
    beta_spec.test_cases.push(tc(1));

    let docs = vec![
        uncovered,
        empty_coloc,
        by_test,
        alpha_test,
        by_spec,
        beta_spec,
        doc_at("src/components/atoms/Button.tsx", Language::Tsx)?,
        doc_at("src/components/atoms/Button.test.tsx", Language::Tsx)?,
        doc_at("src/components/atoms/Button.spec.tsx", Language::Tsx)?,
        doc_at("src/components/atoms/Card.tsx", Language::Tsx)?,
        doc_at("src/components/atoms/Card.stories.tsx", Language::Tsx)?,
        doc_at("src/components/atoms/index.tsx", Language::Tsx)?,
    ];
    let diags = evaluate_workspace_sorted(&docs, &ProjectContext::default(), &cfg);
    let paths: Vec<&str> = diags
        .iter()
        .filter(|d| d.rule_id == "GF076")
        .map(|d| d.path.as_str())
        .collect();
    assert_eq!(
        paths,
        vec!["src/calc.ts", "src/components/atoms/Button.tsx"]
    );
    Ok(())
}

#[test]
fn type2_and_type3_clones_enforce_exact_size_and_domain_guards() -> Res {
    let def = Config::strict_defaults();
    let mut c1 = doc_at("src/c1.rs", Language::Rust)?;
    let mut f_exact = fn_decl(
        "fa",
        Span::new(1, 20)?,
        def.limits.min_clone_lines,
        def.limits.min_clone_nodes,
    );
    f_exact.structural_hash = Some(99);
    let mut f_short = fn_decl("fshort", Span::new(21, 24)?, 20, 10);
    f_short.structural_hash = Some(99);

    let mut f_near_left = fn_decl("fb", Span::new(25, 45)?, def.limits.min_clone_lines, 85);
    f_near_left.shingle_sketch = Some([1, 2, 3, 4, 5, 6, 7, 8]);
    let mut f_near_right = fn_decl("fc", Span::new(50, 70)?, def.limits.min_clone_lines, 100);
    f_near_right.shingle_sketch = Some([1, 2, 3, 4, 5, 6, 7, 9]);
    let mut f_low_sim = fn_decl("flow", Span::new(75, 95)?, def.limits.min_clone_lines, 90);
    f_low_sim.shingle_sketch = Some([1, 90, 91, 92, 93, 94, 95, 96]);
    c1.callables = vec![
        f_exact.clone(),
        f_short,
        f_near_left.clone(),
        f_near_right.clone(),
        f_low_sim,
    ];

    let mut c2 = doc_at("src/c2.rs", Language::Rust)?;
    let mut f_exact2 = f_exact;
    f_exact2.name = "fa_peer".into();
    c2.callables.push(f_exact2);

    let mut test_clones = doc_at("tests/t_clones.rs", Language::Rust)?;
    test_clones.callables = vec![f_near_left, f_near_right];

    let cl_d = evaluate_workspace_sorted(&[c1, c2, test_clones], &ProjectContext::default(), &def);
    let gf020: Vec<_> = cl_d.iter().filter(|d| d.rule_id == "GF020").collect();
    assert_eq!(gf020.len(), 4);
    assert!(
        gf020
            .iter()
            .any(|d| d.path.as_str() == "src/c1.rs" && d.message.contains("src/c2.rs"))
    );
    assert!(
        gf020
            .iter()
            .any(|d| d.path.as_str() == "src/c2.rs" && d.message.contains("src/c1.rs"))
    );
    Ok(())
}
