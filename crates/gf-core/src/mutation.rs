//! Pure `#![no_std]` mutation-testing algebra, survivor records, and `[mutation]` policy (`GF052`).

use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;
use serde::{Deserialize, Serialize};

use crate::diagnostic::Span;
use crate::path::RepoPath;

const MAX_REPLACEMENT_CHARS: usize = 120;

/// Truncates a mutant replacement description cleanly at `120` Unicode scalar values.
#[must_use]
pub fn truncate_replacement(raw: &str) -> String {
    let collapsed = raw.split_whitespace().collect::<Vec<_>>().join(" ");
    if collapsed.chars().count() <= MAX_REPLACEMENT_CHARS {
        return collapsed;
    }
    let head: String = collapsed.chars().take(MAX_REPLACEMENT_CHARS - 3).collect();
    format!("{head}...")
}

/// Mutation testing verification policy (`[mutation]` in `garde-fou.toml`, `GF052`).
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MutationConfig {
    /// Minimum required global mutation score (`0..=100`): `(caught + timeout) * 100 / viable`.
    pub min_mutation_score: Option<u32>,
    /// Critical module globs where any surviving mutant is immediately blocking (`100%` required).
    pub zero_survivor_globs: Vec<String>,
    /// Optional repository-relative path to the JSON mutation report.
    pub report_path: Option<String>,
}

impl MutationConfig {
    /// Returns an inactive `[mutation]` configuration.
    #[must_use]
    pub const fn empty() -> Self {
        Self {
            min_mutation_score: None,
            zero_survivor_globs: Vec::new(),
            report_path: None,
        }
    }

    /// Deterministic fragment included in `Config::canonical_string` for `BLAKE3` cache keys.
    #[must_use]
    pub fn canonical_fragment(&self) -> String {
        format!(
            "mut(min={:?},zs={:?},p={:?})",
            self.min_mutation_score, self.zero_survivor_globs, self.report_path
        )
    }
}

/// Normalized record for a single surviving (or `NoCoverage`) mutant (`GF052`).
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
pub struct SurvivedMutant {
    /// Mutated repository-relative source file.
    pub path: RepoPath,
    /// 1-indexed inclusive source line span of the mutation (`Span::FIRST` fallback).
    pub span: Span,
    /// Enclosing function or method name when reported by the mutation runner.
    pub function_name: Option<String>,
    /// Mutator category or operator name (e.g. `"FnValue"`, `"BinaryOperator"`).
    pub mutator: String,
    /// Survived replacement snippet or description (truncated to 120 chars).
    pub replacement: String,
}

/// Aggregated deterministic outcome of a `cargo-mutants` or `Stryker` / `Mutmut` campaign.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct MutationSummary {
    /// Mutants killed by at least one failing test (`CaughtMutant` / `Killed`).
    pub caught: u32,
    /// Mutants neutralized via timeout detection (`Timeout`).
    pub timeout: u32,
    /// Surviving or uncovered mutants (`MissedMutant` / `Survived` / `NoCoverage`).
    pub missed: u32,
    /// Uncompilable or ignored mutants excluded from the denominator (`Unviable` / `CompileError`).
    pub unviable: u32,
    /// Deterministically sorted list of surviving mutants.
    pub survivors: Vec<SurvivedMutant>,
}

impl MutationSummary {
    /// Total viable mutants (`caught + timeout + missed`), excluding `unviable`.
    #[must_use]
    pub const fn total_viable(&self) -> u32 {
        self.caught
            .saturating_add(self.timeout)
            .saturating_add(self.missed)
    }

    /// Total neutralized mutants (`caught + timeout`).
    #[must_use]
    pub const fn total_neutralized(&self) -> u32 {
        self.caught.saturating_add(self.timeout)
    }

    /// Integer floor mutation score (`0..=100`), or `None` when `total_viable() == 0` (anti-vacuity).
    #[must_use]
    pub fn score_percent_floor(&self) -> Option<u32> {
        let viable = self.total_viable();
        if viable == 0 {
            return None;
        }
        let num = u64::from(self.total_neutralized()).saturating_mul(100);
        let pct = (num / u64::from(viable)).min(100);
        Some(u32::try_from(pct).unwrap_or(100))
    }
}
