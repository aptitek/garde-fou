//! Unified closed algebraic sum type [`LangRuleViolation`] across all Tier 1 AST & schema
//! analyzers (`GF080`).

use core::fmt;
use serde::{Deserialize, Serialize};

use crate::academic_violations::{
    AcademicViolation, NotebookViolation, ObsidianViolation, QuartoViolation,
};
use crate::compiled_violations::CompiledLangViolation;
use crate::format_violations::{TomlViolation, XmlSvgViolation};
use crate::python_violations::PythonViolation;
use crate::shell_ps_violations::{PowerShellViolation, ShellViolation};
use crate::sql_violations::{MigrationViolation, SqlViolation};
use crate::ts_violations::{ReactViolation, TypescriptViolation};
use crate::web_edge_tools::{CloudflareViolation, DrizzleViolation, WebEdgeViolation};
use crate::web_edge_violations::{
    A11yViolation, AstroViolation, M3eViolation, PandaViolation, ReactRouterViolation, W3cViolation,
};
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
    /// Native XML DTD/XXE & package manifest violation (`XML-*`).
    XmlSvg(XmlSvgViolation),
    /// Native TOML 1.0/1.1 & manifest supply-chain violation (`TOML-*`).
    Toml(TomlViolation),
    /// Native SQL AST & embedded SQL injection violation (`SQL-*`).
    Sql(SqlViolation),
    /// Native zero-downtime database migration violation (`MIG-*`).
    Migration(MigrationViolation),
    /// Native Web, Design System (`Panda`/`M3E`), W3C/WCAG, Cloudflare, and Drizzle violation.
    WebEdge(WebEdgeViolation),
    /// Native Jupyter, Quarto, or Obsidian violation (`NB-*`, `QUARTO-*`, `OBS-*`).
    Academic(AcademicViolation),
    /// Native Go, Kotlin, or C# AST violation (`GO-*`, `KT-*`, `CS-*`).
    Compiled(CompiledLangViolation),
}

impl LangRuleViolation {
    const fn yaml_or_format_descriptor(self) -> Option<(&'static str, &'static str)> {
        match self {
            Self::Yaml(v) => Some((v.code(), v.remediation())),
            Self::Compose(v) => Some((v.code(), v.remediation())),
            Self::Workflow(v) => Some((v.code(), v.remediation())),
            Self::XmlSvg(v) => Some((v.code(), v.remediation())),
            Self::Toml(v) => Some((v.code(), v.remediation())),
            Self::Academic(v) => Some((v.code(), v.remediation())),
            Self::Compiled(v) => Some((v.code(), v.remediation())),
            _ => None,
        }
    }

    const fn descriptor(self) -> (&'static str, &'static str) {
        if let Some(pair) = self.yaml_or_format_descriptor() {
            return pair;
        }
        match self {
            Self::Python(v) => (v.code(), v.remediation()),
            Self::TypeScript(v) => (v.code(), v.remediation()),
            Self::React(v) => (v.code(), v.remediation()),
            Self::Shell(v) => (v.code(), v.remediation()),
            Self::PowerShell(v) => (v.code(), v.remediation()),
            Self::Sql(v) => (v.code(), v.remediation()),
            Self::Migration(v) => (v.code(), v.remediation()),
            Self::WebEdge(v) => (v.code(), v.remediation()),
            Self::Yaml(_)
            | Self::Compose(_)
            | Self::Workflow(_)
            | Self::XmlSvg(_)
            | Self::Toml(_)
            | Self::Academic(_)
            | Self::Compiled(_) => ("", ""),
        }
    }

    /// Canonical ecosystem rule code (`"PY-..."`, `"TS-..."`, `"SH-..."`, `"YAML-..."`, `"SQL-..."`, `"MIG-..."`).
    #[must_use]
    pub const fn code(self) -> &'static str {
        self.descriptor().0
    }

    /// Actionable root-cause remediation guidance.
    #[must_use]
    pub const fn remediation(self) -> &'static str {
        self.descriptor().1
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
            | Self::Workflow(_)
            | Self::XmlSvg(_)
            | Self::Toml(_)
            | Self::Sql(_)
            | Self::Migration(_)
            | Self::WebEdge(_)
            | Self::Academic(_)
            | Self::Compiled(_) => false,
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

impl From<XmlSvgViolation> for LangRuleViolation {
    fn from(v: XmlSvgViolation) -> Self {
        Self::XmlSvg(v)
    }
}

impl From<TomlViolation> for LangRuleViolation {
    fn from(v: TomlViolation) -> Self {
        Self::Toml(v)
    }
}

impl From<SqlViolation> for LangRuleViolation {
    fn from(v: SqlViolation) -> Self {
        Self::Sql(v)
    }
}

macro_rules! impl_from_direct {
    ($($ty:ty => $variant:ident),+ $(,)?) => {
        $(impl From<$ty> for LangRuleViolation { fn from(v: $ty) -> Self { Self::$variant(v) } })+
    };
}

impl_from_direct!(
    MigrationViolation => Migration,
    WebEdgeViolation => WebEdge,
    AcademicViolation => Academic,
    CompiledLangViolation => Compiled,
);

macro_rules! impl_from_nested {
    ($outer:ident, $inner:ident; $($ty:ty => $variant:ident),+ $(,)?) => {
        $(impl From<$ty> for LangRuleViolation { fn from(v: $ty) -> Self { Self::$outer($inner::$variant(v)) } })+
    };
}

impl_from_nested!(
    WebEdge, WebEdgeViolation;
    AstroViolation => Astro, ReactRouterViolation => ReactRouter,
    PandaViolation => Panda, M3eViolation => M3e, W3cViolation => W3c,
    A11yViolation => A11y, CloudflareViolation => Cloudflare, DrizzleViolation => Drizzle,
);
impl_from_nested!(
    Academic, AcademicViolation;
    NotebookViolation => Notebook, QuartoViolation => Quarto, ObsidianViolation => Obsidian,
);
impl_from_nested!(
    Compiled, CompiledLangViolation;
    crate::compiled_violations::GoViolation => Go,
    crate::compiled_violations::KotlinViolation => Kotlin,
    crate::compiled_violations::CSharpViolation => CSharp,
);
