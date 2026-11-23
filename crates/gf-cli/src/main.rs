//! Binary entrypoint for `garde-fou`.

use std::env;
use std::process::ExitCode;

use clap::Parser;
use gf_cli::{Cli, execute, output};

fn main() -> ExitCode {
    let cli = Cli::parse();
    let cwd = match env::current_dir() {
        Ok(dir) => dir,
        Err(err) => {
            output::write_stderr(&format!(
                "garde-fou: failed to resolve working directory: {err}"
            ));
            return ExitCode::FAILURE;
        }
    };

    match execute(&cli, &cwd) {
        Ok((rendered, code)) => {
            output::write_stdout(&rendered);
            code
        }
        Err(err) => {
            output::write_stderr(&format!("garde-fou: {err}"));
            ExitCode::FAILURE
        }
    }
}
