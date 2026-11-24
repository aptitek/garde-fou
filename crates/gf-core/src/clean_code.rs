//! KISS, YAGNI, Anti-Bypass (`GF070`, `GF073`–`GF075`), and TDD Fortress (`GF076`)
//! algebraic configuration and `UAST` projection types.

use alloc::collections::BTreeSet;
use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;
use serde::{Deserialize, Serialize};

use crate::diagnostic::Span;

/// Individually configurable structural YAGNI rule (`GF074` & `GF075`).
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
pub enum YagniRule {
    /// Forbid barrel wildcard re-exports (`export *`, `from x import *`, `pub use *`, `GF074`).
    WildcardExports,
    /// Forbid behavioral `trait`/`interface`/`Protocol` contracts with `<= 1` concrete impl (`GF075`).
    SingleImplTraits,
}

impl YagniRule {
    /// Canonical compact token for deterministic cache hashing.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::WildcardExports => "wildcard_exports",
            Self::SingleImplTraits => "single_impl_traits",
        }
    }
}

/// `[clean_code]` configuration governing KISS (`GF073`) and YAGNI (`GF074`–`GF075`).
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
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
    /// Active structural YAGNI rules (`WildcardExports`, `SingleImplTraits`).
    pub yagni_rules: BTreeSet<YagniRule>,
}

impl CleanCodeConfig {
    /// Strict zero-compromise KISS & YAGNI defaults.
    #[must_use]
    pub fn strict_defaults() -> Self {
        Self {
            max_nested_ternaries: 0,
            max_property_chain: 4,
            max_bool_parameters: 1,
            forbid_todos: true,
            yagni_rules: BTreeSet::from([YagniRule::WildcardExports, YagniRule::SingleImplTraits]),
        }
    }

    /// Returns `true` when barrel wildcard re-exports are forbidden (`GF074`).
    #[must_use]
    pub fn has_wildcard_exports(&self) -> bool {
        self.yagni_rules.contains(&YagniRule::WildcardExports)
    }

    /// Returns `true` when single-implementation behavioral contracts are forbidden (`GF075`).
    #[must_use]
    pub fn has_single_impl_traits(&self) -> bool {
        self.yagni_rules.contains(&YagniRule::SingleImplTraits)
    }

    /// Deterministic fragment for `BLAKE3` cache keys.
    #[must_use]
    pub fn canonical_fragment(&self) -> String {
        let rules: Vec<&str> = self.yagni_rules.iter().map(|r| r.as_str()).collect();
        format!(
            "cc(nt={},pc={},bp={},td={},yg={})",
            self.max_nested_ternaries,
            self.max_property_chain,
            self.max_bool_parameters,
            self.forbid_todos,
            rules.join(","),
        )
    }
}

impl Default for CleanCodeConfig {
    fn default() -> Self {
        Self::strict_defaults()
    }
}

/// Optional 1:1 companion file parity gate under `[tdd]` (`GF076`).
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
pub enum TddParityGate {
    /// Require 1:1 unit test coverage (inline tests or companion test file) per `Code` module.
    CompanionTest,
    /// Require `.stories.tsx` companion per Atomic Design UI component.
    ComponentStories,
}

impl TddParityGate {
    /// Canonical compact token for deterministic cache hashing.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::CompanionTest => "companion",
            Self::ComponentStories => "stories",
        }
    }
}

/// `[tdd]` configuration governing assertion integrity and companion parity (`GF076`).
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TddConfig {
    /// Block tests with zero assertions, tautologies, or weak-only existence checks.
    pub forbid_assertion_free_tests: bool,
    /// Active 1:1 companion test and `.stories` gates.
    pub parity_gates: BTreeSet<TddParityGate>,
}

impl TddConfig {
    /// Strict default TDD configuration (`forbid_assertion_free_tests = true`).
    #[must_use]
    pub const fn strict_defaults() -> Self {
        Self {
            forbid_assertion_free_tests: true,
            parity_gates: BTreeSet::new(),
        }
    }

    /// Returns `true` when 1:1 companion test files are required (`GF076`).
    #[must_use]
    pub fn require_companion(&self) -> bool {
        self.parity_gates.contains(&TddParityGate::CompanionTest)
    }

    /// Returns `true` when 1:1 `.stories` files are required for UI components (`GF076`).
    #[must_use]
    pub fn require_stories(&self) -> bool {
        self.parity_gates.contains(&TddParityGate::ComponentStories)
    }

    /// Deterministic fragment for `BLAKE3` cache keys.
    #[must_use]
    pub fn canonical_fragment(&self) -> String {
        let gates: Vec<&str> = self.parity_gates.iter().map(|g| g.as_str()).collect();
        format!(
            "tdd(af={},pg={})",
            self.forbid_assertion_free_tests,
            gates.join(","),
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
    /// `true` if the test only performs weak existence checks (`is_ok()`, `toBeDefined()`) without verifying values.
    pub has_weak_only_assertion: bool,
    /// 1-indexed source span of the test declaration.
    pub span: Span,
}
