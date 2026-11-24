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
    root: &Path,
    env_cfg: (&CheckOptions, &Config),
    py_targets: &[String],
) -> Result<Vec<Diagnostic>, EngineError> {
    let (options, config) = env_cfg;
    let policy = effective_policy(adapter.configured_policy(config), options.strict_tools);
    if policy == ScannerPolicy::Off {
        return Ok(Vec::new());
    }
    let tool_name = adapter.tool_id().as_str();
    let Some(launcher) = resolve_python_tool(root, tool_name) else {
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
    let output = launcher.execute(root, &args)?;
    let stdout = String::from_utf8_lossy(&output.stdout);
    if stdout.trim().is_empty() {
        return Ok(Vec::new());
    }
    let findings = adapter
        .parse_output(&stdout, root)
        .map_err(EngineError::InvalidConfig)?;
    Ok(convert_external_tool_findings(&findings))
}

/// Runs active Tier 3 language tool adapters in parallel, converting findings to `GF082`.
///
/// # Errors
///
/// Returns [`EngineError`] on unexpected process spawn failure or malformed JSON output.
pub fn run_language_tool_adapters(
    root: &Path,
    scope_ctx: (&ProjectContext, Option<&BTreeSet<RepoPath>>),
    options: &CheckOptions,
    config: &Config,
) -> Result<Vec<Diagnostic>, EngineError> {
    let (ctx, staged_filter) = scope_ctx;
    if !ctx.languages.contains(&LanguageId::Python) {
        return Ok(Vec::new());
    }
    let Some(py_targets) = resolve_python_targets(staged_filter) else {
        return Ok(Vec::new());
    };

    let batches: Vec<Vec<Diagnostic>> = PYTHON_ADAPTERS
        .par_iter()
        .map(|adapter| run_single_adapter(*adapter, root, (options, config), &py_targets))
        .collect::<Result<Vec<_>, _>>()?;

    Ok(batches.into_iter().flatten().collect())
}
