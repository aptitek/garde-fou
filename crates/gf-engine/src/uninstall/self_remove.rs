//! Safe self-binary uninstallation (`garde uninstall --self`) with `mise` detection,
//! Windows NTFS active-binary rename handling, and secondary `PATH` binary scanning.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use super::types::{UninstallAction, UninstallExecReport, UninstallPlan};
use crate::types::EngineError;

/// Infers the `mise` tool identifier when `exe` resides inside a `mise/installs/<tool>/...` path.
#[must_use]
pub fn detect_mise_tool_slug(exe: &Path) -> Option<String> {
    let parts: Vec<String> = exe
        .components()
        .map(|c| c.as_os_str().to_string_lossy().into_owned())
        .collect();
    let idx = parts
        .windows(2)
        .position(|w| w[0].eq_ignore_ascii_case("mise") && w[1].eq_ignore_ascii_case("installs"))?;
    let raw_tool = parts.get(idx + 2)?;
    if let Some(rest) = raw_tool.strip_prefix("github-")
        && let Some((owner, repo)) = rest.split_once('-')
    {
        return Some(format!("github:{owner}/{repo}"));
    }
    if let Some(rest) = raw_tool.strip_prefix("cargo-") {
        return Some(format!("cargo:{rest}"));
    }
    Some(raw_tool.clone())
}

fn canonical_or_owned(path: &Path) -> PathBuf {
    let Ok(canon) = fs::canonicalize(path) else {
        return path.to_path_buf();
    };
    canon
}

/// Scans `path_env` for any other `garde` / `garde.exe` binaries distinct from `current_exe`.
#[must_use]
pub fn find_secondary_garde_binaries(current_exe: &Path, path_env: Option<&str>) -> Vec<PathBuf> {
    let Some(raw_path) = path_env else {
        return Vec::new();
    };
    let canonical_self = canonical_or_owned(current_exe);
    let bin_name = if cfg!(windows) { "garde.exe" } else { "garde" };
    let mut found = Vec::new();

    for dir in std::env::split_paths(raw_path) {
        let candidate = dir.join(bin_name);
        if !candidate.is_file() {
            continue;
        }
        let canon = canonical_or_owned(&candidate);
        if canon != canonical_self && !found.contains(&candidate) {
            found.push(candidate);
        }
    }
    found
}

/// Builds an inspectable [`UninstallPlan`] for removing the installed `garde` binary itself.
#[must_use]
pub fn plan_self_uninstall(current_exe: &Path, path_env: Option<&str>) -> UninstallPlan {
    let mut plan = UninstallPlan::default();
    if let Some(slug) = detect_mise_tool_slug(current_exe) {
        plan.actions.push(UninstallAction::MiseUninstall(slug));
    } else {
        plan.actions
            .push(UninstallAction::RemoveSelfBinary(current_exe.to_path_buf()));
    }
    for secondary in find_secondary_garde_binaries(current_exe, path_env) {
        plan.warnings.push(format!(
            "Un autre binaire `garde` subsiste dans PATH : {}",
            secondary.display()
        ));
        plan.actions
            .push(UninstallAction::NoteSecondaryBinary(secondary));
    }
    plan
}

fn remove_binary_path(exe: &Path) -> Result<(), EngineError> {
    if !exe.exists() {
        return Ok(());
    }
    #[cfg(windows)]
    {
        let backup = exe.with_extension("exe.old");
        fs::rename(exe, &backup)?;
        fs::remove_file(&backup).ok();
    }
    #[cfg(not(windows))]
    {
        fs::remove_file(exe)?;
    }
    Ok(())
}

fn run_mise_uninstall(slug: &str) -> Result<(), EngineError> {
    let status = Command::new("mise").args(["uninstall", slug]).status()?;
    if !status.success() {
        return Err(EngineError::InvalidConfig(format!(
            "mise uninstall `{slug}` exited with status {status}"
        )));
    }
    let _reshim = Command::new("mise").arg("reshim").status()?;
    Ok(())
}

/// Executes self-uninstallation (`mise uninstall` + `mise reshim`, or direct binary removal).
///
/// # Errors
///
/// Returns [`EngineError`] if invoking `mise` or removing the executable fails.
pub fn execute_self_uninstall(
    current_exe: &Path,
    path_env: Option<&str>,
) -> Result<UninstallExecReport, EngineError> {
    let plan = plan_self_uninstall(current_exe, path_env);
    let mut applied = Vec::with_capacity(plan.actions.len());
    for action in plan.actions {
        match &action {
            UninstallAction::MiseUninstall(slug) => run_mise_uninstall(slug)?,
            UninstallAction::RemoveSelfBinary(path) => remove_binary_path(path)?,
            _ => {}
        }
        applied.push(action);
    }
    Ok(UninstallExecReport {
        applied,
        warnings: plan.warnings,
    })
}
