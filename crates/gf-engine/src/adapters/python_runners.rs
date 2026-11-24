//! Declarative `ExternalAdapter` sum type across Python, TypeScript/React, Shell,
//! PowerShell, and Docker ecosystems (`GF082`, zero `dyn Trait`).

use std::path::Path;

use gf_core::{Config, ExternalToolFinding, ExternalToolKind, ScannerPolicy};
use gf_parsers::{
    ToolParseError, parse_biome_json, parse_hadolint_json, parse_mypy_ndjson, parse_oxlint_json,
    parse_psscriptanalyzer_json, parse_pyright_json, parse_ruff_json, parse_shellcheck_json,
    parse_tsc_output,
};

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
}

const PSSA_COMMAND: &str = "& { \
    $res = if ($args.Count -gt 0) { foreach ($f in $args) { Invoke-ScriptAnalyzer -Path $f } } \
    else { Invoke-ScriptAnalyzer -Path . -Recurse }; \
    if ($null -ne $res) { $res | Select-Object RuleName,ScriptPath,Line,Column,Message,Severity | ConvertTo-Json -Compress } \
}";

struct AdapterTableEntry {
    tool: ExternalToolKind,
    prefix_args: &'static [&'static str],
    accepts_file_targets: bool,
    dot_when_empty: bool,
    install_hint: &'static str,
}

const ADAPTER_TABLE: [AdapterTableEntry; 9] = [
    AdapterTableEntry {
        tool: ExternalToolKind::Ruff,
        prefix_args: &["check", "--output-format", "json", "--no-fix", "--"],
        accepts_file_targets: true,
        dot_when_empty: true,
        install_hint: "Install `ruff` in `.venv` (`uv add --dev ruff`) or system PATH.",
    },
    AdapterTableEntry {
        tool: ExternalToolKind::Mypy,
        prefix_args: &["--strict", "--output", "json", "--no-error-summary", "--"],
        accepts_file_targets: true,
        dot_when_empty: true,
        install_hint: "Install `mypy` in `.venv` (`uv add --dev mypy`) or system PATH.",
    },
    AdapterTableEntry {
        tool: ExternalToolKind::Pyright,
        prefix_args: &["--outputjson"],
        accepts_file_targets: true,
        dot_when_empty: true,
        install_hint: "Install `pyright` in `.venv` (`uv add --dev pyright`) or system PATH.",
    },
    AdapterTableEntry {
        tool: ExternalToolKind::Oxlint,
        prefix_args: &["--format", "json"],
        accepts_file_targets: true,
        dot_when_empty: true,
        install_hint: "Install `oxlint` (`pnpm add -D oxlint`) in `node_modules/.bin` or system PATH.",
    },
    AdapterTableEntry {
        tool: ExternalToolKind::Biome,
        prefix_args: &["check", "--reporter=json"],
        accepts_file_targets: true,
        dot_when_empty: true,
        install_hint: "Install `@biomejs/biome` (`pnpm add -D @biomejs/biome`) in `node_modules/.bin` or system PATH.",
    },
    AdapterTableEntry {
        tool: ExternalToolKind::Tsc,
        prefix_args: &["--noEmit", "--pretty", "false"],
        accepts_file_targets: false,
        dot_when_empty: false,
        install_hint: "Install `typescript` (`pnpm add -D typescript`) in `node_modules/.bin` or system PATH.",
    },
    AdapterTableEntry {
        tool: ExternalToolKind::Shellcheck,
        prefix_args: &["--format=json1", "--"],
        accepts_file_targets: true,
        dot_when_empty: false,
        install_hint: "Install `shellcheck` (`apt install shellcheck` / `brew install shellcheck`) on system PATH.",
    },
    AdapterTableEntry {
        tool: ExternalToolKind::Hadolint,
        prefix_args: &["--format", "json", "--no-color"],
        accepts_file_targets: true,
        dot_when_empty: false,
        install_hint: "Install `hadolint` (`brew install hadolint` / GitHub release binary) on system PATH.",
    },
    AdapterTableEntry {
        tool: ExternalToolKind::PsScriptAnalyzer,
        prefix_args: &[
            "-NoLogo",
            "-NoProfile",
            "-NonInteractive",
            "-Command",
            PSSA_COMMAND,
            "--",
        ],
        accepts_file_targets: true,
        dot_when_empty: false,
        install_hint: "Install `pwsh` and module `PSScriptAnalyzer` (`Install-Module PSScriptAnalyzer -Scope CurrentUser`).",
    },
];

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

    /// Returns `true` for Python ecosystem adapters (`ruff`, `mypy`, `pyright`).
    #[must_use]
    pub const fn is_python(self) -> bool {
        matches!(self, Self::Ruff | Self::Mypy | Self::Pyright)
    }

    /// Returns `true` for Node/TypeScript ecosystem adapters (`oxlint`, `biome`, `tsc`).
    #[must_use]
    pub const fn is_typescript(self) -> bool {
        matches!(self, Self::Oxlint | Self::Biome | Self::Tsc)
    }

    /// Reads the configured [`ScannerPolicy`] for this adapter from `config`.
    #[must_use]
    pub const fn configured_policy(self, config: &Config) -> ScannerPolicy {
        match self {
            Self::Ruff => config.tools.python.ruff,
            Self::Mypy => config.tools.python.mypy,
            Self::Pyright => config.tools.python.pyright,
            Self::Oxlint => config.tools.typescript.oxlint,
            Self::Biome => config.tools.typescript.biome,
            Self::Tsc => config.tools.typescript.tsc,
            Self::Shellcheck => config.tools.shell.shellcheck,
            Self::Hadolint => config.tools.docker.hadolint,
            Self::PsScriptAnalyzer => config.tools.powershell.psscriptanalyzer,
        }
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
    /// Returns [`ToolParseError`] when `stdout` is malformed JSON/NDJSON.
    pub fn parse_output(
        self,
        stdout: &str,
        root: &Path,
    ) -> Result<Vec<ExternalToolFinding>, ToolParseError> {
        match self {
            Self::Ruff => parse_ruff_json(stdout, root),
            Self::Mypy => parse_mypy_ndjson(stdout, root),
            Self::Pyright => parse_pyright_json(stdout, root),
            Self::Oxlint => parse_oxlint_json(stdout, root),
            Self::Biome => parse_biome_json(stdout, root),
            Self::Tsc => parse_tsc_output(stdout, root),
            Self::Shellcheck => parse_shellcheck_json(stdout, root),
            Self::Hadolint => parse_hadolint_json(stdout, root),
            Self::PsScriptAnalyzer => parse_psscriptanalyzer_json(stdout, root),
        }
    }
}
