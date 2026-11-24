//! Direct unit tests in `gf-rules` for `security_checks.rs`, `mutation_checks.rs`,
//! and `tool_checks.rs` (`GF040`, `GF050`–`GF052`, `GF080`–`GF082`).

use std::slice;

use gf_core::{
    CallableDecl, CallableKind, Config, CoverageConfig, CoverageFailUnder, DebugArtifactKind,
    ExternalToolFinding, ExternalToolId, LangRuleFinding, Language, MutationConfig,
    MutationSummary, PythonViolation, RepoPath, RuffConfigIssue, SecretFinding, Span,
    SurvivedMutant, ToolConfigFinding, ToolConfigViolation, UastDocument, UnsafeExecKind,
    Validation, parse_lcov,
};
use gf_rules::{
    convert_external_tool_findings, evaluate_crap_scores, evaluate_document,
    evaluate_lcov_coverage, evaluate_mutation_summary,
};

type Res = Result<(), Box<dyn std::error::Error>>;

fn simple_fn(name: &str, end: u32, cyc: u32) -> Result<CallableDecl, gf_core::SpanError> {
    Ok(CallableDecl {
        name: name.into(),
        kind: CallableKind::Function,
        span: Span::new(1, end)?,
        effective_lines: end,
        param_count: 0,
        cyclomatic_complexity: cyc,
        cognitive_complexity: 1,
        max_nesting_depth: 1,
        structural_hash: None,
        shingle_sketch: None,
        ast_node_count: 10,
    })
}

#[test]
fn lcov_line_branch_crap_and_secrets_exact_boundaries() -> Res {
    let rp = RepoPath::new("lcov.info")?;
    assert_eq!(
        evaluate_lcov_coverage(None, &CoverageConfig::empty(), &rp),
        Validation::Valid(())
    );
    let mut cov_cfg = CoverageConfig::empty();
    cov_cfg.min_line_coverage = Some(75);
    cov_cfg.min_branch_coverage = Some(50);
    assert!(!evaluate_lcov_coverage(None, &cov_cfg, &rp).is_valid());
    match evaluate_lcov_coverage(Some(&parse_lcov("")), &cov_cfg, &rp) {
        Validation::Valid(()) => assert_eq!(0, 1),
        Validation::Invalid(d) => assert!(d[0].message.contains("zero instrumented records")),
    }

    let exact = parse_lcov(
        "SF:src/a.rs\nDA:1,1\nDA:2,1\nDA:3,1\nDA:4,0\nBRDA:2,0,0,1\nBRDA:2,0,1,0\nend_of_record\n",
    );
    assert_eq!(
        evaluate_lcov_coverage(Some(&exact), &cov_cfg, &rp),
        Validation::Valid(())
    );
    cov_cfg.min_line_coverage = Some(76);
    cov_cfg.min_branch_coverage = Some(51);
    match evaluate_lcov_coverage(Some(&exact), &cov_cfg, &rp) {
        Validation::Valid(()) => assert_eq!(75, 76),
        Validation::Invalid(d) => assert_eq!(d.len(), 2),
    }
    let no_br = parse_lcov("SF:src/a.rs\nDA:1,1\nend_of_record\n");
    cov_cfg.min_branch_coverage = None;
    assert_eq!(
        evaluate_lcov_coverage(Some(&no_br), &cov_cfg, &rp),
        Validation::Valid(())
    );
    cov_cfg.min_line_coverage = None;
    cov_cfg.min_branch_coverage = Some(51);
    assert!(!evaluate_lcov_coverage(Some(&no_br), &cov_cfg, &rp).is_valid());
    let br_only = parse_lcov("SF:src/b.rs\nBRDA:1,0,0,1\nend_of_record\n");
    assert_eq!(
        evaluate_lcov_coverage(Some(&br_only), &cov_cfg, &rp),
        Validation::Valid(())
    );

    verify_crap_and_native_secrets(&no_br)
}

fn verify_crap_and_native_secrets(no_br: &gf_core::LcovSummary) -> Res {
    let mut cdoc = UastDocument::new(RepoPath::new("src/a.rs")?, Language::Rust, 10, 8)?;
    cdoc.secret_findings.push(SecretFinding {
        kind: "pat".into(),
        redacted_preview: "ghp_***".into(),
        span: Span::FIRST,
    });
    assert_eq!(
        evaluate_document(&cdoc, &Config::default())[0].rule_id,
        "GF040"
    );
    cdoc.callables.push(simple_fn("f", 4, 8)?);
    assert_eq!(
        evaluate_crap_scores(slice::from_ref(&cdoc), Some(no_br), Some(8)),
        Validation::Valid(())
    );
    assert!(!evaluate_crap_scores(&[cdoc], Some(no_br), Some(7)).is_valid());

    let zero_cov = parse_lcov("SF:src/z.rs\nDA:1,0\nend_of_record\n");
    let mut zdoc = UastDocument::new(RepoPath::new("src/z.rs")?, Language::Rust, 5, 4)?;
    zdoc.callables.push(simple_fn("z", 3, 10)?);
    assert_eq!(
        evaluate_crap_scores(slice::from_ref(&zdoc), Some(&zero_cov), Some(110)),
        Validation::Valid(())
    );
    assert!(!evaluate_crap_scores(&[zdoc], Some(&zero_cov), Some(109)).is_valid());
    Ok(())
}

