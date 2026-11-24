//! Unified closed algebraic sum type [`LangRuleViolation`] across all Tier 1 AST & schema
//! analyzers (`GF080`).

use core::fmt;
use serde::{Deserialize, Serialize};

use crate::python_violations::PythonViolation;
use crate::shell_ps_violations::{PowerShellViolation, ShellViolation};
use crate::ts_violations::{ReactViolation, TypescriptViolation};
use crate::yaml_violations::{ComposeViolation, WorkflowViolation, YamlViolation};

/// Unified closed sum type across all Tier 1 native language & schema violations (`GF080`).
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
pub enum LangRuleViolation {
    /// Native Python AST violation (`PY-*`).
    Python(PythonViolation),
    /// Native TypeScript/JavaScript AST violation (`TS-*`).
    TypeScript(TypescriptViolation),
    /// Native React 19 / JSX AST violation (`REACT-*`).
    React(ReactViolation),
    /// Native POSIX/Bash Shell AST violation (`SH-*`).
    Shell(ShellViolation),
    /// Native PowerShell AST violation (`PS-*`).
    PowerShell(PowerShellViolation),
    /// Native YAML 1.2 & YAML 1.1 compatibility violation (`YAML-*`).
    Yaml(YamlViolation),
    /// Native Docker Compose security & graph violation (`COMPOSE-*`).
    Compose(ComposeViolation),
    /// Native GitHub Actions workflow security violation (`GHA-*`).
    Workflow(WorkflowViolation),
}

impl LangRuleViolation {
    /// Canonical ecosystem rule code (`"PY-..."`, `"TS-..."`, `"SH-..."`, `"YAML-..."`, `"COMPOSE-..."`, `"GHA-..."`).
    #[must_use]
    pub const fn code(self) -> &'static str {
        match self {
            Self::Python(v) => v.code(),
            Self::TypeScript(v) => v.code(),
            Self::React(v) => v.code(),
            Self::Shell(v) => v.code(),
            Self::PowerShell(v) => v.code(),
            Self::Yaml(v) => v.code(),
            Self::Compose(v) => v.code(),
            Self::Workflow(v) => v.code(),
        }
    }

    /// Actionable root-cause remediation guidance.
    #[must_use]
    pub const fn remediation(self) -> &'static str {
        match self {
            Self::Python(v) => v.remediation(),
            Self::TypeScript(v) => v.remediation(),
            Self::React(v) => v.remediation(),
            Self::Shell(v) => v.remediation(),
            Self::PowerShell(v) => v.remediation(),
            Self::Yaml(v) => v.remediation(),
            Self::Compose(v) => v.remediation(),
            Self::Workflow(v) => v.remediation(),
        }
    }

    /// Returns `true` when exempted inside declared CLI/server `entrypoints`.
    #[must_use]
    pub const fn is_entrypoint_exempt(self) -> bool {
        match self {
            Self::Python(v) => v.is_entrypoint_exempt(),
            Self::TypeScript(v) => v.is_entrypoint_exempt(),
            Self::React(_)
            | Self::Shell(_)
            | Self::PowerShell(_)
            | Self::Yaml(_)
            | Self::Compose(_)
            | Self::Workflow(_) => false,
        }
    }
}

impl fmt::Display for LangRuleViolation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.code())
    }
}

impl From<PythonViolation> for LangRuleViolation {
    fn from(v: PythonViolation) -> Self {
        Self::Python(v)
    }
}

impl From<TypescriptViolation> for LangRuleViolation {
    fn from(v: TypescriptViolation) -> Self {
        Self::TypeScript(v)
    }
}

impl From<ReactViolation> for LangRuleViolation {
    fn from(v: ReactViolation) -> Self {
        Self::React(v)
    }
}

impl From<ShellViolation> for LangRuleViolation {
    fn from(v: ShellViolation) -> Self {
        Self::Shell(v)
    }
}

impl From<PowerShellViolation> for LangRuleViolation {
    fn from(v: PowerShellViolation) -> Self {
        Self::PowerShell(v)
    }
}

impl From<YamlViolation> for LangRuleViolation {
    fn from(v: YamlViolation) -> Self {
        Self::Yaml(v)
    }
}

impl From<ComposeViolation> for LangRuleViolation {
    fn from(v: ComposeViolation) -> Self {
        Self::Compose(v)
    }
}

impl From<WorkflowViolation> for LangRuleViolation {
    fn from(v: WorkflowViolation) -> Self {
        Self::Workflow(v)
    }
}
