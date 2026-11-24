//! Property-based tests (`proptest`) verifying UTF-8 panic-freedom across Shell, PowerShell
//! (including trailing backticks `` ` `` and unclosed `<#` comments), `Dockerfile RUN` bridge,
//! Tier 2 configs, Tier 3 JSON parsers, and `+k` comment-line span covariance.

use std::path::Path;

use gf_core::{Language, RepoPath};
use gf_parsers::{
    parse_hadolint_json, parse_psscriptanalyzer_json, parse_shellcheck_json, parse_source,
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
fn arbitrary_utf8_never_panics_across_shell_powershell_docker_and_configs() -> Result<(), String> {
    let mut r = runner();
    let strategy = (
        "\\PC{0,220}",
        prop_oneof![
            Just(""),
            Just(" `"),
            Just("\n<# unclosed block comment"),
            Just("\n@\"\nunclosed here-string"),
            Just("\n<<EOF\nunterminated"),
        ],
    );
    r.run(&strategy, |(raw, trailing_quirk)| {
        let payload = format!("{raw}{trailing_quirk}");
        for rel in [
            "scripts/fuzz.sh",
            "scripts/fuzz.bash",
            "scripts/fuzz.ps1",
            "scripts/fuzz.psm1",
            "Dockerfile",
            ".shellcheckrc",
            ".hadolint.yaml",
            "PSScriptAnalyzerSettings.psd1",
        ] {
            let rp = RepoPath::new(rel).map_err(|e| TestCaseError::fail(e.to_string()))?;
            let doc =
                parse_source(&rp, &payload).map_err(|e| TestCaseError::fail(e.to_string()))?;
            prop_assert_eq!(doc.path.as_str(), rel);
        }
        let root = Path::new("/workspace");
        prop_assert!(
            parse_shellcheck_json(&payload, root).is_ok()
                || parse_shellcheck_json(&payload, root).is_err()
        );
        prop_assert!(
            parse_hadolint_json(&payload, root).is_ok()
                || parse_hadolint_json(&payload, root).is_err()
        );
        prop_assert!(
            parse_psscriptanalyzer_json(&payload, root).is_ok()
                || parse_psscriptanalyzer_json(&payload, root).is_err()
        );
        Ok(())
    })
    .map_err(|e| e.to_string())
}

#[test]
fn prepending_k_comment_lines_shifts_shell_ps_and_dockerfile_run_spans_by_k() -> Result<(), String>
{
    let mut r = runner();
    r.run(&(1_u32..18), |k| {
        let pad = "# comment line\n".repeat(usize::try_from(k).unwrap_or(1));
        let sh_src = format!("#!/bin/sh\nset -eu\n{pad}rm $UNQUOTED_TARGET\n");
        let sh_rp =
            RepoPath::new("scripts/cov.sh").map_err(|e| TestCaseError::fail(e.to_string()))?;
        let sh_doc =
            parse_source(&sh_rp, &sh_src).map_err(|e| TestCaseError::fail(e.to_string()))?;
        prop_assert_eq!(sh_doc.language, Language::Shell);
        prop_assert_eq!(sh_doc.lang_rule_findings.len(), 1);
        prop_assert_eq!(sh_doc.lang_rule_findings[0].span.start_line(), 3 + k);

        let ps_src = format!(
            "Set-StrictMode -Version Latest\n$ErrorActionPreference = 'Stop'\n{pad}if ($items -eq $null) {{ Write-Output 'x' }}\n"
        );
        let ps_rp =
            RepoPath::new("scripts/cov.ps1").map_err(|e| TestCaseError::fail(e.to_string()))?;
        let ps_doc =
            parse_source(&ps_rp, &ps_src).map_err(|e| TestCaseError::fail(e.to_string()))?;
        prop_assert_eq!(ps_doc.language, Language::PowerShell);
        prop_assert_eq!(ps_doc.lang_rule_findings.len(), 1);
        prop_assert_eq!(ps_doc.lang_rule_findings[0].span.start_line(), 3 + k);

        let df_src =
            format!("FROM debian:12.8-slim\n{pad}RUN echo \"ok\" && \\\n    [[ -f /etc/hosts ]]\n");
        let df_rp = RepoPath::new("Dockerfile").map_err(|e| TestCaseError::fail(e.to_string()))?;
        let df_doc =
            parse_source(&df_rp, &df_src).map_err(|e| TestCaseError::fail(e.to_string()))?;
        prop_assert_eq!(df_doc.lang_rule_findings.len(), 1);
        prop_assert_eq!(df_doc.lang_rule_findings[0].span.start_line(), 3 + k);
        Ok(())
    })
    .map_err(|e| e.to_string())
}
