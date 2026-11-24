//! Declarative `ExternalAdapter` sum type across Python, TypeScript/React, Shell,
//! PowerShell, Docker, and YAML/Compose/CI ecosystems (`GF082`, zero `dyn Trait`).

use std::path::Path;

use gf_core::{Config, ExternalToolFinding, ExternalToolKind, ScannerPolicy};
use gf_parsers::ToolParseError;

use super::adapter_table::{ADAPTER_TABLE, AdapterTableEntry};
pub use super::tool_resolvers::{
    ResolvedPythonTool, resolve_pwsh_analyzer, resolve_python_tool, resolve_python_tool_with_uv,
    resolve_system_binary,
};

/// Closed sum type of external ecosystem verifiers orchestrated without `dyn Trait` (`GF082`).
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum ExternalAdapter {
    /// Astral `ruff check --output-format json --no-fix`.
    Ruff,
    /// `mypy --strict --output json --no-error-summary`.
    Mypy,
    /// `pyright --outputjson`.
    Pyright,
    /// `oxlint --format json`.
    Oxlint,
    /// `biome check --reporter=json`.
    Biome,
    /// `tsc --noEmit --pretty false`.
    Tsc,
    /// `shellcheck --format=json1 -- <files...>`.
    Shellcheck,
    /// `hadolint --format json --no-color <Dockerfiles...>`.
    Hadolint,
    /// `pwsh -NoLogo -NoProfile -NonInteractive -Command ... -- <files...>`.
    PsScriptAnalyzer,
    /// `yamllint -f parsable -- <files...>`.
    Yamllint,
    /// `dclint -f json <compose-files...>`.
    Dclint,
    /// `actionlint -format '{{json .}}' <workflow-files...>`.
    Actionlint,
    /// `zizmor --format sarif <workflow-files...>`.
    Zizmor,
}

impl ExternalAdapter {
    /// All active Python external adapters in deterministic execution order.
    pub const PYTHON_ALL: [Self; 3] = [Self::Ruff, Self::Mypy, Self::Pyright];

    /// All active TypeScript/React external adapters in deterministic execution order.
    pub const TYPESCRIPT_ALL: [Self; 3] = [Self::Oxlint, Self::Biome, Self::Tsc];

    const fn spec(self) -> &'static AdapterTableEntry {
        &ADAPTER_TABLE[self as usize]
    }

    /// Canonical [`ExternalToolKind`] corresponding to this adapter.
    #[must_use]
    pub const fn tool_kind(self) -> ExternalToolKind {
        self.spec().tool
    }

    /// Returns `true` for tools resolvable via `.venv` / `uv` (`ruff`, `mypy`, `pyright`, `yamllint`, `zizmor`).
    #[must_use]
    pub const fn is_python(self) -> bool {
        matches!(
            self,
            Self::Ruff | Self::Mypy | Self::Pyright | Self::Yamllint | Self::Zizmor
        )
    }

    /// Returns `true` for Node/npm CLI adapters (`oxlint`, `biome`, `tsc`, `dclint`).
    #[must_use]
    pub const fn is_typescript(self) -> bool {
        matches!(self, Self::Oxlint | Self::Biome | Self::Tsc | Self::Dclint)
    }

    /// Reads the configured [`ScannerPolicy`] for this adapter from `config`.
    #[must_use]
    pub fn configured_policy(self, config: &Config) -> ScannerPolicy {
        (self.spec().get_policy)(config)
    }

    /// Actionable install guidance emitted in `GF043` when required and absent.
    #[must_use]
    pub const fn install_hint(self) -> &'static str {
        self.spec().install_hint
    }

    /// Builds argv slice targeting `targets` (zero shell interpolation).
    #[must_use]
    pub fn build_args(self, targets: &[String]) -> Vec<String> {
        let entry = self.spec();
        let mut args: Vec<String> = entry
            .prefix_args
            .iter()
            .copied()
            .map(String::from)
            .collect();
        if entry.accepts_file_targets {
            if targets.is_empty() {
                if entry.dot_when_empty {
                    args.push(String::from("."));
                }
            } else {
                args.extend(targets.iter().cloned());
            }
        }
        args
    }

    /// Parses raw `stdout` into normalized [`ExternalToolFinding`]s.
    ///
    /// # Errors
    ///
    /// Returns [`ToolParseError`] when `stdout` is malformed JSON/SARIF.
    pub fn parse_output(
        self,
        stdout: &str,
        root: &Path,
    ) -> Result<Vec<ExternalToolFinding>, ToolParseError> {
        (self.spec().parse_fn)(stdout, root)
    }
}
