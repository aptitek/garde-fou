//! Tier 3 language ecosystem tool ingestion orchestrator (`GF082` & `GF043`).

pub mod parsers;
pub mod python_runners;

use std::collections::BTreeSet;
use std::path::Path;

use gf_core::{Config, Diagnostic, LanguageId, ProjectContext, RepoPath, ScannerPolicy};
use gf_rules::convert_external_tool_findings;
use rayon::prelude::*;

pub use parsers::{parse_mypy_ndjson, parse_pyright_json, parse_ruff_json};
pub use python_runners::{
    LanguageToolAdapter, MypyAdapter, PyrightAdapter, ResolvedPythonTool, RuffAdapter,
    resolve_python_tool,
};

use crate::scanners::gitleaks::missing_tool_diagnostic;
use crate::types::{CheckOptions, EngineError};

const PYTHON_ADAPTERS: [&dyn LanguageToolAdapter; 3] =
    [&RuffAdapter, &MypyAdapter, &PyrightAdapter];

/// Execution context for Tier 3 language tool adapters (zero tuple-packing).
pub struct AdapterCtx<'a> {
    /// Repository root directory.
    pub root: &'a Path,
    /// Detected + configured project context.
    pub context: &'a ProjectContext,
    /// Optional staged file set when running `garde check --staged`.
    pub staged_filter: Option<&'a BTreeSet<RepoPath>>,
    /// CLI check options (`strict_tools`, etc.).
    pub options: &'a CheckOptions,
    /// Validated pure analyzer configuration.
    pub config: &'a Config,
}

fn effective_policy(configured: ScannerPolicy, strict_tools: bool) -> ScannerPolicy {
    if strict_tools && configured == ScannerPolicy::Auto {
        ScannerPolicy::Required
    } else {
        configured
    }
}

fn is_python_config_name(name: &str) -> bool {
    matches!(name, "pyproject.toml" | "ruff.toml" | ".ruff.toml")
}

/// Computes `Some(targets)` when Python adapters should run (`Vec::new()` means `.`),
/// or `None` when `--staged` contains zero `.py` or Python tool config files (0 ms fast-path).
fn resolve_python_targets(staged_filter: Option<&BTreeSet<RepoPath>>) -> Option<Vec<String>> {
    let Some(staged) = staged_filter else {
        return Some(Vec::new());
    };
    let mut py_files = Vec::new();
    let mut config_changed = false;
    for path in staged {
        if path
            .extension()
            .is_some_and(|ext| ext.eq_ignore_ascii_case("py"))
        {
            py_files.push(String::from(path.as_str()));
        } else if is_python_config_name(path.file_name()) {
            config_changed = true;
        }
    }
    if py_files.is_empty() && !config_changed {
        None
    } else if config_changed {
        Some(Vec::new())
    } else {
        Some(py_files)
    }
}

fn run_single_adapter(
    adapter: &dyn LanguageToolAdapter,
    ctx: &AdapterCtx<'_>,
    py_targets: &[String],
) -> Result<Vec<Diagnostic>, EngineError> {
    let policy = effective_policy(
        adapter.configured_policy(ctx.config),
        ctx.options.strict_tools,
    );
    if policy == ScannerPolicy::Off {
        return Ok(Vec::new());
    }
    let tool_name = adapter.tool_id().as_str();
    let Some(launcher) = resolve_python_tool(ctx.root, tool_name) else {
        return if policy == ScannerPolicy::Required {
            Ok(vec![missing_tool_diagnostic(
                tool_name,
                adapter.install_hint(),
            )?])
        } else {
            Ok(Vec::new())
        };
    };

    let args = adapter.build_args(py_targets);
    let output = launcher.execute(ctx.root, &args)?;
    let stdout = String::from_utf8_lossy(&output.stdout);
    if stdout.trim().is_empty() {
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            let stderr_summary = stderr
                .lines()
                .find(|l| !l.trim().is_empty())
                .unwrap_or("empty stdout and stderr")
                .trim();
            return Err(EngineError::InvalidConfig(format!(
                "{tool_name} crashed (status {:?}): {stderr_summary}",
                output.status.code()
            )));
        }
        return Ok(Vec::new());
    }
    let findings = adapter
        .parse_output(&stdout, ctx.root)
        .map_err(EngineError::InvalidConfig)?;
    Ok(convert_external_tool_findings(&findings))
}

/// Runs active Tier 3 language tool adapters in parallel, converting findings to `GF082`.
///
/// # Errors
///
/// Returns [`EngineError`] on unexpected process spawn failure, tool crash, or malformed JSON output.
pub fn run_language_tool_adapters(ctx: &AdapterCtx<'_>) -> Result<Vec<Diagnostic>, EngineError> {
    if !ctx.context.languages.contains(&LanguageId::Python) {
        return Ok(Vec::new());
    }
    let Some(py_targets) = resolve_python_targets(ctx.staged_filter) else {
        return Ok(Vec::new());
    };

    let batches: Vec<Vec<Diagnostic>> = PYTHON_ADAPTERS
        .par_iter()
        .map(|adapter| run_single_adapter(*adapter, ctx, &py_targets))
        .collect::<Result<Vec<_>, _>>()?;

    Ok(batches.into_iter().flatten().collect())
}
