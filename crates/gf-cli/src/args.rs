//! CLI argument structures for `garde` (`check`, `init`, `lock`, `commit-msg`).

use std::path::PathBuf;

use clap::{Args, Parser, Subcommand, ValueEnum};

/// Output format selector for `garde check`.
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

/// Scope and cache flags for `garde check`.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Args)]
pub struct CheckExecutionFlags {
    /// Restrict analysis to staged Git files only.
    #[arg(long)]
    pub staged: bool,
    /// Force full `Tree-sitter` re-evaluation, bypassing cached entries.
    #[arg(long)]
    pub no_cache: bool,
}

/// Arguments for `garde check`.
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
    /// Optional path to a `cargo-mutants` (`outcomes.json`) or `Stryker` (`mutation.json`) report (`GF052`).
    #[arg(long, value_name = "PATH")]
    pub mutants: Option<String>,
    /// Select diagnostic output format (`human`, `agent`, or `sarif`).
    #[arg(long, value_enum, default_value_t = OutputFormat::Human)]
    pub format: OutputFormat,
}

/// Arguments for `garde init`.
#[derive(Clone, Debug, Default, Eq, PartialEq, Args)]
pub struct InitArgs {
    /// Target directory to initialize (defaults to current working directory).
    #[arg(value_name = "PATH")]
    pub path: Option<PathBuf>,
    /// Explicit project name override (defaults to directory name).
    #[arg(long, value_name = "NAME")]
    pub name: Option<String>,
    /// Project language(s) (`rust`, `typescript`, `python`).
    #[arg(long = "lang", value_name = "LANG", value_delimiter = ',')]
    pub langs: Vec<String>,
    /// Infrastructure & fortress feature(s) (`docker`, `git-hooks`, `ci-github`, `task-runner`, `ui`, `i18n`, `content`, `tdd`, `storybook`).
    #[arg(long = "feature", value_name = "FEAT", value_delimiter = ',')]
    pub features: Vec<String>,
    /// Framework(s) (`cargo-workspace`, `react`, `astro`, `nextjs`, `fastapi`, `django`).
    #[arg(long = "framework", value_name = "FW", value_delimiter = ',')]
    pub frameworks: Vec<String>,
    /// Skip running `git init -b main`.
    #[arg(long)]
    pub no_git: bool,
    /// Skip live `skills.sh` downloads (install only embedded `garde-fou` skill).
    #[arg(long)]
    pub offline: bool,
}

/// Arguments for `garde sync`.
#[derive(Clone, Debug, Default, Eq, PartialEq, Args)]
pub struct SyncArgs {
    /// Optional workspace root directory (defaults to current working directory).
    #[arg(value_name = "PATH")]
    pub path: Option<PathBuf>,
    /// Skip live `skills.sh` downloads (write `AGENTS.md` and embedded skill only).
    #[arg(long)]
    pub offline: bool,
    /// Re-fetch and overwrite already installed `skills.sh` skills.
    #[arg(long)]
    pub update_skills: bool,
    /// Install skills from a local directory mirror instead of `skills.sh`.
    #[arg(long, value_name = "DIR")]
    pub skills_from_dir: Option<PathBuf>,
}

/// Arguments for `garde lock`.
#[derive(Clone, Debug, Default, Eq, PartialEq, Args)]
pub struct LockArgs {
    /// Optional workspace root directory containing `garde-fou.toml`.
    #[arg(value_name = "PATH")]
    pub path: Option<PathBuf>,
}

/// Arguments for `garde commit-msg`.
#[derive(Clone, Debug, Eq, PartialEq, Args)]
pub struct CommitMsgArgs {
    /// Path to the Git commit message file (`.git/COMMIT_EDITMSG`).
    #[arg(value_name = "MSG_FILE")]
    pub msg_file: PathBuf,
}

/// Subcommands supported by the `garde` CLI.
#[derive(Clone, Debug, Eq, PartialEq, Subcommand)]
pub enum Commands {
    /// Run static analysis rules, secret/supply-chain scanners, and coverage gates.
    Check(CheckArgs),
    /// Initialize a strict project workspace (`uv init`-style) or adopt an existing repo.
    Init(InitArgs),
    /// Synchronize modular `AGENTS.md` fragments, `.agents/skills/`, and `garde.lock`.
    Sync(SyncArgs),
    /// Cryptographically seal `garde-fou.toml` into `garde.lock` (`BLAKE3`, `GF070`).
    Lock(LockArgs),
    /// Validate a Git commit message file against Conventional Commits (`GF077`).
    CommitMsg(CommitMsgArgs),
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
