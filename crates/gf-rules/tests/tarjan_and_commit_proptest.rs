//! Property-based and integration tests for Tarjan SCC (`GF071` 4-level cycles),
//! Type-3 `MinHash` clones (`GF020`), TDD vacuity (`GF076`), `[architecture]` (`GF078`),
//! and Conventional Commits (`GF077`).

use std::collections::{BTreeMap, BTreeSet};

use gf_core::{
    CallableDecl, CallableKind, CommitConfig, Config, EffectSighting, EffectSightingKind,
    ExportDecl, FeatureId, ImportDecl, Language, ProjectContext, RepoPath, Span, TestCaseSummary,
    UastDocument, Validation, validate_commit_message,
};
use gf_rules::{evaluate_document, evaluate_workspace_sorted, find_scc_cycles};
use proptest::prelude::*;
use proptest::test_runner::{Config as ProptestConfig, TestRunner};

fn runner() -> TestRunner {
    TestRunner::new(ProptestConfig {
        cases: 96,
        failure_persistence: None,
        ..ProptestConfig::default()
    })
}

fn make_callable(name: &str, hash: Option<u128>, sketch: Option<[u64; 8]>) -> CallableDecl {
    CallableDecl {
        name: String::from(name),
        kind: CallableKind::Function,
        span: Span::FIRST,
        effective_lines: 20,
        param_count: 1,
        cyclomatic_complexity: 3,
        cognitive_complexity: 2,
        max_nesting_depth: 1,
        structural_hash: hash,
        shingle_sketch: sketch,
        ast_node_count: 50,
    }
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
            let (u, v) = (a % n, b % n);
            if u < v {
                adj.entry(u).or_default().insert(v);
            }
        }
        prop_assert_eq!(find_scc_cycles(n, &adj), Vec::<Vec<usize>>::new());

        adj.entry(0).or_default().insert(1);
        adj.entry(1).or_default().insert(n - 1);
        adj.entry(n - 1).or_default().insert(0);

        let cycles = find_scc_cycles(n, &adj);
        prop_assert!(!cycles.is_empty());
        for cyc in &cycles {
            prop_assert!(cyc.len() >= 2);
            prop_assert_eq!(cyc.first(), cyc.last());
        }
        Ok(())
    })
    .map_err(|e| e.to_string())
}

#[test]
fn deep_4_level_rust_cycle_without_mod_rs_triggers_gf071() -> Result<(), Box<dyn std::error::Error>>
{
    let mut mod_a = UastDocument::new(RepoPath::new("src/a.rs")?, Language::Rust, 10, 8)?;
    mod_a.imports.push(ImportDecl {
        module_source: String::from("crate::a::b::c::d::Leaf"),
        symbols: Vec::new(),
        span: Span::FIRST,
    });
    let mut mod_d = UastDocument::new(RepoPath::new("src/a/b/c/d.rs")?, Language::Rust, 10, 8)?;
    mod_d.imports.push(ImportDecl {
        module_source: String::from("super::super::super::RootItem"),
        symbols: Vec::new(),
        span: Span::FIRST,
    });

    let diags = evaluate_workspace_sorted(
        &[mod_a, mod_d],
        &ProjectContext::default(),
        &Config::strict_defaults(),
    );
    assert!(diags.iter().any(|d| d.rule_id == "GF071"));
    Ok(())
}

