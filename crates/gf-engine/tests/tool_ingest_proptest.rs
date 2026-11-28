//! Property-based (`proptest`) and pure unit tests for Step 8 tool output & config parsers:
//! `parse_ruff_json`, `parse_mypy_ndjson`, `parse_pyright_json`, `strip_jsonc_to_json`,
//! `extract_tool_config`, and `convert_external_tool_findings`.

use std::path::Path;

use gf_core::{ExternalToolFinding, ExternalToolId, Language, RepoPath};
use gf_engine::{parse_mypy_ndjson, parse_pyright_json, parse_ruff_json};
use gf_parsers::langs::{jsonc::strip_jsonc_to_json, tool_config::extract_tool_config};
use gf_rules::convert_external_tool_findings;
use proptest::prelude::*;
use proptest::test_runner::{Config as ProptestConfig, TestRunner};

fn runner() -> TestRunner {
    TestRunner::new(ProptestConfig {
        cases: 96,
        failure_persistence: None,
        ..ProptestConfig::default()
    })
}

#[test]
fn tier3_pure_parsers_handle_windows_paths_hints_and_zero_indexed_pyright()
-> Result<(), Box<dyn std::error::Error>> {
    let win_root = Path::new("C:\\repo");
    let ruff_raw = r#"[{
        "code": "F401",
        "message": "`os` imported but unused",
        "filename": "C:\\repo\\src\\app.py",
        "location": { "row": 4, "column": 8 },
        "fix": { "message": "Remove unused import: `os`" }
    }]"#;
    let ruff = parse_ruff_json(ruff_raw, win_root)?;
    assert_eq!(ruff.len(), 1);
    assert_eq!(ruff[0].tool, ExternalToolId::Ruff);
    assert_eq!(ruff[0].path.as_str(), "src/app.py");
    assert_eq!((ruff[0].line, ruff[0].column), (4, 8));
    assert!(ruff[0].remediation.contains("Remove unused import"));

    let mypy_raw = "{\"file\":\"C:\\\\repo\\\\src\\\\app.py\",\"line\":12,\"column\":5,\"message\":\"Incompatible types\",\"hint\":\"Use int instead of str\",\"code\":\"assignment\",\"severity\":\"error\"}\n\
                    {\"file\":\"src/app.py\",\"line\":12,\"column\":5,\"message\":\"Revealed type\",\"hint\":null,\"code\":null,\"severity\":\"note\"}\n";
    let mypy = parse_mypy_ndjson(mypy_raw, win_root)?;
    assert_eq!(mypy.len(), 1);
    assert_eq!(mypy[0].path.as_str(), "src/app.py");
    assert!(mypy[0].remediation.contains("Use int instead of str"));

    let pyright_raw = r#"{
        "generalDiagnostics": [
            {
                "file": "C:\\repo\\src\\app.py",
                "severity": "error",
                "message": "Type `str` cannot be assigned to `int`",
                "rule": "reportAssignmentType",
                "range": { "start": { "line": 0, "character": 4 } }
            },
            {
                "file": "C:\\repo\\src\\app.py",
                "severity": "information",
                "message": "Info ignored"
            }
        ]
    }"#;
    let pyright = parse_pyright_json(pyright_raw, win_root)?;
    assert_eq!(pyright.len(), 1);
    assert_eq!((pyright[0].line, pyright[0].column), (1, 5));
    Ok(())
}

#[test]
fn arbitrary_utf8_never_panics_tool_output_and_config_parsers() -> Result<(), String> {
    let mut tr = runner();
    let root = Path::new("/workspace/project");
    let cfg_files = [
        ("pyproject.toml", Language::Toml),
        ("ruff.toml", Language::Toml),
        (".ruff.toml", Language::Toml),
        ("tsconfig.json", Language::Json),
    ];

    tr.run(&(0..cfg_files.len(), ".*"), |(idx, arbitrary)| {
        let ruff_res = parse_ruff_json(&arbitrary, root);
        let mypy_res = parse_mypy_ndjson(&arbitrary, root);
        let pyright_res = parse_pyright_json(&arbitrary, root);
        let stripped = strip_jsonc_to_json(&arbitrary);

        let (name, lang) = cfg_files[idx];
        let path = RepoPath::new(name).map_err(|e| TestCaseError::fail(e.to_string()))?;
        let doc = extract_tool_config(&path, lang, &arbitrary)
            .map_err(|e| TestCaseError::fail(e.to_string()))?;

        prop_assert!(doc.effective_lines <= doc.total_lines);
        prop_assert!(stripped.lines().count() <= arbitrary.lines().count().saturating_add(1));
        prop_assert!(ruff_res.is_ok() || ruff_res.is_err());
        prop_assert!(mypy_res.is_ok() || mypy_res.is_err());
        prop_assert!(pyright_res.is_ok() || pyright_res.is_err());
        Ok(())
    })
    .map_err(|e| e.to_string())
}

#[test]
fn convert_external_tool_findings_is_idempotent_sorted_and_one_based() -> Result<(), String> {
    let mut tr = runner();
    let strat = (
        ((0..ExternalToolId::ALL.len(), 0u32..500u32), 0u32..200u32),
        ("[a-zA-Z0-9_-]{0,16}", "[a-zA-Z0-9 _.-]{1,32}"),
    );

    tr.run(&prop::collection::vec(strat, 1..10), |items| {
        let mut findings = Vec::new();
        for (((tool_idx, line), col), (code, msg)) in items {
            let path =
                RepoPath::new("src/app.py").map_err(|e| TestCaseError::fail(e.to_string()))?;
            findings.push(ExternalToolFinding {
                tool: ExternalToolId::ALL[tool_idx],
                path,
                line,
                column: col,
                native_code: code,
                message: msg,
                remediation: String::from("Fix issue."),
            });
        }

        let mut doubled = findings.clone();
        doubled.extend(findings.iter().cloned());
        doubled.reverse();

        let once = convert_external_tool_findings(&findings);
        let twice = convert_external_tool_findings(&doubled);
        prop_assert_eq!(&once, &twice);
        for diag in &once {
            prop_assert_eq!(&diag.rule_id, "GF082");
            prop_assert!(diag.span.start_line() >= 1);
        }
        Ok(())
    })
    .map_err(|e| e.to_string())
}
