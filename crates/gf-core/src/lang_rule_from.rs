//! `From<T> for LangRuleViolation` trait implementations for extended ecosystems,
//! isolated in a DAG sink module to keep `lang_rule_violation.rs` acyclic (`GF071`) and low fan-out (`GF007`).

use crate::db_procedural_violations::{
    DbProceduralViolation, MariaDbViolation, PostgresAdvViolation, ProceduralSqlViolation,
};
use crate::diagrams_i18n_violations::{
    DiagramI18nViolation, I18nViolation, MermaidViolation, PlantUmlViolation,
};
use crate::extended_violations::ExtendedLangViolation;
use crate::lang_rule_violation::LangRuleViolation;
use crate::php_symfony_violations::{
    PhpSymfonyViolation, PhpViolation, SymfonyViolation, TwigViolation,
};
use crate::py_ecosystems_violations::{
    DjangoViolation, LlmAgentViolation, MlDlViolation, NiceGuiViolation, PyEcosystemViolation,
};

macro_rules! impl_from_ext {
    ($($ty:ty),+ $(,)?) => {
        $(impl From<$ty> for LangRuleViolation {
            fn from(x: $ty) -> Self { Self::Extended(ExtendedLangViolation::from(x)) }
        })+
    };
}

impl_from_ext!(
    PyEcosystemViolation,
    DjangoViolation,
    NiceGuiViolation,
    MlDlViolation,
    LlmAgentViolation,
    DbProceduralViolation,
    MariaDbViolation,
    PostgresAdvViolation,
    ProceduralSqlViolation,
    DiagramI18nViolation,
    MermaidViolation,
    PlantUmlViolation,
    I18nViolation,
    PhpSymfonyViolation,
    PhpViolation,
    SymfonyViolation,
    TwigViolation,
);
