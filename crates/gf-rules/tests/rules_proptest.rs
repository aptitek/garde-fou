//! Unit and property-based monotonicity tests for `gf-rules`.

use gf_core::{
    CallableDecl, CallableKind, Config, ImportDecl, Language, RepoPath, Span, SyntaxErrorNode,
    TypeDecl, TypeKind, UastDocument, UastKind, UastNode,
};
use gf_rules::{evaluate_document, evaluate_document_validated};
use proptest::prelude::*;
use proptest::test_runner::{Config as ProptestConfig, TestRunner};

fn runner() -> TestRunner {
    TestRunner::new(ProptestConfig {
        cases: 128,
        failure_persistence: None,
        ..ProptestConfig::default()
    })
}

#[derive(Clone, Copy)]
struct DocMetrics {
    file_lines: u32,
    fn_lines: u32,
    params: u32,
    cyc: u32,
    cog: u32,
    nest: u32,
    imports: u32,
    methods: u32,
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
            span,
        });
    }
    doc.types.push(TypeDecl {
        name: String::from("SampleType"),
        kind: TypeKind::Struct,
        span,
        field_count: 2,
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
    });
    Ok(doc)
}

#[test]
fn accumulates_all_rule_categories_without_short_circuiting()
-> Result<(), Box<dyn core::error::Error>> {
    let mut doc = build_synthetic_doc(DocMetrics {
        file_lines: 280,
        fn_lines: 60,
        params: 6,
        cyc: 14,
        cog: 18,
        nest: 5,
        imports: 16,
        methods: 12,
    })
    .map_err(|e| e.to_string())?;
    doc.syntax_errors.push(SyntaxErrorNode {
        message: String::from("unexpected token"),
        span: Span::new(3, 3)?,
    });
    doc.root.children.push(UastNode::leaf(
        UastKind::ForbiddenCall {
            symbol: String::from("unwrap"),
        },
        Span::new(1, 280)?,
    ));

    assert!(!evaluate_document_validated(&doc, &Config::strict_defaults()).is_valid());
    let ids: Vec<String> = evaluate_document(&doc, &Config::strict_defaults())
        .into_iter()
        .map(|d| d.rule_id)
        .collect();
    for expected in [
        "GF000", "GF001", "GF002", "GF003", "GF004", "GF005", "GF006", "GF007", "GF008", "GF010",
    ] {
        assert!(ids.iter().any(|id| id == expected), "missing {expected}");
    }
    Ok(())
}

#[test]
fn property_metrics_at_or_below_thresholds_produce_zero_diagnostics() -> Result<(), String> {
    let cfg = Config::strict_defaults();
    let l = cfg.limits;
    let mut tr = runner();
    let within_bounds = (
        1..=l.max_file_lines,
        1..=l.max_function_lines,
        0..=l.max_parameters,
        1..=l.max_cyclomatic_complexity,
        0..=l.max_cognitive_complexity,
        0..=l.max_nesting_depth,
        0..=l.max_imports_per_file,
        0..=l.max_methods_per_type,
    );

    tr.run(
        &within_bounds,
        |(file_lines, fn_lines, params, cyc, cog, nest, imports, methods)| {
            let doc = build_synthetic_doc(DocMetrics {
                file_lines,
                fn_lines,
                params,
                cyc,
                cog,
                nest,
                imports,
                methods,
            })?;
            prop_assert_eq!(evaluate_document(&doc, &cfg), Vec::new());
            Ok(())
        },
    )
    .map_err(|e| e.to_string())
}

#[test]
fn property_exceeding_any_threshold_monotonically_triggers_rule() -> Result<(), String> {
    let cfg = Config::strict_defaults();
    let l = cfg.limits;
    let mut tr = runner();
    let over_bounds = (
        (l.max_file_lines + 1)..600,
        (l.max_function_lines + 1)..200,
        (l.max_parameters + 1)..20,
        (l.max_cyclomatic_complexity + 1)..50,
        (l.max_cognitive_complexity + 1)..50,
        (l.max_nesting_depth + 1)..15,
    );

    tr.run(
        &over_bounds,
        |(file_lines, fn_lines, params, cyc, cog, nest)| {
            let doc = build_synthetic_doc(DocMetrics {
                file_lines,
                fn_lines,
                params,
                cyc,
                cog,
                nest,
                imports: 0,
                methods: 0,
            })?;
            let diags = evaluate_document(&doc, &cfg);
            prop_assert_eq!(diags.len(), 6);
            for diag in &diags {
                prop_assert!(diag.measured > diag.threshold);
                prop_assert_ne!(&diag.remediation, "");
            }
            Ok(())
        },
    )
    .map_err(|e| e.to_string())
}
