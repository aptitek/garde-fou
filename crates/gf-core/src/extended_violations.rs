//! Grouped extended ecosystem Tier 1 violations (`GF080`) keeping `lang_rule_violation.rs`
//! within strict fan-out (`GF007`) and complexity (`GF005`) budgets.

use core::fmt;
use serde::{Deserialize, Serialize};

use crate::dart_flutter_violations::{DartFlutterViolation, DartViolation, FlutterViolation};
use crate::db_procedural_violations::{
    DbProceduralViolation, MariaDbViolation, PostgresAdvViolation, ProceduralSqlViolation,
};
use crate::diagrams_i18n_violations::{
    DiagramI18nViolation, I18nViolation, MermaidViolation, PlantUmlViolation,
};
use crate::php_symfony_violations::{
    PhpSymfonyViolation, PhpViolation, SymfonyViolation, TwigViolation,
};
use crate::py_ecosystems_violations::{
    DjangoViolation, LlmAgentViolation, MlDlViolation, NiceGuiViolation, PyEcosystemViolation,
};

/// Unified closed sum type for extended ecosystem language & framework violations (`GF080`).
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
pub enum ExtendedLangViolation {
    /// Python ecosystems (`DJ-*`, `NICEGUI-*`, `ML-*`, `LLM-*`).
    PyEcosystem(PyEcosystemViolation),
    /// MariaDB, advanced `PostgreSQL`/PL/pgSQL, and Oracle PL/SQL (`MARIADB-*`, `PG-*`, `PLSQL-*`).
    DbProcedural(DbProceduralViolation),
    /// Mermaid v11, `PlantUML`, and universal `i18n` v2 (`MERMAID-*`, `PLANTUML-*`, `I18N-*`).
    DiagramI18n(DiagramI18nViolation),
    /// PHP 8.4, Symfony 7, Doctrine ORM 3, and Twig 3 (`PHP-*`, `SYMFONY-*`, `TWIG-*`).
    PhpSymfony(PhpSymfonyViolation),
    /// Dart 3.6+ and Flutter 3.x (`DART-*`, `FLUTTER-*`).
    DartFlutter(DartFlutterViolation),
}

impl ExtendedLangViolation {
    /// Canonical rule code.
    #[must_use]
    pub const fn code(self) -> &'static str {
        match self {
            Self::PyEcosystem(v) => v.code(),
            Self::DbProcedural(v) => v.code(),
            Self::DiagramI18n(v) => v.code(),
            Self::PhpSymfony(v) => v.code(),
            Self::DartFlutter(v) => v.code(),
        }
    }

    /// Actionable root-cause remediation.
    #[must_use]
    pub const fn remediation(self) -> &'static str {
        match self {
            Self::PyEcosystem(v) => v.remediation(),
            Self::DbProcedural(v) => v.remediation(),
            Self::DiagramI18n(v) => v.remediation(),
            Self::PhpSymfony(v) => v.remediation(),
            Self::DartFlutter(v) => v.remediation(),
        }
    }
}

impl fmt::Display for ExtendedLangViolation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.code())
    }
}

macro_rules! impl_ext_direct {
    ($($ty:ty => $v:ident),+ $(,)?) => {
        $(impl From<$ty> for ExtendedLangViolation {
            fn from(x: $ty) -> Self { Self::$v(x) }
        })+
    };
}

macro_rules! impl_ext_nested {
    ($outer:ident, $inner:ident; $($ty:ty => $v:ident),+ $(,)?) => {
        $(impl From<$ty> for ExtendedLangViolation {
            fn from(x: $ty) -> Self { Self::$outer($inner::$v(x)) }
        })+
    };
}

impl_ext_direct!(
    PyEcosystemViolation => PyEcosystem,
    DbProceduralViolation => DbProcedural,
    DiagramI18nViolation => DiagramI18n,
    PhpSymfonyViolation => PhpSymfony,
    DartFlutterViolation => DartFlutter,
);
impl_ext_nested!(
    PyEcosystem, PyEcosystemViolation;
    DjangoViolation => Django,
    NiceGuiViolation => NiceGui,
    MlDlViolation => MlDl,
    LlmAgentViolation => LlmAgent,
);
impl_ext_nested!(
    DbProcedural, DbProceduralViolation;
    MariaDbViolation => MariaDb,
    PostgresAdvViolation => PostgresAdv,
    ProceduralSqlViolation => ProceduralSql,
);
impl_ext_nested!(
    DiagramI18n, DiagramI18nViolation;
    MermaidViolation => Mermaid,
    PlantUmlViolation => PlantUml,
    I18nViolation => I18n,
);
impl_ext_nested!(
    PhpSymfony, PhpSymfonyViolation;
    PhpViolation => Php,
    SymfonyViolation => Symfony,
    TwigViolation => Twig,
);
impl_ext_nested!(
    DartFlutter, DartFlutterViolation;
    DartViolation => Dart,
    FlutterViolation => Flutter,
);
