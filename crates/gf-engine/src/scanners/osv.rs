//! External `osv-scanner` runner producing unified `GF042` supply-chain diagnostics.

use std::path::Path;
use std::process::Command;

use gf_core::{Diagnostic, ScannerPolicy};

use super::gitleaks::missing_tool_diagnostic;
use super::sarif_ingest::{SarifMapping, ingest_sarif_json};
use crate::{EngineError, ScanScope};

const DEPENDENCY_MANIFESTS: &[&str] = &[
    "Cargo.toml",
    "Cargo.lock",
    "package.json",
    "pnpm-lock.yaml",
    "package-lock.json",
    "yarn.lock",
    "pyproject.toml",
    "uv.lock",
    "poetry.lock",
    "requirements.txt",
    "go.mod",
    "go.sum",
];

const SCANNABLE_LOCKFILES: &[&str] = &[
    "Cargo.lock",
    "pnpm-lock.yaml",
    "package-lock.json",
    "yarn.lock",
    "uv.lock",
    "poetry.lock",
    "requirements.txt",
    "go.sum",
];

fn probe_osv_installed() -> bool {
    Command::new("osv-scanner")
        .arg("--version")
        .output()
        .is_ok_and(|out| out.status.success())
}

fn has_staged_dependency_manifest(root: &Path) -> bool {
    if !root.join(".git").exists() {
        return false;
    }
    let Ok(out) = Command::new("git")
        .args(["diff", "--cached", "--name-only", "--diff-filter=ACMR"])
        .current_dir(root)
        .output()
    else {
        return false;
    };
    let stdout = String::from_utf8_lossy(&out.stdout);
    stdout.lines().any(|line| {
        let file_name = line.trim().rsplit('/').next().unwrap_or("");
        DEPENDENCY_MANIFESTS.contains(&file_name)
    })
}

fn has_any_lockfile(root: &Path) -> bool {
    SCANNABLE_LOCKFILES.iter().any(|lf| root.join(lf).is_file())
}

/// Runs `osv-scanner` according to `policy` and `scope`, returning `GF042` / `GF043` diagnostics.
///
/// # Errors
///
/// Returns [`EngineError`] on unexpected I/O failure or malformed SARIF output.
pub fn run_osv_scanner(
    root: &Path,
    scope: ScanScope,
    policy: ScannerPolicy,
) -> Result<Vec<Diagnostic>, EngineError> {
    if policy == ScannerPolicy::Off {
        return Ok(Vec::new());
    }
    if !probe_osv_installed() {
        return if policy == ScannerPolicy::Required {
            Ok(vec![missing_tool_diagnostic(
                "osv-scanner",
                "Install `osv-scanner` via `mise install` (`task setup`) or run inside the configured toolchain environment.",
            )?])
        } else {
            Ok(Vec::new())
        };
    }

    if scope == ScanScope::StagedOnly && !has_staged_dependency_manifest(root) {
        return Ok(Vec::new());
    }
    if !has_any_lockfile(root) {
        return Ok(Vec::new());
    }

    let output = Command::new("osv-scanner")
        .args(["scan", "source", "--format", "sarif", "-r", "."])
        .current_dir(root)
        .output()?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    let trimmed = stdout.trim();
    if trimmed.is_empty() || !trimmed.starts_with('{') {
        return Ok(Vec::new());
    }

    ingest_sarif_json(
        trimmed,
        root,
        SarifMapping {
            target_rule_id: "GF042",
            fallback_path: "Cargo.lock",
            remediation: "Upgrade the vulnerable dependency to a patched version in your manifest/lockfile and re-verify with `garde-fou check`.",
        },
    )
}
