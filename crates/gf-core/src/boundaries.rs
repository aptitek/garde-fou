//! Integrity lock policies (`garde.lock`, `GF070`) and declarative `[[boundaries]]` (`GF072`),
//! including the default Atomic Design hierarchy (`atoms -> molecules -> organisms -> templates -> pages`).

use alloc::format;
use alloc::string::String;
use alloc::vec;
use alloc::vec::Vec;
use serde::{Deserialize, Serialize};

/// Integrity lock policy (`[integrity]` in `garde-fou.toml`).
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct IntegrityConfig {
    /// Require a cryptographically matching `garde.lock` alongside `garde-fou.toml`.
    pub require_lock: bool,
}

impl IntegrityConfig {
    /// Returns the default integrity policy (`require_lock = false`).
    #[must_use]
    pub const fn defaults() -> Self {
        Self {
            require_lock: false,
        }
    }
}

/// Verification state of `garde.lock` relative to `garde-fou.toml` (`GF070`).
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub enum LockStatus {
    /// Neither `garde.lock` nor `require_lock = true` is active.
    #[default]
    NotRequired,
    /// `garde.lock` exists and its `BLAKE3` digest matches `garde-fou.toml`.
    Verified,
    /// `require_lock = true` is active, but `garde.lock` is missing from the workspace root.
    Missing,
    /// `garde.lock` exists, but `garde-fou.toml` was modified without running `garde lock`.
    Tampered {
        /// Digest recorded inside `garde.lock`.
        expected: String,
        /// Actual digest computed from `garde-fou.toml`.
        actual: String,
    },
}

/// Single architectural import boundary rule (`[[boundaries]]` in `garde-fou.toml`, `GF072`).
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BoundaryRule {
    /// Glob matching source files subject to this boundary (e.g. `"crates/gf-core/**"`).
    pub from: String,
    /// Optional closed allow-list of permitted import targets/globs (empty = allow all not denied).
    pub allow: Vec<String>,
    /// Forbidden target globs or module/crate identifiers.
    pub deny: Vec<String>,
    /// Human/agent rationale surfaced in `GF072` remediation output.
    pub reason: String,
}

impl BoundaryRule {
    /// Deterministic string fragment included in `Config::canonical_string`.
    #[must_use]
    pub fn canonical_fragment(&self) -> String {
        format!(
            "({}=>+{:?}-{:?}:{})",
            self.from, self.allow, self.deny, self.reason
        )
    }
}

fn s_vec(items: &[&str]) -> Vec<String> {
    items.iter().map(|&s| String::from(s)).collect()
}

/// Built-in Atomic Design import hierarchy activated when `FeatureId::Ui` is active (`GF072`).
#[must_use]
pub fn default_atomic_design_boundaries() -> Vec<BoundaryRule> {
    vec![
        BoundaryRule {
            from: String::from("**/atoms/**"),
            allow: Vec::new(),
            deny: s_vec(&[
                "**/molecules/**",
                "**/organisms/**",
                "**/templates/**",
                "**/pages/**",
                "**/routes/**",
                "**/api/**",
                "**/services/**",
                "**/store/**",
            ]),
            reason: String::from(
                "Atomic Design: `atoms` must remain stateless leaf UI primitives without importing `molecules`, `organisms`, `templates`, `pages`, or `api`/`services`/`store`.",
            ),
        },
        BoundaryRule {
            from: String::from("**/molecules/**"),
            allow: Vec::new(),
            deny: s_vec(&[
                "**/organisms/**",
                "**/templates/**",
                "**/pages/**",
                "**/routes/**",
                "**/api/**",
                "**/services/**",
            ]),
            reason: String::from(
                "Atomic Design: `molecules` compose `atoms` only and must not import `organisms`, `templates`, `pages`, or `api`/`services`.",
            ),
        },
        BoundaryRule {
            from: String::from("**/organisms/**"),
            allow: Vec::new(),
            deny: s_vec(&["**/templates/**", "**/pages/**", "**/routes/**"]),
            reason: String::from(
                "Atomic Design: `organisms` must not depend upward on `templates`, `pages`, or `routes`.",
            ),
        },
    ]
}
