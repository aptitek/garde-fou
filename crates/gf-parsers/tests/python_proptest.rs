//! Stage 3 Python property-based test suite (`proptest`):
//! 1. Totality & anti-panic over arbitrary UTF-8, `0` / `u32::MAX` bounds, truncated JSON, and multibyte Unicode.
//! 2. `Span` covariance law (`+k` `# header` lines shift diagnostics by `+k`, preserving codes & `structural_hash`).
//! 3. Guaranteed `pyproject.toml` detection law (`0` findings when strict, `>= 1` on any weakening mutation).

use std::path::Path;

use gf_core::{Config, Language, RepoPath};
use gf_parsers::{
    langs::tool_config::extract_tool_config, parse_mypy_ndjson, parse_pyright_json,
    parse_ruff_json, parse_source,
};
use gf_rules::evaluate_document;
use proptest::prelude::*;
use proptest::test_runner::{Config as ProptestConfig, TestRunner};

fn runner(cases: u32) -> TestRunner {
    TestRunner::new(ProptestConfig {
        cases,
        failure_persistence: None,
        ..ProptestConfig::default()
    })
}

const SAMPLE_PYTHON_CORPUS: &[&str] = &[
    "def bad_default(items: list[int] = []) -> list[int]:\n    return items\n",
    "from typing import Any\n\ndef leak(x: Any, y) -> int:\n    print(x)\n    return 1\n",
    "import subprocess\n\ndef exec_cmd(cmd: str) -> None:\n    subprocess.run(cmd, shell=True)\n    try:\n        eval(cmd)\n    except:\n        pass\n",
    "def clean_add(a: int, b: int) -> int:\n    total = a + b\n    return total * 2\n",
];

#[derive(Clone, Copy, Debug)]
enum PyprojectMutation {
    OmitStrict,
    StrictFalse,
    IgnoreErrorsTrue,
    DisallowUntypedFalse,
    WarnReturnAnyFalse,
    ProdOverrideIgnoreErrors,
    RuffIgnoreAll,
    RuffExtendIgnoreAll,
    RuffEmptySelect,
    PytestNoCov,
    CoverageBelowMin,
}

const ALL_MUTATIONS: &[PyprojectMutation] = &[
    PyprojectMutation::OmitStrict,
    PyprojectMutation::StrictFalse,
    PyprojectMutation::IgnoreErrorsTrue,
    PyprojectMutation::DisallowUntypedFalse,
    PyprojectMutation::WarnReturnAnyFalse,
    PyprojectMutation::ProdOverrideIgnoreErrors,
    PyprojectMutation::RuffIgnoreAll,
    PyprojectMutation::RuffExtendIgnoreAll,
    PyprojectMutation::RuffEmptySelect,
    PyprojectMutation::PytestNoCov,
    PyprojectMutation::CoverageBelowMin,
];

fn render_mypy_block(mutation: Option<PyprojectMutation>) -> String {
    let mypy_strict = match mutation {
        Some(PyprojectMutation::OmitStrict) => "",
        Some(PyprojectMutation::StrictFalse) => "strict = false\n",
        _ => "strict = true\n",
    };
    let mypy_extra = match mutation {
        Some(PyprojectMutation::IgnoreErrorsTrue) => "ignore_errors = true\n",
        Some(PyprojectMutation::DisallowUntypedFalse) => "disallow_untyped_defs = false\n",
        Some(PyprojectMutation::WarnReturnAnyFalse) => "warn_return_any = false\n",
        Some(PyprojectMutation::ProdOverrideIgnoreErrors) => {
            "\n[[tool.mypy.overrides]]\nmodule = \"app.*\"\nignore_errors = true\n"
        }
        _ => "",
    };
    format!("[tool.mypy]\npython_version = \"3.12\"\n{mypy_strict}{mypy_extra}")
}

fn render_pyproject(
    pkg: &str,
    line_len: u32,
    fail_under: u32,
    mutation: Option<PyprojectMutation>,
) -> String {
    let ruff_body = match mutation {
        Some(PyprojectMutation::RuffIgnoreAll) => "select = [\"E\"]\nignore = [\"ALL\"]\n",
        Some(PyprojectMutation::RuffExtendIgnoreAll) => {
            "select = [\"E\"]\nextend-ignore = [\"ALL\"]\n"
        }
        Some(PyprojectMutation::RuffEmptySelect) => "select = []\n",
        _ => "select = [\"E\", \"F\", \"B\", \"UP\"]\n",
    };
    let addopts = if matches!(mutation, Some(PyprojectMutation::PytestNoCov)) {
        "[\"--strict-markers\", \"--no-cov\"]"
    } else {
        "[\"--strict-markers\"]"
    };
    let eff_cov = if matches!(mutation, Some(PyprojectMutation::CoverageBelowMin)) {
        60
    } else {
        fail_under
    };
    let mypy_block = render_mypy_block(mutation);
    format!(
        "[project]\nname = \"{pkg}\"\nversion = \"0.1.0\"\n\n\
         [tool.ruff]\nline-length = {line_len}\n\n\
         [tool.ruff.lint]\n{ruff_body}\n\
         {mypy_block}\n\
         [tool.pytest.ini_options]\naddopts = {addopts}\n\n\
         [tool.coverage.report]\nfail_under = {eff_cov}\n"
    )
}

