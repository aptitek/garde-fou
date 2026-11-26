//! CLI argument structures for `garde` (`check`, `init`, `tui`, `config`, `cache`, `uninstall`, `eject`, `sync`, `lock`, `commit-msg`).

use std::path::PathBuf;

use clap::{Args, Parser, Subcommand, ValueEnum};

pub use crate::args_ops::{
    CacheArgs, CacheSubcommand, CheckModeFlags, ConfigArgs, EjectArgs, UninstallArgs,
    UninstallBehaviorFlags, UninstallScopeFlags,
};

/// Output format selector for `garde check`.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, ValueEnum)]
pub enum OutputFormat {
    /// Concise Lipgloss-styled human-oriented terminal output.
    #[default]
    Human,
    /// Structured Markdown optimized for AI coding agents.
    Agent,
    /// OASIS `SARIF` 2.1.0 JSON report.
    Sarif,
}

/// Terminal ANSI color policy flag (`--color`).
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, ValueEnum)]
pub enum ColorChoice {
    /// Detect automatically from `NO_COLOR`, `TERM`, `CI`, `CLICOLOR_FORCE`, and TTY state.
    #[default]
    Auto,
    /// Force ANSI colors even when redirected.
    Always,
    /// Disable all ANSI escape codes (`ColorPolicy::Plain`).
    Never,
}

/// UI language selector flag (`--locale`).
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, ValueEnum)]
pub enum LocaleChoice {
    /// Detect from `GARDE_LOCALE` / `LC_ALL` / `LANG`, defaulting to `en`.
    #[default]
    Auto,
    /// Render human-facing output in French (`locales/fr.toml`).
    Fr,
    /// Render human-facing output in English (`locales/en.toml`).
    En,
}

/// Scope and cache flags shared by `garde check` and `garde tui`.
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
    /// Interactive TUI (`-i`) or live watch (`-w`) switches.
    #[command(flatten)]
    pub mode: CheckModeFlags,
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

/// Arguments for `garde tui`.
#[derive(Clone, Debug, Default, Eq, PartialEq, Args)]
pub struct TuiArgs {
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
}

/// Interactive wizard mode controls for `garde init`.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Args)]
pub struct InitModeFlags {
    /// Launch the interactive Gum-style project wizard.
    #[arg(short = 'i', long)]
    pub interactive: bool,
    /// Force non-interactive initialization (`Minimal` defaults when unconfigured) even in a TTY.
    #[arg(short = 'y', long, conflicts_with = "interactive")]
    pub yes: bool,
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
    /// Explicit workspace archetype (`minimal`, `scripts`, `infra`, `knowledge`, `academic`, `software`).
    #[arg(long, value_name = "KIND")]
    pub archetype: Option<String>,
    /// Project language(s) (`rust`, `typescript`, `python`, `shell`, `powershell`, `yaml`, `sql`, `toml`, `markdown`, `css`, `xml`).
    #[arg(long = "lang", value_name = "LANG", value_delimiter = ',')]
    pub langs: Vec<String>,
    /// Infrastructure & fortress feature(s) (`docker`, `git-hooks`, `ci-github`, `task-runner`, `ui`, `panda-css`, `m3e`, `w3c`, `i18n`, `content`, `tdd`, `storybook`).
    #[arg(long = "feature", value_name = "FEAT", value_delimiter = ',')]
    pub features: Vec<String>,
    /// Framework(s) (`cargo-workspace`, `react`, `astro`, `nextjs`, `fastapi`, `django`, `react-router`, `cloudflare-workers`, `drizzle`, `docker-compose`, `obsidian`, `quarto`, `jupyter`).
    #[arg(long = "framework", value_name = "FW", value_delimiter = ',')]
    pub frameworks: Vec<String>,
    /// Interactive / non-interactive mode switches.
    #[command(flatten)]
    pub mode: InitModeFlags,
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
    /// Launch the 3-tab interactive live cockpit (`Diagnostics`, `Config`, `Cache`).
    Tui(TuiArgs),
    /// Open the interactive checkbox configurator for `garde-fou.toml` (`Tab [2]`).
    Config(ConfigArgs),
    /// Inspect, prune, or clean the `.garde-fou-cache/` BLAKE3 cache.
    Cache(CacheArgs),
    /// Surgically detach Garde-fou from a project (`--project`) and/or uninstall `garde` (`--self`).
    Uninstall(UninstallArgs),
    /// Alias for `garde uninstall --project` (non-destructive project detachment).
    Eject(EjectArgs),
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
    /// Control ANSI color output (`auto`, `always`, `never`).
    #[arg(long, global = true, value_enum, default_value_t = ColorChoice::Auto)]
    pub color: ColorChoice,
    /// Select human-facing UI language (`auto`, `fr`, `en`).
    #[arg(long, global = true, value_enum, default_value_t = LocaleChoice::Auto)]
    pub locale: LocaleChoice,
    /// Subcommand to execute.
    #[command(subcommand)]
    pub command: Commands,
}
