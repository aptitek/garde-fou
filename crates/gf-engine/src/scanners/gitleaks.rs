//! External `gitleaks` runner producing unified `GF041` (and `GF043` missing-tool) diagnostics.

use std::fs;
use std::io::ErrorKind;
use std::path::Path;
use std::process::{Command, Output};
use std::sync::atomic::{AtomicU64, Ordering};

use gf_core::{Diagnostic, RepoPath, ScannerPolicy, Severity, Span};

use super::sarif_ingest::{SarifMapping, ingest_sarif_json};
use crate::{EngineError, ScanScope};

static GITLEAKS_SEQ: AtomicU64 = AtomicU64::new(0);

/// Builds a blocking `GF043` diagnostic when a required external scanner binary is missing.
///
/// # Errors
///
/// Returns [`EngineError::InvalidPath`] only if the static fallback path is invalid.
pub fn missing_tool_diagnostic(
    tool_name: &str,
    install_hint: &str,
) -> Result<Diagnostic, EngineError> {
    Ok(Diagnostic {
        rule_id: String::from("GF043"),
        severity: Severity::Error,
        message: format!(
            "required security scanner `{tool_name}` is missing from PATH (policy = `required`)"
        ),
        remediation: String::from(install_hint),
        measured: 1,
        threshold: 0,
        path: RepoPath::new("garde-fou.toml")?,
        span: Span::FIRST,
    })
}

fn probe_gitleaks_installed() -> bool {
    Command::new("gitleaks")
        .arg("version")
        .output()
        .is_ok_and(|out| out.status.success())
}

fn spawn_gitleaks(root: &Path, scope: ScanScope, report_str: &str) -> std::io::Result<Output> {
    if scope == ScanScope::StagedOnly {
        Command::new("gitleaks")
            .args([
                "git",
                "--pre-commit",
                "--staged",
                "--redact",
                "--no-banner",
                "--report-format",
                "sarif",
                "--report-path",
                report_str,
            ])
            .current_dir(root)
            .output()
    } else {
        Command::new("gitleaks")
            .args([
                "dir",
                ".",
                "--redact",
                "--no-banner",
                "--report-format",
                "sarif",
                "--report-path",
                report_str,
            ])
            .current_dir(root)
            .output()
    }
}

/// Runs `gitleaks` according to `policy` and `scope`, returning `GF041` / `GF043` diagnostics.
///
/// # Errors
///
/// Returns [`EngineError`] if spawning `gitleaks` fails unexpectedly or its SARIF is corrupt.
pub fn run_gitleaks(
    root: &Path,
    scope: ScanScope,
    policy: ScannerPolicy,
) -> Result<Vec<Diagnostic>, EngineError> {
    if policy == ScannerPolicy::Off {
        return Ok(Vec::new());
    }
    if !probe_gitleaks_installed() {
        return if policy == ScannerPolicy::Required {
            Ok(vec![missing_tool_diagnostic(
                "gitleaks",
                "Install `gitleaks` via `mise install` (`task setup`) or set `[scanners] secrets = \"auto\"` only in untrusted sandboxes.",
            )?])
        } else {
            Ok(Vec::new())
        };
    }
    if scope == ScanScope::StagedOnly && !root.join(".git").exists() {
        return Ok(Vec::new());
    }

    let seq = GITLEAKS_SEQ.fetch_add(1, Ordering::Relaxed);
    let report_path =
        std::env::temp_dir().join(format!("gf-gitleaks-{}-{seq}.sarif", std::process::id()));
    let report_str = report_path.to_string_lossy().into_owned();

    match spawn_gitleaks(root, scope, &report_str) {
        Ok(out) => {
            let _ = out.status;
        }
        Err(err) if err.kind() == ErrorKind::NotFound => return Ok(Vec::new()),
        Err(err) => return Err(EngineError::Io(err)),
    }

    if !report_path.is_file() {
        return Ok(Vec::new());
    }
    let sarif_text = fs::read_to_string(&report_path)?;
    let _ = fs::remove_file(&report_path);
    if sarif_text.trim().is_empty() {
        return Ok(Vec::new());
    }

    ingest_sarif_json(
        &sarif_text,
        root,
        SarifMapping {
            target_rule_id: "GF041",
            fallback_path: "garde-fou.toml",
            remediation: "Revoke the leaked secret immediately, purge it from Git index/history, and load credentials via environment variables.",
        },
    )
}
