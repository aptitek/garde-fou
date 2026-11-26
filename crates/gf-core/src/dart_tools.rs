//! Pure `#![no_std]` Tier 2 & Tier 3 configuration types for Dart 3 and Flutter 3 (`dart analyze`).

use alloc::format;
use alloc::string::String;
use serde::{Deserialize, Serialize};

use crate::external_tool_kind::ExternalToolKind;
use crate::policies::ScannerPolicy;

/// Tier 2 Dart/Flutter configuration weakening (`GF081`).
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
pub enum DartConfigIssue {
    /// `analysis_options.yaml` missing strict-casts / strict-raw-types or disabling `use_build_context_synchronously`.
    AnalysisOptionsWeakened,
    /// `pubspec.yaml` missing `environment.sdk` constraint or using unpinned `any` dependency.
    PubspecUnpinnedOrMissingSdk,
}

impl DartConfigIssue {
    /// Affected external tool.
    #[must_use]
    pub const fn tool(self) -> ExternalToolKind {
        ExternalToolKind::DartAnalyze
    }

    /// Dotted configuration key.
    #[must_use]
    pub const fn key(self) -> &'static str {
        match self {
            Self::AnalysisOptionsWeakened => "analyzer.language.strict-casts",
            Self::PubspecUnpinnedOrMissingSdk => "environment.sdk",
        }
    }

    /// Actionable root-cause remediation.
    #[must_use]
    pub const fn remediation(self) -> &'static str {
        match self {
            Self::AnalysisOptionsWeakened => {
                "Enable `strict-casts: true` and `strict-raw-types: true` in `analysis_options.yaml` and keep `use_build_context_synchronously` enabled."
            }
            Self::PubspecUnpinnedOrMissingSdk => {
                "Pin `environment.sdk` (e.g. `^3.6.0`) and replace `any` dependency versions in `pubspec.yaml`."
            }
        }
    }
}

/// `[tools.dart]` execution policy for `dart analyze --format=machine` / `--format=json` (`GF082`).
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DartToolsConfig {
    /// Policy for `dart analyze` (`auto` by default).
    pub dart_analyze: ScannerPolicy,
}

impl DartToolsConfig {
    /// Default Dart tool policy (`dart_analyze = Auto`).
    #[must_use]
    pub const fn defaults() -> Self {
        Self {
            dart_analyze: ScannerPolicy::Auto,
        }
    }

    /// Deterministic cache-key fragment.
    #[must_use]
    pub fn canonical_fragment(self) -> String {
        format!("dart({})", self.dart_analyze.as_str())
    }
}

impl Default for DartToolsConfig {
    fn default() -> Self {
        Self::defaults()
    }
}

/// Returns `true` for Dart/Flutter Tier 2 audited configs (`pubspec.yaml`, `analysis_options.yaml`).
#[must_use]
pub fn is_dart_tool_config_file(path: &str) -> bool {
    let name = path.rsplit('/').next().unwrap_or(path);
    matches!(name, "pubspec.yaml" | "analysis_options.yaml")
}

/// Returns `true` for code-generated Dart files (`*.g.dart`, `*.freezed.dart`, `*.gr.dart`, `*.mocks.dart`).
#[must_use]
pub fn is_dart_generated_file(path: &str) -> bool {
    [".g.dart", ".freezed.dart", ".gr.dart", ".mocks.dart"]
        .iter()
        .any(|ext| path.ends_with(ext))
}
