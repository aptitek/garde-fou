//! Mutation-testing configuration issue enums (`mutants.toml` & `stryker.config.json`, `GF081`).

use serde::{Deserialize, Serialize};

/// Specific `mutants.toml` scope sabotage (`GF081`).
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
pub enum MutantsConfigIssue {
    /// `examine_globs = []`.
    EmptyExamineGlobs,
    /// `examine_re = []`.
    EmptyExamineRe,
    /// Broad wildcard `exclude_globs` entry (`*`, `**`, `src/**`, `crates/**`).
    BroadExcludeGlobs,
}

impl MutantsConfigIssue {
    /// Canonical key inside `mutants.toml`.
    #[must_use]
    pub const fn key(self) -> &'static str {
        match self {
            Self::EmptyExamineGlobs => "examine_globs",
            Self::EmptyExamineRe => "examine_re",
            Self::BroadExcludeGlobs => "exclude_globs",
        }
    }
}

/// Specific `stryker.config.json` mutation gate weakening (`GF081`).
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
pub enum StrykerConfigIssue {
    /// `"mutate": []`.
    EmptyMutate,
    /// `"thresholds.break": 0` or `null`.
    DisabledBreakThreshold,
}

impl StrykerConfigIssue {
    /// Canonical key inside `stryker.config.json`.
    #[must_use]
    pub const fn key(self) -> &'static str {
        match self {
            Self::EmptyMutate => "mutate",
            Self::DisabledBreakThreshold => "thresholds.break",
        }
    }
}
