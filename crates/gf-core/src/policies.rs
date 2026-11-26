//! Pure `#![no_std]` policies for `[rules]` (`GF010`–`GF011`), `[architecture]` (`GF078`),
//! `[scanners]` (`GF041`–`GF043`), and `[coverage]` (`LCOV` + `CRAP`, `GF050`–`GF051`).

use alloc::collections::BTreeSet;
use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;
use core::fmt;
use serde::{Deserialize, Serialize};

pub use crate::boundaries::ArchitectureConfig;

/// Individually configurable forbidden construct under `[rules]`.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
pub enum ForbiddenRuleKind {
    /// `unsafe` blocks, functions, traits, or `impl`s (`GF010`).
    Unsafe,
    /// `.unwrap()` calls on `Option` or `Result` (`GF010`).
    Unwrap,
    /// `.expect(...)` calls (`GF010`).
    Expect,
    /// `panic!(...)` and `unreachable!(...)` macros (`GF010`).
    Panic,
    /// `todo!(...)` macro (`GF010`).
    Todo,
    /// `unimplemented!(...)` macro (`GF010`).
    Unimplemented,
    /// `dbg!(...)` macro (`GF010`).
    Dbg,
    /// Silent error swallowing: `.unwrap_or_default()` on error, `let _ = fallible()`,
    /// `except Exception: return ...` (`GF011`).
    SilentErrorSwallow,
}

impl ForbiddenRuleKind {
    /// All 8 forbidden construct categories in deterministic order.
    pub const ALL: [Self; 8] = [
        Self::Unsafe,
        Self::Unwrap,
        Self::Expect,
        Self::Panic,
        Self::Todo,
        Self::Unimplemented,
        Self::Dbg,
        Self::SilentErrorSwallow,
    ];

    /// Canonical `snake_case` identifier used in diagnostics and cache keys.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Unsafe => "unsafe",
            Self::Unwrap => "unwrap",
            Self::Expect => "expect",
            Self::Panic => "panic",
            Self::Todo => "todo",
            Self::Unimplemented => "unimplemented",
            Self::Dbg => "dbg",
            Self::SilentErrorSwallow => "silent_error_swallow",
        }
    }
}

impl fmt::Display for ForbiddenRuleKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Granular `[rules]` set of active forbidden constructs (`GF010` & `GF011`, zero booleans).
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ForbiddenRulesConfig {
    /// Deterministic set of enabled [`ForbiddenRuleKind`] checks.
    pub active: BTreeSet<ForbiddenRuleKind>,
}

impl ForbiddenRulesConfig {
    /// Strict defaults enabling all 8 [`ForbiddenRuleKind`] checks.
    #[must_use]
    pub fn strict_defaults() -> Self {
        Self {
            active: BTreeSet::from(ForbiddenRuleKind::ALL),
        }
    }

    /// Empty rule set with all [`ForbiddenRuleKind`] checks disabled.
    #[must_use]
    pub const fn empty() -> Self {
        Self {
            active: BTreeSet::new(),
        }
    }

    /// Returns `true` when `kind` is enforced under `[rules]`.
    #[must_use]
    pub fn contains(&self, kind: ForbiddenRuleKind) -> bool {
        self.active.contains(&kind)
    }

    /// Inserts or removes `kind` according to `enabled`.
    pub fn set_enabled(&mut self, kind: ForbiddenRuleKind, enabled: bool) {
        if enabled {
            self.active.insert(kind);
        } else {
            self.active.remove(&kind);
        }
    }

    /// Deterministic fragment for `BLAKE3` cache keys.
    #[must_use]
    pub fn canonical_fragment(&self) -> String {
        let names: Vec<&str> = self.active.iter().map(|k| k.as_str()).collect();
        format!("rl({})", names.join(","))
    }
}

impl Default for ForbiddenRulesConfig {
    fn default() -> Self {
        Self::strict_defaults()
    }
}

/// Policy controlling orchestration of an external security scanner or verifier.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub enum ScannerPolicy {
    /// Execute if the binary is found in `PATH`, skip silently otherwise.
    #[default]
    Auto,
    /// Fail immediately with blocking `GF043` if the binary is absent from `PATH`.
    Required,
    /// Explicitly disable this external scanner.
    Off,
}

impl ScannerPolicy {
    /// Parses a case-insensitive scanner policy (`"auto"`, `"required"`, `"off"`).
    #[must_use]
    pub const fn from_config_str(raw: &str) -> Option<Self> {
        if raw.eq_ignore_ascii_case("auto") {
            Some(Self::Auto)
        } else if raw.eq_ignore_ascii_case("required") {
            Some(Self::Required)
        } else if raw.eq_ignore_ascii_case("off") || raw.eq_ignore_ascii_case("false") {
            Some(Self::Off)
        } else {
            None
        }
    }

    /// Canonical lowercase label.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Auto => "auto",
            Self::Required => "required",
            Self::Off => "off",
        }
    }
}

/// External scanner policies configured under `[scanners]`.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ScannersConfig {
    /// Policy for `gitleaks` (`GF041`).
    pub secrets: ScannerPolicy,
    /// Policy for `osv-scanner` (`GF042`).
    pub vulns: ScannerPolicy,
}

impl ScannersConfig {
    /// Default scanner configuration (`secrets = "auto"`, `vulns = "auto"`).
    #[must_use]
    pub const fn defaults() -> Self {
        Self {
            secrets: ScannerPolicy::Auto,
            vulns: ScannerPolicy::Auto,
        }
    }
}

/// Universal `LCOV` line/branch coverage and `CRAP` score gate under `[coverage]` (`GF050`–`GF051`).
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CoverageConfig {
    /// Minimum required line-coverage percentage (`0..=100`), enforced by `GF050`.
    pub min_line_coverage: Option<u32>,
    /// Minimum required `BRDA` branch-coverage percentage (`0..=100`), enforced by `GF050`.
    pub min_branch_coverage: Option<u32>,
    /// Maximum allowed `CRAP` score per callable (e.g. `Some(30)`), enforced by `GF051`.
    pub max_crap_score: Option<u32>,
    /// Repository-relative path to the `LCOV` file (defaults to `"lcov.info"` when active).
    pub lcov_path: Option<String>,
}

impl CoverageConfig {
    /// Empty coverage gate (disabled unless a threshold or `--lcov` is configured).
    #[must_use]
    pub const fn empty() -> Self {
        Self {
            min_line_coverage: None,
            min_branch_coverage: None,
            max_crap_score: None,
            lcov_path: None,
        }
    }

    /// Deterministic fragment for `BLAKE3` cache keys.
    #[must_use]
    pub fn canonical_fragment(&self) -> String {
        format!(
            "cov(l={:?},b={:?},cr={:?},p={:?})",
            self.min_line_coverage, self.min_branch_coverage, self.max_crap_score, self.lcov_path
        )
    }
}
