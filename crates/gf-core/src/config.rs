//! Configuration and Single Responsibility Principle (SRP) threshold types.

/// Quantitative SRP thresholds enforced on analyzed source files.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LimitsConfig {
    /// Maximum number of lines allowed in a single source file.
    pub max_file_lines: u32,
    /// Maximum number of lines allowed in a single function body.
    pub max_function_lines: u32,
    /// Maximum cognitive complexity allowed per function.
    pub max_cognitive_complexity: u32,
    /// Maximum parameter count allowed per function signature.
    pub max_function_params: u32,
}

impl LimitsConfig {
    /// Returns Garde-fou's strict dogfooding limits.
    #[must_use]
    pub const fn strict_defaults() -> Self {
        Self {
            max_file_lines: 250,
            max_function_lines: 50,
            max_cognitive_complexity: 10,
            max_function_params: 4,
        }
    }
}

impl Default for LimitsConfig {
    fn default() -> Self {
        Self::strict_defaults()
    }
}

/// Top-level pure analyzer configuration consumed by `gf-rules`.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct Config {
    /// Quantitative SRP thresholds.
    pub limits: LimitsConfig,
    /// Whether forbidden constructs (`unwrap`, `panic!`, `unsafe`, etc.) are denied.
    pub forbid_panicking_constructs: bool,
}

impl Config {
    /// Returns Garde-fou's strict default configuration.
    #[must_use]
    pub const fn strict_defaults() -> Self {
        Self {
            limits: LimitsConfig::strict_defaults(),
            forbid_panicking_constructs: true,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{Config, LimitsConfig};

    #[test]
    fn strict_defaults_match_architectural_invariants() {
        let cfg = Config::strict_defaults();
        assert_eq!(cfg.limits, LimitsConfig::default());
        assert_eq!(cfg.limits.max_file_lines, 250);
        assert_eq!(cfg.limits.max_function_lines, 50);
        assert_eq!(cfg.limits.max_cognitive_complexity, 10);
        assert_eq!(cfg.limits.max_function_params, 4);
        assert!(cfg.forbid_panicking_constructs);
        assert_eq!(Config::default().limits, cfg.limits);
    }
}
