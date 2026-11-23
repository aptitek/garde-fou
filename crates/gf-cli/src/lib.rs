//! Command-line definition and command dispatcher for `garde-fou`.

pub mod output;
pub mod reporter;

use std::path::Path;
use std::process::ExitCode;

use clap::{Parser, Subcommand, ValueEnum};
use gf_engine::{CheckOptions, EngineError, ScanScope, run_check_with_options, scaffold_config};

use reporter::{render_check_report, render_init_outcome};

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

/// Subcommands supported by the `garde-fou` CLI.
#[derive(Clone, Debug, Eq, PartialEq, Subcommand)]
pub enum Commands {
    /// Run static analysis rules and quality gates on the repository.
    Check {
        /// Restrict analysis to staged Git files only.
        #[arg(long)]
        staged: bool,
        /// Force full `Tree-sitter` re-evaluation, bypassing cached entries.
        #[arg(long)]
        no_cache: bool,
        /// Select diagnostic output format (`human`, `agent`, or `sarif`).
        #[arg(long, value_enum, default_value_t = OutputFormat::Human)]
        format: OutputFormat,
    },
    /// Scaffold an initial `garde-fou.toml` configuration in the workspace.
    Init,
}

/// Top-level `clap` parser for the `garde-fou` binary.
#[derive(Clone, Debug, Eq, PartialEq, Parser)]
#[command(
    name = "garde-fou",
    version,
    about = "Ultra-strict polyglot static analysis orchestrator"
)]
pub struct Cli {
    /// Subcommand to execute.
    #[command(subcommand)]
    pub command: Commands,
}

/// Executes a parsed [`Cli`] command relative to `cwd`, returning the formatted output
/// and process [`ExitCode`].
///
/// # Errors
///
/// Returns [`EngineError`] if workspace configuration, I/O, or parsing fails.
pub fn execute(cli: &Cli, cwd: &Path) -> Result<(String, ExitCode), EngineError> {
    match cli.command {
        Commands::Check {
            staged,
            no_cache,
            format,
        } => {
            let scope = if staged {
                ScanScope::StagedOnly
            } else {
                ScanScope::All
            };
            let report = run_check_with_options(cwd, CheckOptions { scope, no_cache })?;
            let exit = if report.is_clean() {
                ExitCode::SUCCESS
            } else {
                ExitCode::FAILURE
            };
            Ok((render_check_report(&report, format), exit))
        }
        Commands::Init => {
            let outcome = scaffold_config(cwd)?;
            Ok((render_init_outcome(&outcome), ExitCode::SUCCESS))
        }
    }
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::process::ExitCode;
    use std::time::{SystemTime, UNIX_EPOCH};

    use clap::Parser;

    use super::{Cli, Commands, OutputFormat, execute};

    #[test]
    fn parses_cli_flags_and_executes_subcommands() -> Result<(), Box<dyn std::error::Error>> {
        let parsed = Cli::try_parse_from([
            "garde-fou",
            "check",
            "--staged",
            "--no-cache",
            "--format",
            "agent",
        ])?;
        assert_eq!(
            parsed.command,
            Commands::Check {
                staged: true,
                no_cache: true,
                format: OutputFormat::Agent,
            }
        );

        let nanos = SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos();
        let dir = std::env::temp_dir().join(format!("gf-cli-unit-{nanos}"));
        fs::create_dir_all(&dir)?;

        let init_cli = Cli::try_parse_from(["garde-fou", "init"])?;
        let (init_text, init_code) = execute(&init_cli, &dir)?;
        assert_eq!(init_code, ExitCode::SUCCESS);
        assert!(init_text.contains("created"));

        let fresh_check = Cli::try_parse_from(["garde-fou", "check", "--no-cache"])?;
        let (_, check_code) = execute(&fresh_check, &dir)?;
        assert_eq!(check_code, ExitCode::SUCCESS);

        let _ = fs::remove_dir_all(&dir);
        Ok(())
    }
}
