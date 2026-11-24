//! Tier 3 language ecosystem tool ingestion orchestrator (`GF082` & `GF043`).

pub mod python_runners;
pub mod ts_runners;

use std::collections::BTreeSet;
use std::path::Path;

use gf_core::{
    Config, Diagnostic, LanguageId, ProjectContext, RepoPath, ScannerPolicy, is_tsconfig_filename,
};
pub use gf_parsers::{
    ToolParseError, parse_biome_json, parse_mypy_ndjson, parse_oxlint_json, parse_pyright_json,
    parse_ruff_json, parse_tsc_output,
};
use gf_rules::convert_external_tool_findings;
use rayon::prelude::*;

pub use python_runners::{
    ExternalAdapter, ResolvedPythonTool, resolve_python_tool, resolve_python_tool_with_uv,
};
pub use ts_runners::resolve_node_tool;

use crate::scanners::gitleaks::missing_tool_diagnostic;
use crate::types::{CheckOptions, EngineError};

const TS_EXTENSIONS: &[&str] = &["ts", "tsx", "mts", "cts", "js", "jsx", "mjs", "cjs"];

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

fn is_ts_config_name(name: &str) -> bool {
    is_tsconfig_filename(name)
        || matches!(
            name,
            "package.json" | "biome.json" | "biome.jsonc" | ".oxlintrc.json"
        )
}

fn resolve_staged_targets(
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

fn resolve_adapter_launcher(
    adapter: ExternalAdapter,
    policy: ScannerPolicy,
    root: &Path,
) -> Option<ResolvedPythonTool> {
    let tool_name = adapter.tool_kind().as_str();
    if adapter.is_python() {
        return resolve_python_tool(root, tool_name);
    }
    if adapter == ExternalAdapter::Tsc
        && policy == ScannerPolicy::Auto
        && root.join("package.json").is_file()
        && !root.join("node_modules").is_dir()
    {
        return None;
    }
    resolve_node_tool(root, tool_name).map(ResolvedPythonTool::Direct)
}

fn run_single_adapter(
    adapter: ExternalAdapter,
    ctx: &AdapterCtx<'_>,
    targets: &[String],
) -> Result<Vec<Diagnostic>, EngineError> {
    let policy = effective_policy(
        adapter.configured_policy(ctx.config),
        ctx.options.strict_tools,
    );
    if policy == ScannerPolicy::Off {
        return Ok(Vec::new());
    }
    let tool_name = adapter.tool_kind().as_str();
    let Some(launcher) = resolve_adapter_launcher(adapter, policy, ctx.root) else {
        return if policy == ScannerPolicy::Required {
            Ok(vec![missing_tool_diagnostic(
                tool_name,
                adapter.install_hint(),
            )?])
        } else {
            Ok(Vec::new())
        };
    };

    let args = adapter.build_args(targets);
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
        .map_err(|e| EngineError::InvalidConfig(e.to_string()))?;
    Ok(convert_external_tool_findings(&findings))
}

/// Runs active Tier 3 language tool adapters in parallel, converting findings to `GF082`.
///
/// # Errors
///
/// Returns [`EngineError`] on unexpected process spawn failure, tool crash, or malformed JSON output.
pub fn run_language_tool_adapters(ctx: &AdapterCtx<'_>) -> Result<Vec<Diagnostic>, EngineError> {
    let mut active_jobs: Vec<(ExternalAdapter, Vec<String>)> = Vec::new();
    if ctx.context.languages.contains(&LanguageId::Python)
        && let Some(py_targets) =
            resolve_staged_targets(ctx.staged_filter, &["py"], is_python_config_name)
    {
        for adapter in ExternalAdapter::PYTHON_ALL {
            active_jobs.push((adapter, py_targets.clone()));
        }
    }
    if ctx.context.languages.contains(&LanguageId::TypeScript)
        && let Some(ts_targets) =
            resolve_staged_targets(ctx.staged_filter, TS_EXTENSIONS, is_ts_config_name)
    {
        for adapter in ExternalAdapter::TYPESCRIPT_ALL {
            active_jobs.push((adapter, ts_targets.clone()));
        }
    }
    if active_jobs.is_empty() {
        return Ok(Vec::new());
    }

    let batches: Vec<Vec<Diagnostic>> = active_jobs
        .par_iter()
        .map(|(adapter, targets)| run_single_adapter(*adapter, ctx, targets))
        .collect::<Result<Vec<_>, _>>()?;

    Ok(batches.into_iter().flatten().collect())
}
