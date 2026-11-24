//! Pure `#![no_std]` line/branch coverage algebra and `CRAP` complexity-risk metrics (`GF050`–`GF051`).

use alloc::collections::{BTreeMap, BTreeSet};
use alloc::string::String;
use serde::{Deserialize, Serialize};

use crate::diagnostic::Span;

fn ratio_percent_floor(hit: u32, found: u32) -> u32 {
    if found == 0 {
        return 100;
    }
    let num = u64::from(hit).saturating_mul(100);
    let pct = (num / u64::from(found)).min(100);
    u32::try_from(pct).unwrap_or(100)
}

/// Computes the integer `CRAP` score `comp^2 * (1 - cov)^3 + comp` from `cyclomatic` and `coverage_pct` (`0..=100`).
#[must_use]
pub fn compute_crap_score(cyclomatic: u32, coverage_pct: u32) -> u32 {
    let comp = u64::from(cyclomatic);
    let uncovered = u64::from(100_u32.saturating_sub(coverage_pct.min(100)));
    let penalty_num = comp
        .saturating_mul(comp)
        .saturating_mul(uncovered)
        .saturating_mul(uncovered)
        .saturating_mul(uncovered);
    let penalty = penalty_num.saturating_add(999_999) / 1_000_000;
    u32::try_from(penalty.saturating_add(comp)).unwrap_or(u32::MAX)
}

/// Line, branch, and per-line execution counters for a single instrumented source file.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct FileCoverage {
    /// Total instrumented lines (`LF` / distinct `DA` lines).
    pub lines_found: u32,
    /// Instrumented lines executed at least once (`LH` / `DA` with hits > 0).
    pub lines_hit: u32,
    /// Total instrumented branches (`BRF` / `BRDA` entries where `<taken> != "-"`).
    pub branches_found: u32,
    /// Instrumented branches taken at least once (`BRH` / `BRDA` with `<taken> > 0`).
    pub branches_hit: u32,
    /// Set of 1-based line numbers executed at least once (`DA:<line>,<hits>` where `hits > 0`).
    pub executed_lines: BTreeSet<u32>,
    /// Per-line execution counts (`1-based line -> hit count`) from `DA:` directives.
    pub line_hits: BTreeMap<u32, u64>,
}

impl FileCoverage {
    /// Integer percentage (`0..=100`) of instrumented lines executed in this file.
    #[must_use]
    pub fn percent_floor(&self) -> u32 {
        ratio_percent_floor(self.lines_hit, self.lines_found)
    }

    /// Integer percentage (`0..=100`) of branches taken, or `None` if `branches_found == 0`.
    #[must_use]
    pub fn branch_percent_floor(&self) -> Option<u32> {
        (self.branches_found > 0)
            .then(|| ratio_percent_floor(self.branches_hit, self.branches_found))
    }

    /// Computes line coverage percentage (`0..=100`) strictly inside `span`, or `None` if no `DA:` lines fall in `span`.
    #[must_use]
    pub fn span_line_coverage_pct(&self, span: Span) -> Option<u32> {
        let mut found = 0_u32;
        let mut hit = 0_u32;
        for (_, &count) in self.line_hits.range(span.start_line()..=span.end_line()) {
            found = found.saturating_add(1);
            if count > 0 {
                hit = hit.saturating_add(1);
            }
        }
        (found > 0).then(|| ratio_percent_floor(hit, found))
    }

    /// Computes the `CRAP` score of a callable spanning `span`, or `None` if `span` has no `DA:` entries.
    #[must_use]
    pub fn span_crap_score(&self, span: Span, cyclomatic: u32) -> Option<u32> {
        self.span_line_coverage_pct(span)
            .map(|cov_pct| compute_crap_score(cyclomatic, cov_pct))
    }
}

/// Aggregated deterministic summary of an `LCOV` coverage report.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct LcovSummary {
    /// Total instrumented lines across all records.
    pub total_lines_found: u32,
    /// Total executed lines across all records.
    pub total_lines_hit: u32,
    /// Total instrumented branches across all records.
    pub total_branches_found: u32,
    /// Total taken branches across all records.
    pub total_branches_hit: u32,
    /// Per-file coverage keyed by normalized source path.
    pub files: BTreeMap<String, FileCoverage>,
}

impl LcovSummary {
    /// Integer line-coverage percentage (`0..=100`) across the entire report.
    #[must_use]
    pub fn line_coverage_pct(&self) -> u32 {
        if self.total_lines_found == 0 {
            return 0;
        }
        ratio_percent_floor(self.total_lines_hit, self.total_lines_found)
    }

    /// Integer branch-coverage percentage (`0..=100`), or `None` if `total_branches_found == 0`.
    #[must_use]
    pub fn branch_coverage_pct(&self) -> Option<u32> {
        (self.total_branches_found > 0)
            .then(|| ratio_percent_floor(self.total_branches_hit, self.total_branches_found))
    }
}
