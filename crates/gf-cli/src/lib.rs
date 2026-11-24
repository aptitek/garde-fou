//! Command-line definition and command dispatcher for `garde` (**Garde-fou**).

pub mod args;
mod commands;
pub mod output;
pub mod reporter;

use std::path::Path;
use std::process::ExitCode;

use gf_engine::EngineError;

pub use args::{
    CheckArgs, CheckExecutionFlags, Cli, Commands, CommitMsgArgs, InitArgs, LockArgs, OutputFormat,
};
use commands::{execute_check, execute_commit_msg, execute_init, execute_lock};

/// Executes a parsed [`Cli`] command relative to `cwd`, returning the formatted output
/// and process [`ExitCode`].
///
/// # Errors
///
/// Returns [`EngineError`] if workspace configuration, I/O, or parsing fails.
pub fn execute(cli: &Cli, cwd: &Path) -> Result<(String, ExitCode), EngineError> {
    match &cli.command {
        Commands::Check(args) => execute_check(args, cwd),
        Commands::Init(args) => execute_init(args, cwd),
        Commands::Lock(args) => execute_lock(args, cwd),
        Commands::CommitMsg(args) => execute_commit_msg(args, cwd),
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
    fn parses_enriched_check_flags() -> Result<(), Box<dyn std::error::Error>> {
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
        ])?;
        let (init_text, init_code) = execute(&init_cli, &dir)?;
        assert_eq!(init_code, ExitCode::SUCCESS);
        assert!(init_text.contains("demo-svc"));
        assert!(dir.join("garde.lock").is_file());

        let msg_file = dir.join("COMMIT_EDITMSG");
        fs::write(&msg_file, "feat(rules): add tarjan cycle detector\n")?;
        let ok_msg = Cli::try_parse_from(["garde", "commit-msg", "COMMIT_EDITMSG"])?;
        let (_, ok_code) = execute(&ok_msg, &dir)?;
        assert_eq!(ok_code, ExitCode::SUCCESS);

        fs::write(&msg_file, "wip: fix stuff\n")?;
        let bad_msg = Cli::try_parse_from(["garde", "commit-msg", "COMMIT_EDITMSG"])?;
        let (bad_out, bad_code) = execute(&bad_msg, &dir)?;
        assert_eq!(bad_code, ExitCode::FAILURE);
        assert!(bad_out.contains("GF077"));

        let fresh_check = Cli::try_parse_from(["garde", "check", "--no-cache"])?;
        let (_, check_code) = execute(&fresh_check, &dir)?;
        assert_eq!(check_code, ExitCode::SUCCESS);

        let _ = fs::remove_dir_all(&dir);
        Ok(())
    }
}
