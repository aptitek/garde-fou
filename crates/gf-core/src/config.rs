//! Configuration and Single Responsibility Principle (SRP) threshold types.

use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;
use serde::{Deserialize, Serialize};

/// Quantitative SRP and complexity thresholds enforced on analyzed source files.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LimitsConfig {
    /// Maximum number of lines allowed in a single source file.
    pub max_file_lines: u32,
    /// Maximum number of effective (non-blank) lines allowed per function or method.
    pub max_function_lines: u32,
    /// Maximum parameter count allowed per callable signature.
    pub max_parameters: u32,
    /// Maximum `McCabe` cyclomatic complexity allowed per callable.
    pub max_cyclomatic_complexity: u32,
    /// Maximum cognitive complexity (including nesting penalty) allowed per callable.
    pub max_cognitive_complexity: u32,
    /// Maximum control-flow nesting depth allowed per callable.
    pub max_nesting_depth: u32,
    /// Maximum import declarations allowed in a single file (module fan-out).
    pub max_imports_per_file: u32,
    /// Maximum methods allowed on a single class, trait, or `impl` block.
    pub max_methods_per_type: u32,
}

impl LimitsConfig {
    /// Returns Garde-fou's strict zero-debt dogfooding limits.
    #[must_use]
    pub const fn strict_defaults() -> Self {
        Self {
            max_file_lines: 250,
            max_function_lines: 50,
            max_parameters: 4,
            max_cyclomatic_complexity: 10,
            max_cognitive_complexity: 10,
            max_nesting_depth: 4,
            max_imports_per_file: 15,
            max_methods_per_type: 10,
        }
    }
}

impl Default for LimitsConfig {
    fn default() -> Self {
        Self::strict_defaults()
    }
}

/// Top-level pure analyzer configuration consumed by `gf-rules`.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Config {
    /// Quantitative SRP and complexity thresholds.
    pub limits: LimitsConfig,
    /// Whether forbidden constructs (`unwrap`, `panic!`, `unsafe`, etc.) are denied.
    pub forbid_panicking_constructs: bool,
    /// Explicit repository-relative glob patterns excluded from analysis.
    pub ignore_paths: Vec<String>,
    /// Explicit repository-relative glob patterns marking binary/library entrypoints.
    pub entrypoints: Vec<String>,
}

impl Config {
    /// Returns Garde-fou's strict default configuration with zero implicit exclusions.
    #[must_use]
    pub const fn strict_defaults() -> Self {
        Self {
            limits: LimitsConfig::strict_defaults(),
            forbid_panicking_constructs: true,
            ignore_paths: Vec::new(),
            entrypoints: Vec::new(),
        }
    }

    /// Produces a deterministic canonical representation suitable for `BLAKE3` cache keys.
    #[must_use]
    pub fn canonical_string(&self) -> String {
        let mut ignores = self.ignore_paths.clone();
        ignores.sort();
        let mut entries = self.entrypoints.clone();
        entries.sort();
        let l = self.limits;
        format!(
            "v2|fl={}|fnl={}|p={}|cyc={}|cog={}|nest={}|imp={}|meth={}|forbid={}|ign={}|ep={}",
            l.max_file_lines,
            l.max_function_lines,
            l.max_parameters,
            l.max_cyclomatic_complexity,
            l.max_cognitive_complexity,
            l.max_nesting_depth,
            l.max_imports_per_file,
            l.max_methods_per_type,
            self.forbid_panicking_constructs,
            ignores.join(","),
            entries.join(","),
        )
    }
}

impl Default for Config {
    fn default() -> Self {
        Self::strict_defaults()
    }
}

#[cfg(test)]
mod tests {
    use alloc::string::String;
    use alloc::vec;

    use super::{Config, LimitsConfig};

    #[test]
    fn strict_defaults_and_canonical_string_are_order_independent() {
        let cfg = Config::strict_defaults();
        assert_eq!(cfg, Config::default());
        assert_eq!(cfg.limits, LimitsConfig::default());
        assert_eq!(cfg.limits.max_file_lines, 250);
        assert_eq!(cfg.limits.max_parameters, 4);
        assert_eq!(cfg.limits.max_imports_per_file, 15);
        assert_eq!(cfg.limits.max_methods_per_type, 10);

        let mut a = Config::strict_defaults();
        a.ignore_paths = vec![String::from("b/**"), String::from("a/**")];
        let mut b = Config::strict_defaults();
        b.ignore_paths = vec![String::from("a/**"), String::from("b/**")];
        assert_eq!(a.canonical_string(), b.canonical_string());
    }
}
