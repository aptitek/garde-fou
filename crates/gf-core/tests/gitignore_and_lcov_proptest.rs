//! Property-based tests for `merge_managed_block` idempotence and `LCOV` parsing invariants.

use gf_core::{
    Config, FrameworkId, LanguageId, ProjectContext, ScannerPolicy, compose_dockerignore,
    compose_gitignore, merge_managed_block, parse_lcov,
};
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
fn merge_managed_block_is_idempotent_and_preserves_custom_lines() -> Result<(), String> {
    let mut ctx = ProjectContext::default();
    ctx.languages.insert(LanguageId::Rust);
    ctx.frameworks.insert(FrameworkId::Astro);
    let block_v1 = compose_gitignore(&ctx);

    ctx.languages.insert(LanguageId::Python);
    let block_v2 = compose_gitignore(&ctx);

    let mut tr = runner();
    let custom_lines = prop::collection::vec("[a-zA-Z0-9_./*-]{1,20}", 0..6);
    tr.run(&custom_lines, |lines| {
        let user_content = lines.join("\n");
        let existing = (!user_content.is_empty()).then_some(user_content.as_str());

        let once = merge_managed_block(existing, &block_v1);
        let twice = merge_managed_block(Some(&once), &block_v1);
        prop_assert_eq!(&once, &twice);

        let upgraded = merge_managed_block(Some(&once), &block_v2);
        prop_assert!(upgraded.contains("__pycache__/"));
        for line in &lines {
            prop_assert!(upgraded.contains(line.as_str()));
        }
        Ok(())
    })
    .map_err(|e| e.to_string())
}

#[test]
fn lcov_parser_never_panics_and_computes_exact_ratios() -> Result<(), String> {
    let sample = "TN:\nSF:./src/lib.rs\nDA:1,3\nDA:2,3\nDA:3,0\nDA:4,1\nend_of_record\n";
    let parsed = parse_lcov(sample);
    assert_eq!(parsed.total_lines_found, 4);
    assert_eq!(parsed.total_lines_hit, 3);
    assert_eq!(parsed.line_coverage_pct(), 75);

    let mut tr = runner();
    let strat = prop_oneof![
        ".*",
        "(SF:[a-z/_.]{1,12}\n(DA:[0-9]{1,3},[0-9]{1,3}\n){0,8}(LF:[0-9]{1,2}\nLH:[0-9]{1,2}\n)?end_of_record\n){0,4}",
    ];
    tr.run(&strat, |raw| {
        let summary = parse_lcov(&raw);
        prop_assert!(summary.total_lines_hit <= summary.total_lines_found);
        prop_assert!(summary.line_coverage_pct() <= 100);
        Ok(())
    })
    .map_err(|e| e.to_string())
}

#[test]
fn scanner_policy_and_dockerignore_cover_all_variants() {
    assert_eq!(
        ScannerPolicy::from_config_str("auto"),
        Some(ScannerPolicy::Auto)
    );
    assert_eq!(
        ScannerPolicy::from_config_str("required"),
        Some(ScannerPolicy::Required)
    );
    assert_eq!(
        ScannerPolicy::from_config_str("off"),
        Some(ScannerPolicy::Off)
    );
    assert_eq!(ScannerPolicy::from_config_str("invalid"), None);

    let mut ctx = ProjectContext::default();
    ctx.languages.insert(LanguageId::Rust);
    ctx.languages.insert(LanguageId::TypeScript);
    ctx.languages.insert(LanguageId::Python);
    let dockerignore = compose_dockerignore(&ctx);
    for expected in [".git", "target", "node_modules", ".venv", "__pycache__"] {
        assert!(dockerignore.contains(expected));
    }
    assert!(
        Config::strict_defaults()
            .canonical_string()
            .starts_with("v4|")
    );
}
