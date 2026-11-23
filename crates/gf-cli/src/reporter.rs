//! Pure output formatters (`human`, compact deterministic `agent` Markdown, and `SARIF` 2.1.0).

use core::fmt::Write as _;

use gf_core::Diagnostic;
use gf_engine::{CheckReport, InitOutcome, ScanScope};
use serde_json::json;

use crate::OutputFormat;

const fn scope_label(scope: ScanScope) -> &'static str {
    match scope {
        ScanScope::All => "workspace",
        ScanScope::StagedOnly => "staged",
    }
}

fn render_human(report: &CheckReport) -> String {
    let ctx_summary = report.context.summary_inline();
    if report.is_clean() {
        let short_fp = report
            .cache_fingerprint
            .get(0..12)
            .unwrap_or(&report.cache_fingerprint);
        return format!(
            "garde-fou: ok (scope={}, {ctx_summary}, files={}, cache_hits={}, cache_misses={}, blake3={short_fp})",
            scope_label(report.scope),
            report.files_checked,
            report.cache_hits,
            report.cache_misses,
        );
    }
    let mut buf = format!("garde-fou: FAIL ({ctx_summary})\n");
    for diag in &report.diagnostics {
        let _ = writeln!(
            buf,
            "[{}] {}:{}-{} (measured={}, max={}): {}\n  help: {}",
            diag.rule_id,
            diag.path,
            diag.span.start_line(),
            diag.span.end_line(),
            diag.measured,
            diag.threshold,
            diag.message,
            diag.remediation
        );
    }
    buf.trim_end().to_string()
}

fn render_agent_markdown(report: &CheckReport) -> String {
    let status = if report.is_clean() { "PASS" } else { "FAIL" };
    let mut buf = format!(
        "## Garde-fou Report\n\n- **Status**: `{status}` | **Scope**: `{}` | **Files**: `{}` (`{}` hits, `{}` misses) | **Violations**: `{}`\n- **Context**: `{}`\n",
        scope_label(report.scope),
        report.files_checked,
        report.cache_hits,
        report.cache_misses,
        report.diagnostics.len(),
        report.context.summary_inline(),
    );
    if !report.diagnostics.is_empty() {
        buf.push_str("\n### Actionable Violations\n\n");
        for diag in &report.diagnostics {
            let _ = writeln!(
                buf,
                "- **[{}]** `{}:{}-{}` (`measured: {}`, `max: {}`) — {} | **Remediation**: {}",
                diag.rule_id,
                diag.path,
                diag.span.start_line(),
                diag.span.end_line(),
                diag.measured,
                diag.threshold,
                diag.message,
                diag.remediation
            );
        }
    }
    buf.trim_end().to_string()
}

fn sarif_result(diag: &Diagnostic) -> serde_json::Value {
    json!({
        "ruleId": diag.rule_id,
        "level": "error",
        "message": {
            "text": format!("{} Remediation: {}", diag.message, diag.remediation)
        },
        "properties": {
            "measured": diag.measured,
            "threshold": diag.threshold,
            "remediation": diag.remediation
        },
        "locations": [{
            "physicalLocation": {
                "artifactLocation": { "uri": diag.path.as_str() },
                "region": {
                    "startLine": diag.span.start_line(),
                    "endLine": diag.span.end_line()
                }
            }
        }]
    })
}

fn render_sarif(report: &CheckReport) -> String {
    let results: Vec<serde_json::Value> = report.diagnostics.iter().map(sarif_result).collect();
    let payload = json!({
        "version": "2.1.0",
        "$schema": "https://json.schemastore.org/sarif-2.1.0.json",
        "runs": [{
            "tool": {
                "driver": {
                    "name": "garde-fou",
                    "version": env!("CARGO_PKG_VERSION")
                }
            },
            "results": results
        }]
    });
    payload.to_string()
}

/// Formats a [`CheckReport`] according to the requested [`OutputFormat`].
#[must_use]
pub fn render_check_report(report: &CheckReport, format: OutputFormat) -> String {
    match format {
        OutputFormat::Human => render_human(report),
        OutputFormat::Agent => render_agent_markdown(report),
        OutputFormat::Sarif => render_sarif(report),
    }
}

/// Formats an [`InitOutcome`] for terminal display.
#[must_use]
pub fn render_init_outcome(outcome: &InitOutcome) -> String {
    match outcome {
        InitOutcome::Created(path) => format!("garde-fou: created {}", path.display()),
        InitOutcome::AlreadyPresent(path) => {
            format!("garde-fou: preserved existing {}", path.display())
        }
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use gf_core::{Diagnostic, ProjectContext, RepoPath, Severity, Span};
    use gf_engine::{CheckReport, InitOutcome, ScanScope};

    use super::{render_check_report, render_init_outcome};
    use crate::OutputFormat;

    #[test]
    fn renders_all_formats_with_context_metrics_and_remediation()
    -> Result<(), Box<dyn std::error::Error>> {
        let clean = CheckReport {
            scope: ScanScope::All,
            context: ProjectContext::default(),
            files_checked: 3,
            cache_hits: 2,
            cache_misses: 1,
            cache_fingerprint: "0123456789abcdef0123456789abcdef".to_string(),
            diagnostics: Vec::new(),
        };
        assert!(render_check_report(&clean, OutputFormat::Human).contains("langs=[]"));
        assert!(render_check_report(&clean, OutputFormat::Agent).contains("**Context**:"));
        assert!(render_check_report(&clean, OutputFormat::Sarif).contains(r#""version":"2.1.0""#));

        let failing = CheckReport {
            scope: ScanScope::StagedOnly,
            context: ProjectContext::default(),
            files_checked: 1,
            cache_hits: 0,
            cache_misses: 1,
            cache_fingerprint: "abcdef".to_string(),
            diagnostics: vec![Diagnostic {
                rule_id: "GF004".to_string(),
                severity: Severity::Error,
                message: "callable `f` has cognitive complexity 14 (max 10)".to_string(),
                remediation: "Extract pure helper functions.".to_string(),
                measured: 14,
                threshold: 10,
                path: RepoPath::new("src/big.rs")?,
                span: Span::new(5, 42)?,
            }],
        };
        let agent_out = render_check_report(&failing, OutputFormat::Agent);
        assert!(agent_out.contains("`measured: 14`, `max: 10`"));
        assert!(render_check_report(&failing, OutputFormat::Human).contains("[GF004]"));
        assert!(render_check_report(&failing, OutputFormat::Sarif).contains(r#""ruleId":"GF004""#));

        assert!(
            render_init_outcome(&InitOutcome::Created(PathBuf::from("garde-fou.toml")))
                .contains("created")
        );
        assert!(
            render_init_outcome(&InitOutcome::AlreadyPresent(PathBuf::from(
                "garde-fou.toml"
            )))
            .contains("preserved")
        );
        Ok(())
    }
}
