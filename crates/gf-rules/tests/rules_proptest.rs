//! Unit and property-based monotonicity tests for `gf-rules` (`GF000`–`GF011`, `GF050`–`GF051`).

use gf_core::{
    CallableDecl, CallableKind, Config, CoverageConfig, ForbiddenRuleKind, ImportDecl, Language,
    RepoPath, Span, SyntaxErrorNode, TypeDecl, TypeKind, UastDocument, UastKind, UastNode,
    parse_lcov,
};
use gf_rules::{
    evaluate_crap_scores, evaluate_document, evaluate_document_validated, evaluate_lcov_coverage,
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

#[derive(Clone, Copy, Debug)]
struct DocMetrics {
    file_lines: u32,
    fn_lines: u32,
    params: u32,
    cyc: u32,
    cog: u32,
    nest: u32,
    imports: u32,
    methods: u32,
    bool_fields: u32,
}

fn build_synthetic_doc(m: DocMetrics) -> Result<UastDocument, TestCaseError> {
    let path = RepoPath::new("src/synth.rs").map_err(|e| TestCaseError::fail(e.to_string()))?;
    let span = Span::new(1, m.file_lines).map_err(|e| TestCaseError::fail(e.to_string()))?;
    let mut doc = UastDocument::new(path, Language::Rust, m.file_lines, m.file_lines)
        .map_err(|e| TestCaseError::fail(e.to_string()))?;
    for i in 0..m.imports {
        doc.imports.push(ImportDecl {
            module_source: format!("m_{i}"),
            symbols: Vec::new(),
            is_type_only: false,
            span,
        });
    }
    doc.types.push(TypeDecl {
        name: String::from("SampleType"),
        kind: TypeKind::Struct,
        span,
        field_count: m.bool_fields.max(2),
        bool_field_count: m.bool_fields,
        method_count: m.methods,
    });
    doc.callables.push(CallableDecl {
        name: String::from("sample_fn"),
        kind: CallableKind::Function,
        span,
        effective_lines: m.fn_lines,
        param_count: m.params,
        cyclomatic_complexity: m.cyc,
        cognitive_complexity: m.cog,
        max_nesting_depth: m.nest,
        structural_hash: None,
        shingle_sketch: None,
        ast_node_count: 0,
    });
    Ok(doc)
}

fn build_overloaded_doc() -> Result<UastDocument, Box<dyn core::error::Error>> {
    let mut doc = build_synthetic_doc(DocMetrics {
        file_lines: 280,
        fn_lines: 60,
        params: 6,
        cyc: 14,
        cog: 18,
        nest: 5,
        imports: 16,
        methods: 12,
        bool_fields: 3,
    })
    .map_err(|e| e.to_string())?;
    doc.syntax_errors.push(SyntaxErrorNode {
        message: String::from("unexpected token"),
        span: Span::new(3, 3)?,
    });
    for (rule, sym) in [
        (ForbiddenRuleKind::Unwrap, ".unwrap()"),
        (ForbiddenRuleKind::SilentErrorSwallow, "let _ = ..."),
    ] {
        doc.root.children.push(UastNode::leaf(
            UastKind::ForbiddenCall {
                rule,
                symbol: String::from(sym),
            },
            Span::FIRST,
        ));
    }
    Ok(doc)
}

#[test]
fn accumulates_all_rule_categories_including_gf009_and_gf011()
-> Result<(), Box<dyn core::error::Error>> {
    let doc = build_overloaded_doc()?;
    assert!(!evaluate_document_validated(&doc, &Config::strict_defaults()).is_valid());
    let ids: Vec<String> = evaluate_document(&doc, &Config::strict_defaults())
        .into_iter()
        .map(|d| d.rule_id)
        .collect();
    for expected in [
        "GF000", "GF001", "GF002", "GF003", "GF004", "GF005", "GF006", "GF007", "GF008", "GF009",
        "GF010", "GF011",
    ] {
        assert!(ids.iter().any(|id| id == expected), "missing {expected}");
    }

    let mut selective = Config::strict_defaults();
    selective
        .rules
        .set_enabled(ForbiddenRuleKind::Unwrap, false);
    let filtered: Vec<String> = evaluate_document(&doc, &selective)
        .into_iter()
        .map(|d| d.rule_id)
        .collect();
    assert!(!filtered.iter().any(|id| id == "GF010"));
    assert!(filtered.iter().any(|id| id == "GF011"));
    Ok(())
}

#[test]
fn property_metrics_at_or_below_thresholds_produce_zero_diagnostics() -> Result<(), String> {
    let cfg = Config::strict_defaults();
    let l = cfg.limits;
    let mut tr = runner();
    let part_a = (
        (1..=l.max_file_lines, 1..=l.max_function_lines),
        (0..=l.max_parameters, 1..=l.max_cyclomatic_complexity),
    );
    let part_b = (
        (0..=l.max_cognitive_complexity, 0..=l.max_nesting_depth),
        (0..=l.max_imports_per_file, 0..=l.max_methods_per_type),
    );
    let within_bounds = ((part_a, part_b), 0..=l.max_struct_bools).prop_map(
        |(
            (((file_lines, fn_lines), (params, cyc)), ((cog, nest), (imports, methods))),
            bool_fields,
        )| DocMetrics {
            file_lines,
            fn_lines,
            params,
            cyc,
            cog,
            nest,
            imports,
            methods,
            bool_fields,
        },
    );

    tr.run(&within_bounds, |metrics| {
        let doc = build_synthetic_doc(metrics)?;
        prop_assert_eq!(evaluate_document(&doc, &cfg), Vec::new());
        Ok(())
    })
    .map_err(|e| e.to_string())
}

#[test]
fn property_exceeding_any_threshold_monotonically_triggers_rule() -> Result<(), String> {
    let cfg = Config::strict_defaults();
    let l = cfg.limits;
    let mut tr = runner();
    let part_a = (
        ((l.max_file_lines + 1)..600, (l.max_function_lines + 1)..200),
        (
            (l.max_parameters + 1)..20,
            (l.max_cyclomatic_complexity + 1)..50,
        ),
    );
    let part_b = (
        (
            (l.max_cognitive_complexity + 1)..50,
            (l.max_nesting_depth + 1)..15,
        ),
        (l.max_struct_bools + 1)..10,
    );
    let over_bounds = (part_a, part_b).prop_map(
        |(((file_lines, fn_lines), (params, cyc)), ((cog, nest), bool_fields))| DocMetrics {
            file_lines,
            fn_lines,
            params,
            cyc,
            cog,
            nest,
            imports: 0,
            methods: 0,
            bool_fields,
        },
    );

    tr.run(&over_bounds, |metrics| {
        let doc = build_synthetic_doc(metrics)?;
        let diags = evaluate_document(&doc, &cfg);
        prop_assert_eq!(diags.len(), 7);
        for diag in &diags {
            prop_assert!(diag.measured > diag.threshold);
            prop_assert_ne!(&diag.remediation, "");
        }
        Ok(())
    })
    .map_err(|e| e.to_string())
}

#[test]
fn lcov_branch_coverage_gf050_and_crap_score_gf051_enforce_thresholds()
-> Result<(), Box<dyn core::error::Error>> {
    let summary =
        parse_lcov("SF:src/synth.rs\nDA:1,0\nDA:2,0\nBRDA:1,0,0,1\nBRDA:1,0,1,0\nend_of_record\n");
    let cov_cfg = CoverageConfig {
        min_line_coverage: Some(80),
        min_branch_coverage: Some(80),
        max_crap_score: Some(30),
        lcov_path: None,
    };
    assert!(
        !evaluate_lcov_coverage(Some(&summary), &cov_cfg, &RepoPath::new("lcov.info")?).is_valid()
    );

    let doc = build_synthetic_doc(DocMetrics {
        file_lines: 10,
        fn_lines: 8,
        params: 1,
        cyc: 8,
        cog: 5,
        nest: 2,
        imports: 0,
        methods: 0,
        bool_fields: 0,
    })
    .map_err(|e| e.to_string())?;
    assert!(!evaluate_crap_scores(&[doc], Some(&summary), Some(30)).is_valid());
    Ok(())
}
