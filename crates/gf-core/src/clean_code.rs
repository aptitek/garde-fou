//! KISS, YAGNI, Anti-Bypass (`GF070`, `GF073`–`GF075`), and TDD Fortress (`GF076`)
//! configuration and `UAST` projection types.

use alloc::format;
use alloc::string::String;
use serde::{Deserialize, Serialize};

use crate::diagnostic::Span;

/// Boolean YAGNI gates grouped to keep every struct `<= 2` booleans.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct YagniGates {
    /// Forbid barrel wildcard re-exports (`export *`, `from x import *`, `pub use *`).
    pub forbid_wildcard_exports: bool,
    /// Forbid behavioral `trait`/`interface`/`Protocol` contracts with `<= 1` concrete impl.
    pub forbid_single_impl_traits: bool,
}

impl YagniGates {
    /// Strict defaults (`true` for both YAGNI gates).
    #[must_use]
    pub const fn strict_defaults() -> Self {
        Self {
            forbid_wildcard_exports: true,
            forbid_single_impl_traits: true,
        }
    }
}

/// `[clean_code]` configuration governing KISS (`GF073`) and YAGNI (`GF074`–`GF075`).
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CleanCodeConfig {
    /// Maximum allowed nested ternary depth (`0` forbids nested ternaries).
    pub max_nested_ternaries: u32,
    /// Maximum chained field/property segments before violating Law of Demeter (`a.b.c.d` = `4`).
    pub max_property_chain: u32,
    /// Maximum `bool`/`boolean` parameters allowed on a callable signature (`1`).
    pub max_bool_parameters: u32,
    /// Forbid `TODO`, `FIXME`, `HACK`, `XXX` comments and commented-out code in `Code`.
    pub forbid_todos: bool,
    /// Structural YAGNI toggles (`export *` and single-implementation traits).
    pub yagni: YagniGates,
}

impl CleanCodeConfig {
    /// Strict zero-compromise KISS & YAGNI defaults.
    #[must_use]
    pub const fn strict_defaults() -> Self {
        Self {
            max_nested_ternaries: 0,
            max_property_chain: 4,
            max_bool_parameters: 1,
            forbid_todos: true,
            yagni: YagniGates::strict_defaults(),
        }
    }

    /// Deterministic fragment for `BLAKE3` cache keys.
    #[must_use]
    pub fn canonical_fragment(&self) -> String {
        format!(
            "cc(nt={},pc={},bp={},td={},wc={},si={})",
            self.max_nested_ternaries,
            self.max_property_chain,
            self.max_bool_parameters,
            self.forbid_todos,
            self.yagni.forbid_wildcard_exports,
            self.yagni.forbid_single_impl_traits,
        )
    }
}

impl Default for CleanCodeConfig {
    fn default() -> Self {
        Self::strict_defaults()
    }
}

/// Companion file parity gates grouped for `[tdd]` (`<= 2` booleans).
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TddParityGates {
    /// Require 1:1 unit test coverage (inline tests or companion test file) per `Code` module.
    pub require_companion: bool,
    /// Require `.stories.tsx` companion per Atomic Design UI component.
    pub require_stories: bool,
}

/// `[tdd]` configuration governing assertion integrity and companion parity (`GF076`).
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TddConfig {
    /// Block tests with zero assertions or tautological assertions (`assert!(true)`).
    pub forbid_assertion_free_tests: bool,
    /// Optional 1:1 companion test and `.stories` gates.
    pub parity: TddParityGates,
}

impl TddConfig {
    /// Strict default TDD configuration (`forbid_assertion_free_tests = true`).
    #[must_use]
    pub const fn strict_defaults() -> Self {
        Self {
            forbid_assertion_free_tests: true,
            parity: TddParityGates {
                require_companion: false,
                require_stories: false,
            },
        }
    }

    /// Deterministic fragment for `BLAKE3` cache keys.
    #[must_use]
    pub fn canonical_fragment(&self) -> String {
        format!(
            "tdd(af={},cp={},st={})",
            self.forbid_assertion_free_tests,
            self.parity.require_companion,
            self.parity.require_stories,
        )
    }
}

impl Default for TddConfig {
    fn default() -> Self {
        Self::strict_defaults()
    }
}

/// Linter-suppression comment or unjustified attribute detected in CST (`GF070`).
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct BypassFinding {
    /// Matched directive summary (e.g. `"eslint-disable"`, `"@ts-ignore"`, or lint allow attributes).
    pub directive: String,
    /// 1-indexed source span of the comment or attribute node.
    pub span: Span,
}

/// Category of local KISS (`GF073`) or YAGNI (`GF074`) finding.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum KissYagniKind {
    /// Nested ternary/conditional expression (`GF073`).
    NestedTernary,
    /// Law of Demeter property access train (`a.b.c.d.e`, `GF073`).
    PropertyChain,
    /// Barrel wildcard re-export/import (`export *`, `from x import *`, `pub use *`, `GF074`).
    WildcardReExport,
    /// Callable with boolean parameter count (`GF074`).
    ExcessiveBoolParams,
    /// Deferred work marker (`TODO`, `FIXME`, `HACK`, `XXX`) in `Code` comment (`GF074`).
    TodoMarker,
    /// Multi-line block of commented-out dead code (`GF074`).
    CommentedOutCode,
}

/// Local KISS or YAGNI finding attached to a [`crate::UastDocument`].
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct KissYagniFinding {
    /// Category of KISS/YAGNI smell.
    pub kind: KissYagniKind,
    /// Quantitative score (nesting depth, chain segments, or `bool` parameter count).
    pub measured: u32,
    /// Human-readable description of the construct.
    pub detail: String,
    /// 1-indexed source span.
    pub span: Span,
}

/// Behavioral contract (`trait`, `interface` with methods, Python `ABC`/`Protocol`, `GF075`).
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ContractDecl {
    /// Unqualified identifier of the behavioral contract.
    pub name: String,
    /// Number of declared abstract/required methods (`>= 1`).
    pub method_count: u32,
    /// Whether the contract is publicly exported at module top level.
    pub is_public: bool,
    /// 1-indexed source span.
    pub span: Span,
}

/// Concrete implementation binding `(contract, implementor)` recorded in a file (`GF075`).
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
pub struct ContractImpl {
    /// Unqualified name of the implemented `trait`, `interface`, or `Protocol`.
    pub contract: String,
    /// Name of the implementing type or class.
    pub implementor: String,
}

/// Extracted unit/integration test case summary (`GF076`).
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct TestCaseSummary {
    /// Name or description of the test case.
    pub name: String,
    /// Number of real assertion statements/calls inside the test body.
    pub assertion_count: u32,
    /// `true` if a tautological assertion (e.g. `assert!(true)`) was found.
    pub has_tautology: bool,
    /// 1-indexed source span of the test declaration.
    pub span: Span,
}