#[test]
fn mutation_summary_exact_boundaries() -> Res {
    let rp = RepoPath::new("mutants.out/outcomes.json")?;
    assert_eq!(
        evaluate_mutation_summary(None, &MutationConfig::empty(), &rp),
        Validation::Valid(())
    );
    let mcfg = MutationConfig {
        min_mutation_score: Some(80),
        zero_survivor_globs: vec![String::from("src/core/**")],
        report_path: None,
    };
    assert!(!evaluate_mutation_summary(None, &mcfg, &rp).is_valid());
    match evaluate_mutation_summary(Some(&MutationSummary::default()), &mcfg, &rp) {
        Validation::Valid(()) => assert_eq!(0, 1),
        Validation::Invalid(d) => assert!(d[0].message.contains("zero viable mutants")),
    }

    let surv = SurvivedMutant {
        path: RepoPath::new("src/core/calc.rs")?,
        span: Span::FIRST,
        function_name: Some("add".into()),
        mutator: "BinOp".into(),
        replacement: "-".into(),
    };
    let sum70 = MutationSummary {
        caught: 7,
        timeout: 0,
        missed: 3,
        unviable: 0,
        survivors: vec![surv],
    };
    match evaluate_mutation_summary(Some(&sum70), &mcfg, &rp) {
        Validation::Valid(()) => assert_eq!(70, 80),
        Validation::Invalid(d) => {
            assert_eq!(d.len(), 2);
            assert!(d.iter().any(|x| x.message.contains("zero-tolerance")));
        }
    }
    verify_mutation_zero_survivor_and_pass(&sum70, &mcfg, &rp);
    Ok(())
}

fn verify_mutation_zero_survivor_and_pass(
    sum70: &MutationSummary,
    mcfg: &MutationConfig,
    rp: &RepoPath,
) {
    let zs_only = MutationConfig {
        min_mutation_score: None,
        zero_survivor_globs: vec![String::from("src/core/**")],
        report_path: None,
    };
    assert!(!evaluate_mutation_summary(Some(sum70), &zs_only, rp).is_valid());
    let sum80 = MutationSummary {
        caught: 8,
        timeout: 0,
        missed: 2,
        unviable: 0,
        survivors: Vec::new(),
    };
    assert_eq!(
        evaluate_mutation_summary(Some(&sum80), mcfg, rp),
        Validation::Valid(())
    );
}

#[test]
fn tool_and_lang_rules_exact_boundaries() -> Res {
    let mut cfg = Config::strict_defaults();
    cfg.entrypoints = vec![String::from("src/cli.py")];
    let mut py_cli = UastDocument::new(RepoPath::new("src/cli.py")?, Language::Python, 10, 8)?;
    py_cli.lang_rule_findings = vec![
        LangRuleFinding::new(
            PythonViolation::LeftoverDebugArtifact(DebugArtifactKind::PrintCall),
            Span::FIRST,
            "print",
        ),
        LangRuleFinding::new(
            PythonViolation::UnsafeExecution(UnsafeExecKind::ShellSubprocess),
            Span::FIRST,
            "shell",
        ),
    ];
    py_cli.tool_config_findings.push(ToolConfigFinding {
        violation: ToolConfigViolation::RuffWeakened(RuffConfigIssue::StandaloneEmptySelect),
        span: Span::FIRST,
        message: "m".into(),
    });
    let cli_d = evaluate_document(&py_cli, &cfg);
    assert_eq!(cli_d.len(), 2);
    assert!(cli_d.iter().any(|d| d.message.contains("PY-S602")));

    let mut py_lib = UastDocument::new(RepoPath::new("src/lib.py")?, Language::Python, 10, 8)?;
    py_lib
        .lang_rule_findings
        .push(py_cli.lang_rule_findings[0].clone());
    assert_eq!(evaluate_document(&py_lib, &cfg).len(), 1);

    let ext = ExternalToolFinding {
        tool: ExternalToolId::Mypy,
        path: RepoPath::new("src/lib.py")?,
        line: 0,
        column: 0,
        native_code: String::new(),
        message: "bad".into(),
        remediation: "fix".into(),
    };
    assert_eq!(convert_external_tool_findings(&[ext])[0].rule_id, "GF082");
    verify_py_coverage_floor_and_import_resolver()
}

fn verify_py_coverage_floor_and_import_resolver() -> Res {
    let mut cfg = Config::strict_defaults();
    cfg.coverage.min_line_coverage = Some(85);
    for (pct, expected_len) in [(84_u32, 1_usize), (85, 0), (86, 0)] {
        let mut doc = UastDocument::new(RepoPath::new("pyproject.toml")?, Language::Toml, 4, 4)?;
        doc.coverage_fail_under = Some(CoverageFailUnder {
            floor_pct: pct,
            span: Span::FIRST,
        });
        assert_eq!(evaluate_document(&doc, &cfg).len(), expected_len);
    }
    Ok(())
}