#[test]
fn extreme_line_bounds_unicode_and_truncated_json_are_total()
-> Result<(), Box<dyn std::error::Error>> {
    let root = Path::new("/workspace/repo");
    let ruff = format!(
        "[{{\"code\":\"E1\",\"message\":\"é\",\"filename\":\"src/é.py\",\"location\":{{\"row\":0,\"column\":{}}},\"fix\":null}}]",
        u32::MAX
    );
    let mypy = format!(
        "{{\"file\":\"src/é.py\",\"line\":0,\"column\":{},\"message\":\"λ\",\"hint\":\"h\",\"code\":\"c\",\"severity\":\"error\"}}",
        u32::MAX
    );
    let pyright = format!(
        "{{\"generalDiagnostics\":[{{\"file\":\"src/é.py\",\"severity\":\"error\",\"message\":\"Ω\",\"rule\":\"r\",\"range\":{{\"start\":{{\"line\":{},\"character\":0}}}}}}]}}",
        u32::MAX
    );
    assert_eq!(parse_ruff_json(&ruff, root)?[0].line, 1);
    assert_eq!(parse_mypy_ndjson(&mypy, root)?[0].line, 1);
    assert_eq!(parse_pyright_json(&pyright, root)?[0].line, u32::MAX);
    assert!(parse_ruff_json("[{\"code\":", root).is_err());
    assert!(parse_mypy_ndjson("{\"file\":", root).is_err());
    assert!(parse_pyright_json("{\"generalDiagnostics\":[{", root).is_err());
    Ok(())
}

#[test]
fn totality_arbitrary_utf8_never_panics_python_and_tool_parsers() -> Result<(), String> {
    let mut tr = runner(96);
    let root = Path::new("/workspace/repo");
    let targets = prop::sample::select(vec!["src/fuzz.py", "pyproject.toml", "ruff.toml"]);

    tr.run(&(".*", targets), |(raw, target)| {
        let path = RepoPath::new(target).map_err(|e| TestCaseError::fail(e.to_string()))?;
        let doc = if target == "src/fuzz.py" {
            parse_source(&path, &raw)
        } else {
            extract_tool_config(&path, Language::Toml, &raw)
        }
        .map_err(|e| TestCaseError::fail(e.to_string()))?;
        prop_assert!(doc.effective_lines <= doc.total_lines);
        prop_assert!(parse_ruff_json(&raw, root).is_ok() || parse_ruff_json(&raw, root).is_err());
        prop_assert!(
            parse_mypy_ndjson(&raw, root).is_ok() || parse_mypy_ndjson(&raw, root).is_err()
        );
        prop_assert!(
            parse_pyright_json(&raw, root).is_ok() || parse_pyright_json(&raw, root).is_err()
        );
        Ok(())
    })
    .map_err(|e| e.to_string())
}

#[test]
fn span_covariance_law_shifts_lines_by_k_preserving_codes_and_hashes() -> Result<(), String> {
    let mut tr = runner(64);
    let cfg = Config::strict_defaults();

    tr.run(
        &(0usize..SAMPLE_PYTHON_CORPUS.len(), 1u32..25u32),
        |(idx, k)| {
            let base_src = SAMPLE_PYTHON_CORPUS[idx];
            let prefix = "# header comment\n".repeat(usize::try_from(k).unwrap_or(1));
            let shifted_src = format!("{prefix}{base_src}");
            let path =
                RepoPath::new("src/cov.py").map_err(|e| TestCaseError::fail(e.to_string()))?;

            let base_doc =
                parse_source(&path, base_src).map_err(|e| TestCaseError::fail(e.to_string()))?;
            let shifted_doc = parse_source(&path, &shifted_src)
                .map_err(|e| TestCaseError::fail(e.to_string()))?;

            let b_hash: Vec<Option<u128>> = base_doc
                .callables
                .iter()
                .map(|c| c.structural_hash)
                .collect();
            let s_hash: Vec<Option<u128>> = shifted_doc
                .callables
                .iter()
                .map(|c| c.structural_hash)
                .collect();
            prop_assert_eq!(b_hash, s_hash);

            let base_diags = evaluate_document(&base_doc, &cfg);
            let shifted_diags = evaluate_document(&shifted_doc, &cfg);
            prop_assert_eq!(base_diags.len(), shifted_diags.len());
            for (b, s) in base_diags.iter().zip(shifted_diags.iter()) {
                prop_assert_eq!((&b.rule_id, &b.message), (&s.rule_id, &s.message));
                prop_assert_eq!(b.span.start_line() + k, s.span.start_line());
                prop_assert_eq!(b.span.end_line() + k, s.span.end_line());
            }
            Ok(())
        },
    )
    .map_err(|e| e.to_string())
}

#[test]
fn pyproject_strict_vs_weakened_guaranteed_detection_law() -> Result<(), String> {
    let mut tr = runner(96);
    let mut cfg = Config::strict_defaults();
    cfg.coverage.min_line_coverage = Some(85);
    let strat = (
        "[a-z][a-z0-9_-]{2,12}",
        79u32..120u32,
        85u32..100u32,
        0usize..ALL_MUTATIONS.len(),
    );

    tr.run(&strat, |(pkg, line_len, fail_under, mut_idx)| {
        let path =
            RepoPath::new("pyproject.toml").map_err(|e| TestCaseError::fail(e.to_string()))?;
        let clean = parse_source(&path, &render_pyproject(&pkg, line_len, fail_under, None))
            .map_err(|e| TestCaseError::fail(e.to_string()))?;
        prop_assert_eq!(evaluate_document(&clean, &cfg), Vec::new());

        let weak = parse_source(
            &path,
            &render_pyproject(&pkg, line_len, fail_under, Some(ALL_MUTATIONS[mut_idx])),
        )
        .map_err(|e| TestCaseError::fail(e.to_string()))?;
        let diags = evaluate_document(&weak, &cfg);
        prop_assert!(!diags.is_empty() && diags.iter().all(|d| d.rule_id == "GF081"));
        Ok(())
    })
    .map_err(|e| e.to_string())
}
