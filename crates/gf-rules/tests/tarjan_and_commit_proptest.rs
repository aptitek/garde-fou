//! Property-based tests for Tarjan SCC cycle detection (`GF071`) and Conventional Commits (`GF077`).

use std::collections::{BTreeMap, BTreeSet};

use gf_core::{
    CallableDecl, CallableKind, CommitConfig, Config, FeatureId, Language, ProjectContext,
    RepoPath, Span, TestCaseSummary, UastDocument, Validation, validate_commit_message,
};
use gf_rules::{evaluate_document, evaluate_workspace_sorted, find_scc_cycles};
use proptest::prelude::*;
use proptest::test_runner::{Config as ProptestConfig, TestRunner};

fn runner() -> TestRunner {
    TestRunner::new(ProptestConfig {
        cases: 128,
        failure_persistence: None,
        ..ProptestConfig::default()
    })
}

#[test]
fn tarjan_never_flags_strict_dags_and_always_detects_added_back_edges() -> Result<(), String> {
    let mut tr = runner();
    let dag_strat = (
        3usize..10usize,
        prop::collection::vec((0usize..10, 0usize..10), 0..24),
    );

    tr.run(&dag_strat, |(n, raw_edges)| {
        let mut adj: BTreeMap<usize, BTreeSet<usize>> = BTreeMap::new();
        for &(a, b) in &raw_edges {
            let u = a % n;
            let v = b % n;
            if u < v {
                adj.entry(u).or_default().insert(v);
            }
        }
        prop_assert_eq!(find_scc_cycles(n, &adj), Vec::<Vec<usize>>::new());

        // Inject a guaranteed forward chain 0 -> 1 -> (n - 1) plus back-edge (n - 1) -> 0.
        adj.entry(0).or_default().insert(1);
        adj.entry(1).or_default().insert(n - 1);
        adj.entry(n - 1).or_default().insert(0);

        let cycles = find_scc_cycles(n, &adj);
        prop_assert!(!cycles.is_empty());
        for cyc in &cycles {
            prop_assert!(cyc.len() >= 2);
            prop_assert_eq!(cyc.first(), cyc.last());
            for win in cyc.windows(2) {
                let has_edge = adj.get(&win[0]).is_some_and(|set| set.contains(&win[1]));
                prop_assert!(has_edge);
            }
        }
        Ok(())
    })
    .map_err(|e| e.to_string())
}

#[test]
fn commit_msg_validator_never_panics_and_enforces_exact_max_header_len() -> Result<(), String> {
    let mut tr = runner();
    let cfg = CommitConfig::strict_defaults();

    tr.run(&".*", |arbitrary| {
        let first = validate_commit_message(&arbitrary, &cfg);
        let second = validate_commit_message(&arbitrary, &cfg);
        prop_assert_eq!(first, second);
        Ok(())
    })
    .map_err(|e| e.to_string())?;

    let exact_strat = (20usize..80usize, "[a-z]{5,12}");
    tr.run(&exact_strat, |(max_len, scope)| {
        let mut custom = CommitConfig::strict_defaults();
        custom.max_header_len = max_len;
        let prefix = format!("feat({scope}): ");
        let pad_ok = max_len.saturating_sub(prefix.len()).max(5);
        let msg_ok = format!("{prefix}{}", "a".repeat(pad_ok));
        if msg_ok.len() <= max_len {
            prop_assert_eq!(
                validate_commit_message(&msg_ok, &custom),
                Validation::Valid(())
            );
        }

        let msg_too_long = format!("{prefix}{}", "b".repeat(max_len));
        let res = validate_commit_message(&msg_too_long, &custom);
        prop_assert!(matches!(res, Validation::Invalid(_)));
        Ok(())
    })
    .map_err(|e| e.to_string())
}

#[test]
fn uast_document_and_language_predicates_hold() -> Result<(), Box<dyn std::error::Error>> {
    let doc = UastDocument::new(RepoPath::new("src/lib.rs")?, Language::Rust, 42, 35)?;
    assert!(doc.language.is_code_analyzed());
    assert!(Language::Css.is_always_discovered());
    assert!(Language::Markdown.is_always_discovered());
    assert!(!Language::Toml.is_always_discovered());
    assert_eq!(doc.total_lines, 42);
    Ok(())
}

#[test]
fn tdd_companion_stories_tautology_and_commit_scopes_hold() -> Result<(), Box<dyn std::error::Error>>
{
    let cfg = Config::strict_defaults();
    let mut ctx = ProjectContext::default();
    ctx.features.insert(FeatureId::Tdd);
    ctx.features.insert(FeatureId::Storybook);

    let mut comp = UastDocument::new(
        RepoPath::new("src/components/atoms/Badge.tsx")?,
        Language::Tsx,
        10,
        8,
    )?;
    comp.callables.push(CallableDecl {
        name: String::from("Badge"),
        kind: CallableKind::Function,
        span: Span::FIRST,
        effective_lines: 6,
        param_count: 0,
        cyclomatic_complexity: 1,
        cognitive_complexity: 0,
        max_nesting_depth: 0,
        structural_hash: None,
        ast_node_count: 12,
    });
    comp.test_cases.push(TestCaseSummary {
        name: String::from("tautology_case"),
        assertion_count: 1,
        has_tautology: true,
        span: Span::FIRST,
    });

    let local_diags = evaluate_document(&comp, &cfg);
    assert!(local_diags.iter().any(|d| d.rule_id == "GF076"));

    let ws_diags = evaluate_workspace_sorted(&[comp], &ctx, &cfg);
    assert!(
        ws_diags
            .iter()
            .any(|d| d.rule_id == "GF076" && d.message.contains(".stories.tsx"))
    );
    Ok(())
}

#[test]
fn commit_msg_enforces_allowed_scopes_and_breaking_bang() {
    let mut scoped_commit = CommitConfig::strict_defaults();
    scoped_commit.allowed_scopes = vec![String::from("rules")];
    assert_eq!(
        validate_commit_message(
            "# comment\nfeat(rules)!: enforce strict dag\n",
            &scoped_commit
        ),
        Validation::Valid(())
    );
    assert!(matches!(
        validate_commit_message("feat(unknown): enforce strict dag\n", &scoped_commit),
        Validation::Invalid(_)
    ));
}
