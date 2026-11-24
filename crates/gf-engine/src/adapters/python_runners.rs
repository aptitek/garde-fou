//! Declarative `ExternalAdapter` sum type and `uv` / `.venv` Python tool resolver (`GF082`).

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use gf_core::{Config, ExternalToolFinding, ExternalToolKind, ScannerPolicy};
use gf_parsers::{ToolParseError, parse_mypy_ndjson, parse_pyright_json, parse_ruff_json};

use crate::types::EngineError;

/// Closed sum type of external ecosystem verifiers orchestrated without `dyn Trait` (`GF082`).
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum ExternalAdapter {
    /// Astral `ruff check --output-format json --no-fix`.
    Ruff,
    /// `mypy --strict --output json --no-error-summary`.
    Mypy,
    /// `pyright --outputjson`.
    Pyright,
}

struct AdapterTableEntry {
    tool: ExternalToolKind,
    prefix_args: &'static [&'static str],
    install_hint: &'static str,
}

const ADAPTER_TABLE: [AdapterTableEntry; 3] = [
    AdapterTableEntry {
        tool: ExternalToolKind::Ruff,
        prefix_args: &["check", "--output-format", "json", "--no-fix", "--"],
        install_hint: "Install `ruff` in `.venv` (`uv add --dev ruff`) or system PATH.",
    },
    AdapterTableEntry {
        tool: ExternalToolKind::Mypy,
        prefix_args: &["--strict", "--output", "json", "--no-error-summary", "--"],
        install_hint: "Install `mypy` in `.venv` (`uv add --dev mypy`) or system PATH.",
    },
    AdapterTableEntry {
        tool: ExternalToolKind::Pyright,
        prefix_args: &["--outputjson"],
        install_hint: "Install `pyright` in `.venv` (`uv add --dev pyright`) or system PATH.",
    },
];

impl ExternalAdapter {
    /// All active Python external adapters in deterministic execution order.
    pub const PYTHON_ALL: [Self; 3] = [Self::Ruff, Self::Mypy, Self::Pyright];

    const fn spec(self) -> &'static AdapterTableEntry {
        &ADAPTER_TABLE[self as usize]
    }

    /// Canonical [`ExternalToolKind`] corresponding to this adapter.
    #[must_use]
    pub const fn tool_kind(self) -> ExternalToolKind {
        self.spec().tool
    }

    /// Reads the configured [`ScannerPolicy`] for this adapter from `config`.
    #[must_use]
    pub const fn configured_policy(self, config: &Config) -> ScannerPolicy {
        match self {
            Self::Ruff => config.tools.python.ruff,
            Self::Mypy => config.tools.python.mypy,
            Self::Pyright => config.tools.python.pyright,
        }
    }

    /// Actionable install guidance emitted in `GF043` when required and absent.
    #[must_use]
    pub const fn install_hint(self) -> &'static str {
        self.spec().install_hint
    }

    /// Builds argv slice targeting `py_targets` (empty slice means full workspace `.`).
    #[must_use]
    pub fn build_args(self, py_targets: &[String]) -> Vec<String> {
        let mut args: Vec<String> = self
            .spec()
            .prefix_args
            .iter()
            .copied()
            .map(String::from)
            .collect();
        if py_targets.is_empty() {
            args.push(String::from("."));
        } else {
            args.extend(py_targets.iter().cloned());
        }
        args
    }

    /// Parses raw `stdout` into normalized [`ExternalToolFinding`]s.
    ///
    /// # Errors
    ///
    /// Returns [`ToolParseError`] when `stdout` is malformed JSON/NDJSON.
    pub fn parse_output(
        self,
        stdout: &str,
        root: &Path,
    ) -> Result<Vec<ExternalToolFinding>, ToolParseError> {
        match self {
            Self::Ruff => parse_ruff_json(stdout, root),
            Self::Mypy => parse_mypy_ndjson(stdout, root),
            Self::Pyright => parse_pyright_json(stdout, root),
        }
    }
}

/// Resolved execution strategy for a Python CLI tool without invoking `sh -c`.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ResolvedPythonTool {
    /// Direct path to `.venv` binary or binary name resolved on `PATH`.
    Direct(PathBuf),
    /// Executed via `<uv_bin> run --no-sync <tool> ...` against an existing `.venv`.
    UvNoSync {
        /// Path or binary name for `uv`.
        uv_bin: PathBuf,
        /// Target Python tool name (`ruff`, `mypy`, `pyright`).
        tool: String,
    },
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
            Self::UvNoSync { uv_bin, tool } => Command::new(uv_bin)
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

/// Resolves `tool_name` in `.venv`, then `PATH`, then `<uv_bin> run --no-sync` (when `.venv` exists).
#[must_use]
pub fn resolve_python_tool_with_uv(
    root: &Path,
    tool_name: &str,
    uv_bin: &Path,
) -> Option<ResolvedPythonTool> {
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
        && Command::new(uv_bin)
            .args(["run", "--no-sync", tool_name, "--version"])
            .current_dir(root)
            .output()
            .is_ok_and(|o| o.status.success())
    {
        return Some(ResolvedPythonTool::UvNoSync {
            uv_bin: uv_bin.to_path_buf(),
            tool: String::from(tool_name),
        });
    }
    None
}

/// Resolves `tool_name` in `.venv`, then `PATH`, then `uv run --no-sync` (when `.venv` exists).
#[must_use]
pub fn resolve_python_tool(root: &Path, tool_name: &str) -> Option<ResolvedPythonTool> {
    resolve_python_tool_with_uv(root, tool_name, Path::new("uv"))
}
