//! Command-line definition and command dispatcher for `garde` (**Garde-fou**).

pub mod args;
pub mod args_ops;
mod cache_cmd;
mod commands;
mod init_cmd;
pub mod output;
pub mod reporter;
pub mod source_loader;
pub mod tui;
mod tui_runner;
pub mod ui;
mod uninstall_cmd;
mod watch_runner;

use std::path::Path;
use std::process::ExitCode;

use gf_engine::EngineError;

pub use crate::{
    args::{
        CheckArgs, CheckExecutionFlags, Cli, ColorChoice, Commands, CommitMsgArgs, InitArgs,
        InitModeFlags, LocaleChoice, LockArgs, OutputFormat, SyncArgs, TuiArgs,
    },
    args_ops::{
        CacheArgs, CacheSubcommand, CheckModeFlags, ConfigArgs, EjectArgs, UninstallArgs,
        UninstallBehaviorFlags, UninstallScopeFlags,
    },
    reporter::{
        render_check_report, render_init_report, render_localized_check_banner,
        render_localized_cli_label, render_sync_report,
    },
};
use crate::{
    cache_cmd::execute_cache,
    commands::{
        execute_check, execute_commit_msg, execute_config, execute_lock, execute_sync, execute_tui,
    },
    init_cmd::execute_init,
    output::probe_stdout_capabilities,
    ui::{ColorPolicy, resolve_color_policy},
    uninstall_cmd::{execute_eject, execute_uninstall},
};

/// Executes a parsed [`Cli`] command relative to `cwd` with an explicit [`ColorPolicy`].
///
/// # Errors
///
/// Returns [`EngineError`] if workspace configuration, I/O, or parsing fails.
pub fn execute_with_policy(
    cli: &Cli,
    cwd: &Path,
    policy: ColorPolicy,
) -> Result<(String, ExitCode), EngineError> {
    match &cli.command {
        Commands::Check(args) => execute_check(args, cwd, policy),
        Commands::Tui(args) => execute_tui(args, cwd, policy),
        Commands::Config(args) => execute_config(args, cwd, policy),
        Commands::Init(args) => execute_init(args, cwd, policy),
        Commands::Sync(args) => execute_sync(args, cwd, policy),
        Commands::Lock(args) => execute_lock(args, cwd, policy),
        Commands::Cache(args) => execute_cache(args, cwd, policy),
        Commands::Uninstall(args) => execute_uninstall(args, cwd, policy),
        Commands::Eject(args) => execute_eject(args, cwd, policy),
        Commands::CommitMsg(args) => execute_commit_msg(args, cwd, policy),
    }
}

/// Executes a parsed [`Cli`] command relative to `cwd`, resolving [`ColorPolicy`] automatically.
///
/// # Errors
///
/// Returns [`EngineError`] if workspace configuration, I/O, or parsing fails.
pub fn execute(cli: &Cli, cwd: &Path) -> Result<(String, ExitCode), EngineError> {
    let policy = resolve_color_policy(cli.color, &probe_stdout_capabilities());
    execute_with_policy(cli, cwd, policy)
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::process::ExitCode;
    use std::time::{SystemTime, UNIX_EPOCH};

    use clap::Parser;

    use super::{
        CheckArgs, CheckExecutionFlags, CheckModeFlags, Cli, ColorChoice, Commands, OutputFormat,
        execute_with_policy,
    };
    use crate::ui::ColorPolicy;

    #[test]
    fn parses_enriched_check_flags() -> Result<(), Box<dyn std::error::Error>> {
        let parsed = Cli::try_parse_from([
            "garde",
            "--color",
            "never",
            "check",
            "--staged",
            "--no-cache",
            "--strict-tools",
            "--lcov",
            "coverage/lcov.info",
            "--mutants",
            "mutants.out/outcomes.json",
            "--format",
            "agent",
        ])?;
        assert_eq!(parsed.color, ColorChoice::Never);
        assert_eq!(
            parsed.command,
            Commands::Check(CheckArgs {
                exec: CheckExecutionFlags {
                    staged: true,
                    no_cache: true,
                },
                strict_tools: true,
                mode: CheckModeFlags {
                    interactive: false,
                    watch: false,
                },
                lcov: Some(String::from("coverage/lcov.info")),
                mutants: Some(String::from("mutants.out/outcomes.json")),
                format: OutputFormat::Agent,
            })
        );
        Ok(())
    }

    #[test]
    fn executes_init_lock_commit_msg_and_clean_check() -> Result<(), Box<dyn std::error::Error>> {
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
            "--offline",
        ])?;
        let (init_text, init_code) = execute_with_policy(&init_cli, &dir, ColorPolicy::Plain)?;
        assert_eq!(init_code, ExitCode::SUCCESS);
        assert!(init_text.contains("demo-svc"));
        assert!(dir.join("garde.lock").is_file());

        let msg_file = dir.join("COMMIT_EDITMSG");
        fs::write(&msg_file, "feat(rules): add tarjan cycle detector\n")?;
        let ok_msg = Cli::try_parse_from(["garde", "commit-msg", "COMMIT_EDITMSG"])?;
        let (_, ok_code) = execute_with_policy(&ok_msg, &dir, ColorPolicy::Plain)?;
        assert_eq!(ok_code, ExitCode::SUCCESS);

        fs::write(&msg_file, "wip: fix stuff\n")?;
        let bad_msg = Cli::try_parse_from(["garde", "commit-msg", "COMMIT_EDITMSG"])?;
        let (bad_out, bad_code) = execute_with_policy(&bad_msg, &dir, ColorPolicy::Plain)?;
        assert_eq!(bad_code, ExitCode::FAILURE);
        assert!(bad_out.contains("GF077"));

        let fresh_check = Cli::try_parse_from(["garde", "check", "--no-cache"])?;
        let (_, check_code) = execute_with_policy(&fresh_check, &dir, ColorPolicy::Plain)?;
        assert_eq!(check_code, ExitCode::SUCCESS);

        fs::remove_dir_all(&dir)?;
        Ok(())
    }
}
