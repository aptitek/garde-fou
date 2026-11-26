//! Pure output formatters (`human` Lipgloss reporter, compact deterministic `agent` Markdown,
//! and `SARIF` 2.1.0).

use std::collections::BTreeMap;
use std::fmt::Write as _;

use gf_core::Diagnostic;
use gf_engine::{CheckReport, InitReport, ScanScope, SyncReport};
use serde_json::json;

use crate::args::OutputFormat;
use crate::ui::{
    ColorPolicy, HumanCheckMeta, SourceFileView, render_human_check, render_init_human,
    render_sync_human,
};

const fn scope_label(scope: ScanScope) -> &'static str {
    match scope {
        ScanScope::All => "workspace",
        ScanScope::StagedOnly => "staged",
    }
}

/// Renders the deterministic, ANSI-free `--format agent` Markdown report.
#[must_use]
pub fn render_agent_markdown(report: &CheckReport) -> String {
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

/// Formats a [`CheckReport`] with explicit source views and terminal styling metadata.
#[must_use]
pub fn render_check_report_rich(
    report: &CheckReport,
    format: OutputFormat,
    sources: &BTreeMap<String, SourceFileView>,
    meta: &HumanCheckMeta,
) -> String {
    match format {
        OutputFormat::Human => render_human_check(report, sources, meta),
        OutputFormat::Agent => render_agent_markdown(report),
        OutputFormat::Sarif => render_sarif(report),
    }
}

/// Formats a [`CheckReport`] according to `format` using plain monochrome defaults for `Human`.
#[must_use]
pub fn render_check_report(report: &CheckReport, format: OutputFormat) -> String {
    render_check_report_rich(report, format, &BTreeMap::new(), &HumanCheckMeta::default())
}

/// Formats an [`InitReport`] in plain monochrome mode.
#[must_use]
pub fn render_init_report(report: &InitReport) -> String {
    render_init_human(report, ColorPolicy::Plain)
}

/// Formats a [`SyncReport`] in plain monochrome mode.
#[must_use]
pub fn render_sync_report(report: &SyncReport) -> String {
    render_sync_human(report, ColorPolicy::Plain)
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use gf_core::{Diagnostic, ProjectContext, RepoPath, Severity, Span};
    use gf_engine::{CheckReport, InitReport, ScanScope};

    use super::{render_check_report, render_init_report};
    use crate::args::OutputFormat;

    #[test]
    fn renders_all_formats_with_context_metrics_and_remediation()
    -> Result<(), Box<dyn std::error::Error>> {
        let clean = CheckReport {
            scope: ScanScope::All,
            context: ProjectContext::default(),
            files_checked: 3,
            cache_hits: 2,
            cache_misses: 1,
            cache_fingerprint: String::from("0123456789abcdef0123456789abcdef"),
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
            cache_fingerprint: String::from("abcdef"),
            diagnostics: vec![Diagnostic {
                rule_id: String::from("GF004"),
                severity: Severity::Error,
                message: String::from("callable `f` has cognitive complexity 14 (max 10)"),
                remediation: String::from("Extract pure helper functions."),
                measured: 14,
                threshold: 10,
                path: RepoPath::new("src/big.rs")?,
                span: Span::new(5, 42)?,
            }],
        };
        assert!(render_check_report(&failing, OutputFormat::Agent).contains("`measured: 14`"));
        assert!(render_check_report(&failing, OutputFormat::Human).contains("[GF004]"));
        assert!(render_check_report(&failing, OutputFormat::Sarif).contains(r#""ruleId":"GF004""#));

        let init_rep = InitReport {
            root: PathBuf::from("/tmp/demo"),
            project_name: String::from("demo"),
            context: ProjectContext::default(),
            git_initialized: true,
            created_files: vec![String::from("garde-fou.toml")],
            updated_files: Vec::new(),
            preserved_files: Vec::new(),
            installed_skills: vec![String::from("garde-fou")],
            warnings: Vec::new(),
        };
        assert!(render_init_report(&init_rep).contains("initialized `demo`"));
        Ok(())
    }
}
