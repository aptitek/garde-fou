//! Pure output formatters (`human`, `agent` Markdown, and `SARIF` 2.1.0).

use core::fmt::Write as _;

use gf_core::Diagnostic;
use gf_engine::{CheckReport, InitOutcome, ScanScope};

use crate::OutputFormat;

const fn scope_label(scope: ScanScope) -> &'static str {
    match scope {
        ScanScope::All => "workspace",
        ScanScope::StagedOnly => "staged",
    }
}

fn render_human(report: &CheckReport) -> String {
    if report.is_clean() {
        return format!(
            "garde-fou: ok (scope={}, files={}, blake3={})",
            scope_label(report.scope),
            report.files_checked,
            &report.cache_fingerprint[..12.min(report.cache_fingerprint.len())]
        );
    }
    let mut buf = String::new();
    for diag in &report.diagnostics {
        let _ = writeln!(
            buf,
            "[{}] {}:{}-{}: {}",
            diag.rule_id,
            diag.path,
            diag.span.start_line(),
            diag.span.end_line(),
            diag.message
        );
    }
    buf.trim_end().to_string()
}

fn render_agent_markdown(report: &CheckReport) -> String {
    let status = if report.is_clean() { "PASS" } else { "FAIL" };
    let mut buf = format!(
        "## Garde-fou Report\n\n- **Status**: `{status}`\n- **Scope**: `{}`\n- **Files Checked**: `{}`\n- **Violations**: `{}`\n",
        scope_label(report.scope),
        report.files_checked,
        report.diagnostics.len()
    );
    if !report.diagnostics.is_empty() {
        buf.push_str("\n### Violations\n\n");
        for diag in &report.diagnostics {
            let _ = writeln!(
                buf,
                "- `{}` at `{}:{}` — {}",
                diag.rule_id,
                diag.path,
                diag.span.start_line(),
                diag.message
            );
        }
    }
    buf.trim_end().to_string()
}

fn escape_json(input: &str) -> String {
    input
        .replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('\n', "\\n")
}

fn render_sarif_result(diag: &Diagnostic) -> String {
    format!(
        r#"{{"ruleId":"{}","level":"error","message":{{"text":"{}"}},"locations":[{{"physicalLocation":{{"artifactLocation":{{"uri":"{}"}},"region":{{"startLine":{},"endLine":{}}}}}}}]}}"#,
        escape_json(&diag.rule_id),
        escape_json(&diag.message),
        escape_json(diag.path.as_str()),
        diag.span.start_line(),
        diag.span.end_line()
    )
}

fn render_sarif(report: &CheckReport) -> String {
    let results: Vec<String> = report.diagnostics.iter().map(render_sarif_result).collect();
    format!(
        r#"{{"version":"2.1.0","$schema":"https://json.schemastore.org/sarif-2.1.0.json","runs":[{{"tool":{{"driver":{{"name":"garde-fou","version":"{}"}}}},"results":[{}]}} ]}}"#,
        env!("CARGO_PKG_VERSION"),
        results.join(",")
    )
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

    use gf_core::{Diagnostic, RepoPath, Severity, Span};
    use gf_engine::{CheckReport, InitOutcome, ScanScope};

    use super::{render_check_report, render_init_outcome};
    use crate::OutputFormat;

    #[test]
    fn renders_all_formats_for_clean_and_failing_reports() -> Result<(), Box<dyn std::error::Error>>
    {
        let clean = CheckReport {
            scope: ScanScope::All,
            files_checked: 3,
            cache_fingerprint: "0123456789abcdef0123456789abcdef".to_string(),
            diagnostics: Vec::new(),
        };
        assert!(render_check_report(&clean, OutputFormat::Human).contains("garde-fou: ok"));
        assert!(render_check_report(&clean, OutputFormat::Agent).contains("`PASS`"));
        assert!(render_check_report(&clean, OutputFormat::Sarif).contains(r#""version":"2.1.0""#));

        let failing = CheckReport {
            scope: ScanScope::StagedOnly,
            files_checked: 1,
            cache_fingerprint: "abcdef".to_string(),
            diagnostics: vec![Diagnostic {
                rule_id: "GF001".to_string(),
                severity: Severity::Error,
                message: "file exceeds \"250\" lines".to_string(),
                path: RepoPath::new("src/big.rs")?,
                span: Span::new(1, 251)?,
            }],
        };
        assert!(render_check_report(&failing, OutputFormat::Human).contains("[GF001]"));
        assert!(render_check_report(&failing, OutputFormat::Agent).contains("### Violations"));
        assert!(render_check_report(&failing, OutputFormat::Sarif).contains(r#""ruleId":"GF001""#));

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
