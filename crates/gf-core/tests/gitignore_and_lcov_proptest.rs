//! Property-based tests for `merge_managed_block` idempotence, `LCOV` (`DA:` + `BRDA:`)
//! branch/span parsing invariants, `CRAP` score monotonicity, and `v9` policy algebra.

use gf_core::{
    ArchitectureConfig, CleanCodeConfig, Config, ForbiddenRuleKind, ForbiddenRulesConfig,
    FrameworkId, LanguageId, MutationSummary, ProjectContext, ScannerPolicy, Span, TddConfig,
    TddParityGate, YagniRule, compose_dockerignore, compose_gitignore, compute_crap_score,
    merge_managed_block, parse_lcov, truncate_replacement,
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
fn lcov_parser_never_panics_and_computes_exact_line_and_brda_ratios() -> Result<(), String> {
    let sample = "TN:\nSF:./src/lib.rs\nDA:1,3\nDA:2,3\nDA:3,0\nDA:4,1\n\
                  BRDA:2,0,0,3\nBRDA:2,0,1,0\nBRDA:3,1,0,-\nend_of_record\n";
    let parsed = parse_lcov(sample);
    assert_eq!((parsed.total_lines_found, parsed.total_lines_hit), (4, 3));
    assert_eq!(parsed.line_coverage_pct(), 75);
    assert_eq!(
        (parsed.total_branches_found, parsed.total_branches_hit),
        (2, 1)
    );
    assert_eq!(parsed.branch_coverage_pct(), Some(50));

    let fallback = parse_lcov("SF:src/fb.rs\nLF:10\nLH:8\nBRF:4\nBRH:3\nend_of_record\n");
    assert_eq!(fallback.line_coverage_pct(), 80);
    assert_eq!(fallback.branch_coverage_pct(), Some(75));

    let mut tr = runner();
    let strat = prop_oneof![
        ".*",
        "(SF:[a-z/_.]{1,10}\n(DA:[0-9]{1,3},[0-9]{1,2}\n){0,6}(BRDA:[0-9]{1,3},[0-9],[0-9],(-|[0-9]{1,2})\n){0,6}end_of_record\n){0,3}",
    ];
    tr.run(&strat, |raw| {
        let summary = parse_lcov(&raw);
        prop_assert!(summary.total_lines_hit <= summary.total_lines_found);
        prop_assert!(summary.total_branches_hit <= summary.total_branches_found);
        prop_assert!(summary.line_coverage_pct() <= 100);
        if summary.total_branches_found == 0 {
            prop_assert_eq!(summary.branch_coverage_pct(), None);
        } else {
            prop_assert!(summary.branch_coverage_pct().is_some_and(|p| p <= 100));
        }
        Ok(())
    })
    .map_err(|e| e.to_string())
}

#[test]
fn span_coverage_and_crap_score_satisfy_algebraic_invariants() -> Result<(), String> {
    assert_eq!(compute_crap_score(5, 0), 30);
    assert_eq!(compute_crap_score(10, 100), 10);

    let mut tr = runner();
    let strat = (
        prop::collection::vec(0u64..5u64, 4..20),
        1u32..15u32,
        0u32..=100u32,
        0u32..=100u32,
    );
    tr.run(&strat, |(hits_vec, cyc, cov_a, cov_b)| {
        let (low_cov, high_cov) = if cov_a <= cov_b {
            (cov_a, cov_b)
        } else {
            (cov_b, cov_a)
        };
        prop_assert!(compute_crap_score(cyc, high_cov) <= compute_crap_score(cyc, low_cov));

        let mut da_lines = Vec::with_capacity(hits_vec.len());
        let mut expected_hit = 0_u32;
        for (idx, &hits) in hits_vec.iter().enumerate() {
            let line_no = u32::try_from(idx + 10).unwrap_or(10);
            da_lines.push(format!("DA:{line_no},{hits}"));
            if hits > 0 {
                expected_hit = expected_hit.saturating_add(1);
            }
        }
        let lcov = format!("SF:src/fn.rs\n{}\nend_of_record\n", da_lines.join("\n"));

        let parsed = parse_lcov(&lcov);
        let file_cov = parsed
            .files
            .get("src/fn.rs")
            .ok_or_else(|| TestCaseError::fail("missing src/fn.rs"))?;
        let end_line = u32::try_from(9 + hits_vec.len()).unwrap_or(10);
        let fn_span = Span::new(10, end_line).map_err(|e| TestCaseError::fail(e.to_string()))?;
        let total = u32::try_from(hits_vec.len()).unwrap_or(1);
        let expected_pct = (expected_hit * 100) / total;

        prop_assert_eq!(file_cov.span_line_coverage_pct(fn_span), Some(expected_pct));
        prop_assert_eq!(
            file_cov.span_crap_score(fn_span, cyc),
            Some(compute_crap_score(cyc, expected_pct))
        );
        Ok(())
    })
    .map_err(|e| e.to_string())
}

#[test]
fn v9_policies_rules_architecture_and_clean_code_cover_all_variants() {
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

    let mut rules = ForbiddenRulesConfig::strict_defaults();
    assert!(rules.contains(ForbiddenRuleKind::Unsafe));
    assert!(rules.contains(ForbiddenRuleKind::SilentErrorSwallow));
    rules.set_enabled(ForbiddenRuleKind::Dbg, false);
    assert!(!rules.contains(ForbiddenRuleKind::Dbg));
    assert_eq!(ForbiddenRulesConfig::empty().active.len(), 0);

    let mut arch = ArchitectureConfig::empty();
    arch.no_std_crates.insert(String::from("gf-core"));
    assert!(arch.canonical_fragment().contains("ns=gf-core"));

    let cc = CleanCodeConfig::strict_defaults();
    assert!(cc.has_wildcard_exports());
    assert!(cc.has_single_impl_traits());
    assert!(cc.yagni_rules.contains(&YagniRule::WildcardExports));

    let mut tdd = TddConfig::strict_defaults();
    assert!(!tdd.require_companion());
    tdd.parity_gates.insert(TddParityGate::CompanionTest);
    assert!(tdd.require_companion());

    let mut ctx = ProjectContext::default();
    ctx.languages
        .extend([LanguageId::Rust, LanguageId::TypeScript, LanguageId::Python]);
    ctx.frameworks
        .extend([FrameworkId::Astro, FrameworkId::NextJs, FrameworkId::Django]);
    let gi = compose_gitignore(&ctx);
    assert!(gi.contains(">>>\n# OS & Editors\n") && gi.contains("\n\n# Garde-fou & Coverage\n"));
    assert!(gi.contains(".astro/") && gi.contains(".next/") && gi.contains("db.sqlite3"));
    let dockerignore = compose_dockerignore(&ctx);
    for expected in [".git", "target", "node_modules", ".venv", "__pycache__"] {
        assert!(dockerignore.contains(expected));
    }
    assert!(
        Config::strict_defaults()
            .canonical_string()
            .starts_with("v17|")
    );
}

#[test]
fn lcov_edge_records_and_span_queries_are_exact() {
    let lcov = parse_lcov(
        "SF:empty.rs\nend_of_record\n\
         SF:br.rs\nDA:0,5\nBRDA:0,0,0,5\nBRDA:2,0,0,5\nBRDA:2,0,1,2\nBRDA:2,0,2,0\nend_of_record\n\
         SF:fn.rs\nDA:10,1\nDA:11,1\nDA:12,1\nDA:13,0\nend_of_record\n",
    );
    assert!(!lcov.files.contains_key("empty.rs"));
    assert_eq!(lcov.files["br.rs"].lines_found, 0);
    assert_eq!(lcov.files["br.rs"].branch_percent_floor(), Some(66));
    assert_eq!(lcov.files["fn.rs"].percent_floor(), 75);
    assert_eq!(lcov.files["fn.rs"].branch_percent_floor(), None);
    assert_eq!(
        lcov.files["fn.rs"].span_line_coverage_pct(Span::FIRST),
        None
    );
}

#[test]
fn mutation_summary_score_satisfies_algebraic_invariants() -> Result<(), String> {
    let mut runner = runner();
    runner
        .run(
            &(0_u32..5_000, 0_u32..1_000, 0_u32..5_000, 0_u32..5_000),
            |(caught, timeout, missed, unviable)| {
                let s = MutationSummary {
                    caught,
                    timeout,
                    missed,
                    unviable,
                    survivors: Vec::new(),
                };
                if s.total_viable() == 0 {
                    prop_assert_eq!(s.score_percent_floor(), None);
                } else {
                    let pct = s.score_percent_floor().unwrap_or(101);
                    prop_assert!(pct <= 100);
                    let with_more_unviable = MutationSummary {
                        unviable: unviable.saturating_add(77),
                        ..s
                    };
                    prop_assert_eq!(with_more_unviable.score_percent_floor(), Some(pct));
                }
                prop_assert!(truncate_replacement(&"x".repeat(200)).chars().count() <= 120);
                Ok(())
            },
        )
        .map_err(|err| format!("{err}"))
}
