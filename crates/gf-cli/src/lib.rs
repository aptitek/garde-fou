//! Command-line definition and command dispatcher for `garde-fou`.

pub mod output;
pub mod reporter;

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use clap::{Args, Parser, Subcommand, ValueEnum};
use gf_core::{FeatureId, FrameworkId, LanguageId};
use gf_engine::{
    CheckOptions, EngineError, InitRequest, ScanScope, run_check_with_options, run_init,
};

use reporter::{render_check_report, render_init_report};

/// Output format selector for `garde-fou check`.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, ValueEnum)]
pub enum OutputFormat {
    /// Concise human-oriented terminal output.
    #[default]
    Human,
    /// Structured Markdown optimized for AI coding agents.
    Agent,
    /// OASIS `SARIF` 2.1.0 JSON report.
    Sarif,
}

/// Scope and cache flags for `garde-fou check`.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Args)]
pub struct CheckExecutionFlags {
    /// Restrict analysis to staged Git files only.
    #[arg(long)]
    pub staged: bool,
    /// Force full `Tree-sitter` re-evaluation, bypassing cached entries.
    #[arg(long)]
    pub no_cache: bool,
}

/// Arguments for `garde-fou check`.
#[derive(Clone, Debug, Default, Eq, PartialEq, Args)]
pub struct CheckArgs {
    /// Execution scope and cache controls.
    #[command(flatten)]
    pub exec: CheckExecutionFlags,
    /// Upgrade `auto` scanner policies (`gitleaks`, `osv-scanner`) to `required` (`GF043`).
    #[arg(long)]
    pub strict_tools: bool,
    /// Optional path to an `LCOV` coverage report (`lcov.info`) verified by `GF050`.
    #[arg(long, value_name = "PATH")]
    pub lcov: Option<String>,
    /// Select diagnostic output format (`human`, `agent`, or `sarif`).
    #[arg(long, value_enum, default_value_t = OutputFormat::Human)]
    pub format: OutputFormat,
}

/// Arguments for `garde-fou init`.
#[derive(Clone, Debug, Default, Eq, PartialEq, Args)]
pub struct InitArgs {
    /// Target directory to initialize (defaults to current working directory).
    #[arg(value_name = "PATH")]
    pub path: Option<PathBuf>,
    /// Explicit project name override (defaults to directory name).
    #[arg(long, value_name = "NAME")]
    pub name: Option<String>,
    /// Project language(s) (`rust`, `typescript`, `python`).
    #[arg(long = "lang", value_name = "LANG")]
    pub langs: Vec<String>,
    /// Infrastructure feature(s) (`docker`, `git-hooks`, `ci-github`, `task-runner`).
    #[arg(long = "feature", value_name = "FEAT")]
    pub features: Vec<String>,
    /// Framework(s) (`cargo-workspace`, `react`, `astro`, `nextjs`, `fastapi`, `django`).
    #[arg(long = "framework", value_name = "FW")]
    pub frameworks: Vec<String>,
    /// Skip running `git init -b main`.
    #[arg(long)]
    pub no_git: bool,
}

/// Subcommands supported by the `garde-fou` CLI.
#[derive(Clone, Debug, Eq, PartialEq, Subcommand)]
pub enum Commands {
    /// Run static analysis rules, secret/supply-chain scanners, and coverage gates.
    Check(CheckArgs),
    /// Initialize a strict project workspace (`uv init`-style) or adopt an existing repo.
    Init(InitArgs),
}

/// Top-level `clap` parser for the `garde` binary.
#[derive(Clone, Debug, Eq, PartialEq, Parser)]
#[command(
    name = "garde",
    version,
    about = "Ultra-strict polyglot static analysis orchestrator (Garde-fou)"
)]
pub struct Cli {
    /// Subcommand to execute.
    #[command(subcommand)]
    pub command: Commands,
}

