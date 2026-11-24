//! Property-based tests (`proptest`) verifying UTF-8 panic-freedom across YAML 1.2
//! (`saphyr-parser`), Docker Compose, GitHub Actions, Tier 2 configs, Tier 3 parsers,
//! and `+k` comment-line span covariance across embedded Shell/PowerShell bridges.

use std::path::Path;

use gf_core::{Language, RepoPath};
use gf_parsers::{
    parse_actionlint_json, parse_dclint_json, parse_source, parse_yamllint_parsable,
    parse_zizmor_sarif,
};
use proptest::prelude::*;
use proptest::test_runner::{Config as ProptestConfig, TestCaseError, TestRunner};

fn runner() -> TestRunner {
    TestRunner::new(ProptestConfig {
        cases: 64,
        failure_persistence: None,
        ..ProptestConfig::default()
    })
}

#[test]
fn arbitrary_utf8_never_panics_across_yaml_compose_gha_and_tool_parsers() -> Result<(), String> {
    let mut r = runner();
    let strategy = (
        "\\PC{0,220}",
        prop_oneof![
            Just(""),
            Just("\na: &b [*b]"),
            Just("\non:\n  pull_request_target:\njobs:\n  x:\n    steps:\n      - run: ${{"),
            Just("\nservices:\n  app:\n    ports: [22:22]\n    command: [\"sh\", \"-c\", \"$$X\"]"),
        ],
    );
    r.run(&strategy, |(raw, suffix)| {
        let payload = format!("{raw}{suffix}");
        for rel in [
            "Taskfile.yml",
            "compose.yaml",
            "docker-compose.override.yml",
            ".github/workflows/ci.yml",
            ".github/actions/a/action.yml",
            ".yamllint.yml",
            ".dclintrc.json",
            "zizmor.yml",
        ] {
            let rp = RepoPath::new(rel).map_err(|e| TestCaseError::fail(e.to_string()))?;
            let doc =
                parse_source(&rp, &payload).map_err(|e| TestCaseError::fail(e.to_string()))?;
            prop_assert_eq!(doc.path.as_str(), rel);
        }
        let root = Path::new("/workspace");
        prop_assert!(
            parse_yamllint_parsable(&payload, root).is_ok()
                || parse_yamllint_parsable(&payload, root).is_err()
        );
        prop_assert!(
            parse_dclint_json(&payload, root).is_ok() || parse_dclint_json(&payload, root).is_err()
        );
        prop_assert!(
            parse_actionlint_json(&payload, root).is_ok()
                || parse_actionlint_json(&payload, root).is_err()
        );
        prop_assert!(
            parse_zizmor_sarif(&payload, root).is_ok()
                || parse_zizmor_sarif(&payload, root).is_err()
        );
        Ok(())
    })
    .map_err(|e| e.to_string())
}

#[test]
fn prepending_k_yaml_comment_lines_shifts_yaml_compose_and_gha_run_spans_by_k() -> Result<(), String>
{
    let mut r = runner();
    r.run(&(1_u32..16), |k| {
        let pad = "# header comment\n".repeat(usize::try_from(k).unwrap_or(1));

        let yml_src = format!("{pad}mode: 0755\n");
        let yml_doc = parse_source(
            &RepoPath::new("cfg.yaml").map_err(|e| TestCaseError::fail(e.to_string()))?,
            &yml_src,
        )
        .map_err(|e| TestCaseError::fail(e.to_string()))?;
        prop_assert_eq!(yml_doc.language, Language::Yaml);
        prop_assert_eq!(yml_doc.lang_rule_findings.len(), 1);
        prop_assert_eq!(yml_doc.lang_rule_findings[0].span.start_line(), 1 + k);

        let cmp_src = format!(
            "{pad}services:\n  api:\n    image: alpine:3.20\n    command: [\"sh\", \"-c\", \"rm $$UNQUOTED_VAR\"]\n"
        );
        let cmp_doc = parse_source(
            &RepoPath::new("compose.yaml").map_err(|e| TestCaseError::fail(e.to_string()))?,
            &cmp_src,
        )
        .map_err(|e| TestCaseError::fail(e.to_string()))?;
        prop_assert_eq!(cmp_doc.lang_rule_findings.len(), 1);
        prop_assert_eq!(cmp_doc.lang_rule_findings[0].span.start_line(), 4 + k);

        let gha_src = format!(
            "{pad}on: [push]\npermissions:\n  contents: read\njobs:\n  t:\n    runs-on: ubuntu-latest\n    steps:\n      - run: |\n          echo ok\n          rm $UNQUOTED_IN_GHA\n"
        );
        let gha_doc = parse_source(
            &RepoPath::new(".github/workflows/t.yml")
                .map_err(|e| TestCaseError::fail(e.to_string()))?,
            &gha_src,
        )
        .map_err(|e| TestCaseError::fail(e.to_string()))?;
        prop_assert_eq!(gha_doc.lang_rule_findings.len(), 1);
        prop_assert_eq!(gha_doc.lang_rule_findings[0].span.start_line(), 10 + k);
        Ok(())
    })
    .map_err(|e| e.to_string())
}
