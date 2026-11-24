//! `uv`-aware Python ecosystem tool runners (`RuffAdapter`, `MypyAdapter`, `PyrightAdapter`).

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use gf_core::{Config, ExternalToolFinding, ExternalToolId, ScannerPolicy};

use super::parsers::{parse_mypy_ndjson, parse_pyright_json, parse_ruff_json};
use crate::types::EngineError;

/// Pluggable contract for external ecosystem tool verifiers (`GF082`).
pub trait LanguageToolAdapter: Sync {
    /// Canonical tool identifier.
    fn tool_id(&self) -> ExternalToolId;

    /// Returns the user-configured [`ScannerPolicy`] for this tool.
    fn configured_policy(&self, config: &Config) -> ScannerPolicy;

    /// Builds CLI arguments targeting `py_targets` (empty slice means full workspace `.`).
    fn build_args(&self, py_targets: &[String]) -> Vec<String>;

    /// Parses raw `stdout` into normalized [`ExternalToolFinding`]s.
    ///
    /// # Errors
    ///
    /// Returns a diagnostic parse error message if `stdout` is malformed.
    fn parse_output(&self, stdout: &str, root: &Path) -> Result<Vec<ExternalToolFinding>, String>;

    /// Actionable remediation hint when `policy == Required` and the binary is missing (`GF043`).
    fn install_hint(&self) -> &'static str;
}

/// Tier 3 adapter for Astral `ruff check --output-format json`.
#[derive(Clone, Copy, Debug, Default)]
pub struct RuffAdapter;

impl LanguageToolAdapter for RuffAdapter {
    fn tool_id(&self) -> ExternalToolId {
        ExternalToolId::Ruff
    }

    fn configured_policy(&self, config: &Config) -> ScannerPolicy {
        config.tools.python.ruff
    }

    fn build_args(&self, py_targets: &[String]) -> Vec<String> {
        let mut args = vec![
            String::from("check"),
            String::from("--output-format"),
            String::from("json"),
            String::from("--no-fix"),
            String::from("--"),
        ];
        append_targets_or_dot(&mut args, py_targets);
        args
    }

    fn parse_output(&self, stdout: &str, root: &Path) -> Result<Vec<ExternalToolFinding>, String> {
        parse_ruff_json(stdout, root)
    }

    fn install_hint(&self) -> &'static str {
        "Install `ruff` in `.venv` (`uv add --dev ruff`) or system PATH."
    }
}

/// Tier 3 adapter for `mypy --strict --output json`.
#[derive(Clone, Copy, Debug, Default)]
pub struct MypyAdapter;

impl LanguageToolAdapter for MypyAdapter {
    fn tool_id(&self) -> ExternalToolId {
        ExternalToolId::Mypy
    }

    fn configured_policy(&self, config: &Config) -> ScannerPolicy {
        config.tools.python.mypy
    }

    fn build_args(&self, py_targets: &[String]) -> Vec<String> {
        let mut args = vec![
            String::from("--strict"),
            String::from("--output"),
            String::from("json"),
            String::from("--no-error-summary"),
            String::from("--"),
        ];
        append_targets_or_dot(&mut args, py_targets);
        args
    }

    fn parse_output(&self, stdout: &str, root: &Path) -> Result<Vec<ExternalToolFinding>, String> {
        parse_mypy_ndjson(stdout, root)
    }

    fn install_hint(&self) -> &'static str {
        "Install `mypy` in `.venv` (`uv add --dev mypy`) or system PATH."
    }
}

/// Tier 3 adapter for `pyright --outputjson`.
#[derive(Clone, Copy, Debug, Default)]
pub struct PyrightAdapter;

impl LanguageToolAdapter for PyrightAdapter {
    fn tool_id(&self) -> ExternalToolId {
        ExternalToolId::Pyright
    }

    fn configured_policy(&self, config: &Config) -> ScannerPolicy {
        config.tools.python.pyright
    }

    fn build_args(&self, py_targets: &[String]) -> Vec<String> {
        let mut args = vec![String::from("--outputjson")];
        append_targets_or_dot(&mut args, py_targets);
        args
    }

    fn parse_output(&self, stdout: &str, root: &Path) -> Result<Vec<ExternalToolFinding>, String> {
        parse_pyright_json(stdout, root)
    }

    fn install_hint(&self) -> &'static str {
        "Install `pyright` in `.venv` (`uv add --dev pyright`) or system PATH."
    }
}

fn append_targets_or_dot(args: &mut Vec<String>, py_targets: &[String]) {
    if py_targets.is_empty() {
        args.push(String::from("."));
    } else {
        args.extend(py_targets.iter().cloned());
    }
}

/// Resolved execution strategy for a Python CLI tool without invoking `sh -c`.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ResolvedPythonTool {
    /// Direct path to `.venv` binary or binary name resolved on `PATH`.
    Direct(PathBuf),
    /// Executed via `uv run --no-sync <tool> ...` against an existing `.venv`.
    UvNoSync(String),
}

impl ResolvedPythonTool {
    /// Executes the resolved tool in `root` with `args`.
    ///
    /// # Errors
    ///
    /// Returns [`EngineError::Io`] if spawning the process fails.
    pub fn execute(&self, root: &Path, args: &[String]) -> Result<Output, EngineError> {
        let output = match self {
            Self::Direct(program) => Command::new(program)
                .args(args)
                .current_dir(root)
                .output()?,
            Self::UvNoSync(tool) => Command::new("uv")
                .args(["run", "--no-sync", tool.as_str()])
                .args(args)
                .current_dir(root)
                .output()?,
        };
        Ok(output)
    }
}

fn probe_venv_binary(root: &Path, tool_name: &str) -> Option<PathBuf> {
    let unix_bin = root.join(".venv").join("bin").join(tool_name);
    if unix_bin.is_file() {
        return Some(unix_bin);
    }
    let win_bin = root
        .join(".venv")
        .join("Scripts")
        .join(format!("{tool_name}.exe"));
    win_bin.is_file().then_some(win_bin)
}

/// Resolves `tool_name` in `.venv`, then `PATH`, then `uv run --no-sync` (when `.venv` exists).
#[must_use]
pub fn resolve_python_tool(root: &Path, tool_name: &str) -> Option<ResolvedPythonTool> {
    if let Some(venv_bin) = probe_venv_binary(root, tool_name) {
        return Some(ResolvedPythonTool::Direct(venv_bin));
    }
    let in_path = Command::new(tool_name)
        .arg("--version")
        .output()
        .is_ok_and(|o| o.status.success());
    if in_path {
        return Some(ResolvedPythonTool::Direct(PathBuf::from(tool_name)));
    }
    let has_uv_project = (root.join("uv.lock").is_file() || root.join("pyproject.toml").is_file())
        && root.join(".venv").is_dir();
    if has_uv_project
        && Command::new("uv")
            .args(["run", "--no-sync", tool_name, "--version"])
            .current_dir(root)
            .output()
            .is_ok_and(|o| o.status.success())
    {
        return Some(ResolvedPythonTool::UvNoSync(String::from(tool_name)));
    }
    None
}
