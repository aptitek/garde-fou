//! CLI argument definitions for `garde config`, `garde cache`, `garde uninstall`, and `garde eject`.

use std::path::PathBuf;

use clap::{Args, Subcommand};

/// Interactive and live watch mode switches for `garde check`.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Args)]
pub struct CheckModeFlags {
    /// Open the interactive 3-tab keyboard TUI cockpit after analysis (alias for `garde tui`).
    #[arg(short = 'i', long)]
    pub interactive: bool,
    /// Watch workspace files and re-run incremental BLAKE3 checks on every save.
    #[arg(short = 'w', long, conflicts_with = "interactive")]
    pub watch: bool,
}

/// Arguments for `garde config` (opens directly on Tab `[2] Projet & Garde-fous`).
#[derive(Clone, Debug, Default, Eq, PartialEq, Args)]
pub struct ConfigArgs {
    /// Optional workspace root directory (defaults to current working directory).
    #[arg(value_name = "PATH")]
    pub path: Option<PathBuf>,
    /// Skip remote `skills.sh` downloads when applying configuration changes.
    #[arg(long)]
    pub offline: bool,
}

/// Sub-actions for `garde cache`.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Subcommand)]
pub enum CacheSubcommand {
    /// Display `.garde-fou-cache/` schema version, entries, and disk usage.
    #[default]
    Status,
    /// Remove obsolete schema directories (`v1..v{N-1}`) and orphaned entries while keeping warm cache.
    Prune,
    /// Delete the entire `.garde-fou-cache/` directory and report freed bytes.
    Clean,
}

/// Arguments for `garde cache [status|prune|clean] [PATH]`.
#[derive(Clone, Debug, Default, Eq, PartialEq, Args)]
pub struct CacheArgs {
    /// Cache operation (`status`, `prune`, or `clean`; defaults to `status`).
    #[command(subcommand)]
    pub action: Option<CacheSubcommand>,
    /// Optional workspace root directory (defaults to current working directory).
    #[arg(value_name = "PATH", global = true)]
    pub path: Option<PathBuf>,
}

/// Target scope flags for `garde uninstall`.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Args)]
pub struct UninstallScopeFlags {
    /// Detach Garde-fou surgically from the target project repository.
    #[arg(long)]
    pub project: bool,
    /// Uninstall the `garde` binary from this machine (`mise` or standalone executable).
    #[arg(long = "self")]
    pub self_bin: bool,
}

/// Safety and skill-cleanup flags shared by `garde uninstall` and `garde eject`.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Args)]
pub struct UninstallBehaviorFlags {
    /// Also remove catalog-installed skills from `.agents/skills/` (custom user skills are always preserved).
    #[arg(long)]
    pub purge_skills: bool,
    /// Preview the exact uninstall plan without modifying or deleting any file on disk.
    #[arg(long)]
    pub dry_run: bool,
}

/// Arguments for `garde uninstall`.
#[derive(Clone, Debug, Default, Eq, PartialEq, Args)]
pub struct UninstallArgs {
    /// Optional project root directory (defaults to current working directory).
    #[arg(value_name = "PATH")]
    pub path: Option<PathBuf>,
    /// Target uninstall scope (`--project` and/or `--self`).
    #[command(flatten)]
    pub scope: UninstallScopeFlags,
    /// Cleanup and dry-run controls.
    #[command(flatten)]
    pub behavior: UninstallBehaviorFlags,
    /// Skip interactive confirmation prompt.
    #[arg(short = 'y', long)]
    pub yes: bool,
}

/// Arguments for `garde eject [PATH]` (alias for `garde uninstall --project`).
#[derive(Clone, Debug, Default, Eq, PartialEq, Args)]
pub struct EjectArgs {
    /// Optional project root directory (defaults to current working directory).
    #[arg(value_name = "PATH")]
    pub path: Option<PathBuf>,
    /// Cleanup and dry-run controls.
    #[command(flatten)]
    pub behavior: UninstallBehaviorFlags,
    /// Skip interactive confirmation prompt.
    #[arg(short = 'y', long)]
    pub yes: bool,
}
