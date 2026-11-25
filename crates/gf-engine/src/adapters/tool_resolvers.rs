//! Direct `std::process::Command` binary resolvers (`uv`/`.venv`, `node_modules/.bin`, `PATH`, `pwsh`)
//! with zero `sh -c` shell interpolation (`GF082`).

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use gf_core::{Language, RepoPath, UastDocument, is_audited_tool_config_file};

use crate::types::EngineError;

/// Resolved execution strategy for an external CLI tool without invoking `sh -c`.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ResolvedPythonTool {
    /// Direct path to `.venv`/`node_modules/.bin` binary or binary name resolved on `PATH`.
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

/// Checks whether `bin_name` is executable on `PATH` via `<bin_name> --version`.
#[must_use]
pub fn resolve_system_binary(bin_name: &str) -> Option<PathBuf> {
    let ok = Command::new(bin_name)
        .arg("--version")
        .output()
        .is_ok_and(|o| o.status.success());
    ok.then(|| PathBuf::from(bin_name))
}

/// Resolves `pwsh` (or `powershell` on Windows) when `PSScriptAnalyzer` is available.
#[must_use]
pub fn resolve_pwsh_analyzer() -> Option<PathBuf> {
    for candidate in ["pwsh", "powershell"] {
        let ok = Command::new(candidate)
            .args([
                "-NoLogo",
                "-NoProfile",
                "-NonInteractive",
                "-Command",
                "if ($null -eq (Get-Command Invoke-ScriptAnalyzer -ErrorAction SilentlyContinue)) { exit 1 }",
            ])
            .output()
            .is_ok_and(|o| o.status.success());
        if ok {
            return Some(PathBuf::from(candidate));
        }
    }
    None
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
    if let Some(sys_bin) = resolve_system_binary(tool_name) {
        return Some(ResolvedPythonTool::Direct(sys_bin));
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

pub fn is_python_config_name(name: &str) -> bool {
    matches!(name, "pyproject.toml" | "ruff.toml" | ".ruff.toml")
}

pub fn is_ts_config_name(name: &str) -> bool {
    gf_core::is_tsconfig_filename(name)
        || matches!(
            name,
            "package.json" | "biome.json" | "biome.jsonc" | ".oxlintrc.json"
        )
}

pub fn is_shellcheck_config_name(name: &str) -> bool {
    matches!(name, ".shellcheckrc" | "shellcheckrc")
}

pub fn is_hadolint_config_name(name: &str) -> bool {
    matches!(
        name,
        ".hadolint.yaml" | ".hadolint.yml" | "hadolint.yaml" | "hadolint.yml"
    )
}

pub fn is_pssa_config_name(name: &str) -> bool {
    name.eq_ignore_ascii_case("psscriptanalyzersettings.psd1")
}

pub fn resolve_staged_targets(
    staged_filter: Option<&BTreeSet<RepoPath>>,
    exts: &[&str],
    is_cfg_fn: fn(&str) -> bool,
) -> Option<Vec<String>> {
    let Some(staged) = staged_filter else {
        return Some(Vec::new());
    };
    let mut matched_files = Vec::new();
    let mut config_changed = false;
    for path in staged {
        if path
            .extension()
            .is_some_and(|ext| exts.iter().any(|cand| cand.eq_ignore_ascii_case(ext)))
        {
            matched_files.push(String::from(path.as_str()));
        } else if is_cfg_fn(path.file_name()) {
            config_changed = true;
        }
    }
    if matched_files.is_empty() && !config_changed {
        None
    } else if config_changed {
        Some(Vec::new())
    } else {
        Some(matched_files)
    }
}

pub fn resolve_matching_doc_targets(
    docs: &[UastDocument],
    staged_filter: Option<&BTreeSet<RepoPath>>,
    doc_pred: impl Fn(&UastDocument) -> bool,
    is_cfg_fn: fn(&str) -> bool,
) -> Option<Vec<String>> {
    let all_matching: Vec<String> = docs
        .iter()
        .filter(|d| doc_pred(d))
        .map(|d| String::from(d.path.as_str()))
        .collect();
    if all_matching.is_empty() {
        return None;
    }
    let Some(staged) = staged_filter else {
        return Some(all_matching);
    };
    if staged.iter().any(|p| is_cfg_fn(p.file_name())) {
        return Some(all_matching);
    }
    let staged_matching: Vec<String> = all_matching
        .into_iter()
        .filter(|p| RepoPath::new(p).is_ok_and(|rp| staged.contains(&rp)))
        .collect();
    (!staged_matching.is_empty()).then_some(staged_matching)
}

pub fn resolve_explicit_doc_targets(
    docs: &[UastDocument],
    staged_filter: Option<&BTreeSet<RepoPath>>,
    target_lang: Language,
    is_cfg_fn: fn(&str) -> bool,
) -> Option<Vec<String>> {
    resolve_matching_doc_targets(
        docs,
        staged_filter,
        |d| d.language == target_lang && !is_audited_tool_config_file(&d.path),
        is_cfg_fn,
    )
}