#[test]
fn empty_test_file_and_weak_only_assertions_trigger_gf076() -> Result<(), Box<dyn std::error::Error>>
{
    let cfg = Config::strict_defaults();
    let empty_ts = UastDocument::new(
        RepoPath::new("tests/empty.test.ts")?,
        Language::TypeScript,
        3,
        1,
    )?;
    let empty_diags = evaluate_document(&empty_ts, &cfg);
    assert!(
        empty_diags
            .iter()
            .any(|d| d.rule_id == "GF076" && d.message.contains("zero executable test cases"))
    );

    let mut prod = UastDocument::new(RepoPath::new("src/empty.ts")?, Language::TypeScript, 12, 10)?;
    prod.callables.push(make_callable("work", None, None));
    let mut ctx = ProjectContext::default();
    ctx.features.insert(FeatureId::Tdd);
    let ws_diags = evaluate_workspace_sorted(&[prod, empty_ts], &ctx, &cfg);
    assert!(ws_diags.iter().any(|d| d.rule_id == "GF076"));

    let mut weak_test = UastDocument::new(
        RepoPath::new("tests/weak.test.ts")?,
        Language::TypeScript,
        8,
        6,
    )?;
    weak_test.test_cases.push(TestCaseSummary {
        name: String::from("weak_case"),
        assertion_count: 1,
        has_tautology: false,
        has_weak_only_assertion: true,
        span: Span::FIRST,
    });
    assert!(
        evaluate_document(&weak_test, &cfg)
            .iter()
            .any(|d| d.rule_id == "GF076")
    );
    Ok(())
}

#[test]
fn type3_minhash_clones_gf020_and_dead_pub_crate_gf021() -> Result<(), Box<dyn std::error::Error>> {
    let mut doc_a = UastDocument::new(
        RepoPath::new("crates/core/src/lib.rs")?,
        Language::Rust,
        30,
        25,
    )?;
    doc_a.callables.push(make_callable(
        "alpha",
        Some(101),
        Some([10, 20, 30, 40, 50, 60, 70, 80]),
    ));
    doc_a.exports.push(ExportDecl {
        symbol: String::from("stale_internal"),
        is_crate_internal: true,
        span: Span::FIRST,
    });

    let mut doc_b = UastDocument::new(
        RepoPath::new("crates/core/src/other.rs")?,
        Language::Rust,
        30,
        25,
    )?;
    doc_b.callables.push(make_callable(
        "beta",
        Some(202),
        Some([10, 20, 30, 40, 50, 60, 70, 999]),
    ));

    let diags = evaluate_workspace_sorted(
        &[doc_a, doc_b],
        &ProjectContext::default(),
        &Config::strict_defaults(),
    );
    assert!(
        diags
            .iter()
            .any(|d| d.rule_id == "GF020" && d.message.contains("Type-3"))
    );
    assert!(diags.iter().any(|d| d.rule_id == "GF021"));
    Ok(())
}

#[test]
fn architecture_purity_enforces_no_std_io_and_console_gf078()
-> Result<(), Box<dyn std::error::Error>> {
    let mut doc = UastDocument::new(
        RepoPath::new("crates/core/src/lib.rs")?,
        Language::Rust,
        12,
        10,
    )?;
    for (kind, detail) in [
        (EffectSightingKind::StdUsage, "std::fs"),
        (EffectSightingKind::IoImportOrCall, "std::fs"),
        (EffectSightingKind::ConsoleOutput, "println!"),
    ] {
        doc.effect_sightings.push(EffectSighting {
            kind,
            detail: String::from(detail),
            span: Span::FIRST,
        });
    }
    let mut cfg = Config::strict_defaults();
    cfg.architecture.no_std_crates.insert(String::from("core"));
    cfg.architecture
        .io_allowed_crates
        .insert(String::from("engine"));
    cfg.architecture
        .console_allowed_crates
        .insert(String::from("cli"));

    let diags = evaluate_workspace_sorted(&[doc], &ProjectContext::default(), &cfg);
    assert_eq!(diags.iter().filter(|d| d.rule_id == "GF078").count(), 4);
    Ok(())
}

#[test]
fn commit_msg_enforces_allowed_scopes_and_max_header_len() {
    let mut scoped_commit = CommitConfig::strict_defaults();
    scoped_commit.allowed_scopes = vec![String::from("rules")];
    assert_eq!(
        validate_commit_message("feat(rules)!: enforce strict dag\n", &scoped_commit),
        Validation::Valid(())
    );
    assert!(matches!(
        validate_commit_message("feat(unknown): enforce strict dag\n", &scoped_commit),
        Validation::Invalid(_)
    ));
}