fn build_init_request(args: &InitArgs, cwd: &Path) -> Result<InitRequest, EngineError> {
    let root = args.path.as_ref().map_or_else(
        || cwd.to_path_buf(),
        |rel| {
            if rel.is_absolute() {
                rel.clone()
            } else {
                cwd.join(rel)
            }
        },
    );
    let mut languages = BTreeSet::new();
    for raw in &args.langs {
        let id = LanguageId::from_config_str(raw)
            .ok_or_else(|| EngineError::InvalidConfig(format!("unknown --lang `{raw}`")))?;
        languages.insert(id);
    }
    let mut features = BTreeSet::new();
    for raw in &args.features {
        let id = FeatureId::from_config_str(raw)
            .ok_or_else(|| EngineError::InvalidConfig(format!("unknown --feature `{raw}`")))?;
        features.insert(id);
    }
    let mut frameworks = BTreeSet::new();
    for raw in &args.frameworks {
        let id = FrameworkId::from_config_str(raw)
            .ok_or_else(|| EngineError::InvalidConfig(format!("unknown --framework `{raw}`")))?;
        frameworks.insert(id);
    }

    Ok(InitRequest {
        root,
        project_name: args.name.clone(),
        languages,
        features,
        frameworks,
        run_git_init: !args.no_git,
    })
}

/// Executes a parsed [`Cli`] command relative to `cwd`, returning the formatted output
/// and process [`ExitCode`].
///
/// # Errors
///
/// Returns [`EngineError`] if workspace configuration, I/O, or parsing fails.
pub fn execute(cli: &Cli, cwd: &Path) -> Result<(String, ExitCode), EngineError> {
    match &cli.command {
        Commands::Check(args) => {
            let scope = if args.exec.staged {
                ScanScope::StagedOnly
            } else {
                ScanScope::All
            };
            let options = CheckOptions {
                scope,
                no_cache: args.exec.no_cache,
                strict_tools: args.strict_tools,
                lcov_path: args.lcov.clone(),
            };
            let report = run_check_with_options(cwd, &options)?;
            let exit = if report.is_clean() {
                ExitCode::SUCCESS
            } else {
                ExitCode::FAILURE
            };
            Ok((render_check_report(&report, args.format), exit))
        }
        Commands::Init(args) => {
            let req = build_init_request(args, cwd)?;
            let report = run_init(&req)?;
            Ok((render_init_report(&report), ExitCode::SUCCESS))
        }
    }
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::process::ExitCode;
    use std::time::{SystemTime, UNIX_EPOCH};

    use clap::Parser;

    use super::{CheckArgs, CheckExecutionFlags, Cli, Commands, OutputFormat, execute};

    #[test]
    fn parses_enriched_cli_flags_and_bootstraps_clean_project()
    -> Result<(), Box<dyn std::error::Error>> {
        let parsed = Cli::try_parse_from([
            "garde",
            "check",
            "--staged",
            "--no-cache",
            "--strict-tools",
            "--lcov",
            "coverage/lcov.info",
            "--format",
            "agent",
        ])?;
        assert_eq!(
            parsed.command,
            Commands::Check(CheckArgs {
                exec: CheckExecutionFlags {
                    staged: true,
                    no_cache: true,
                },
                strict_tools: true,
                lcov: Some(String::from("coverage/lcov.info")),
                format: OutputFormat::Agent,
            })
        );

        let nanos = SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos();
        let dir = std::env::temp_dir().join(format!("gf-cli-unit-{nanos}"));
        fs::create_dir_all(&dir)?;

        let init_cli = Cli::try_parse_from([
            "garde",
            "init",
            "--name",
            "demo-svc",
            "--lang",
            "rust",
            "--feature",
            "docker",
        ])?;
        let (init_text, init_code) = execute(&init_cli, &dir)?;
        assert_eq!(init_code, ExitCode::SUCCESS);
        assert!(init_text.contains("demo-svc"));

        let fresh_check = Cli::try_parse_from(["garde", "check", "--no-cache"])?;
        let (_, check_code) = execute(&fresh_check, &dir)?;
        assert_eq!(check_code, ExitCode::SUCCESS);

        let _ = fs::remove_dir_all(&dir);
        Ok(())
    }
}
