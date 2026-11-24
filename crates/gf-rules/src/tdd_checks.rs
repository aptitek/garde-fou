//! Pure `#![no_std]` single-file TDD Fortress (`GF076`): vacuous test-file blocking,
//! assertion-free test blocking, tautology detection, and weak-only assertion detection.

use alloc::format;

use gf_core::{
    Config, Diagnostic, DomainKind, Language, RepoPath, Span, TestCaseSummary, UastDocument,
    Validation, classify_path, glob_matches,
};

use crate::checks::{SourceLoc, rule_error};

const SHARED_TEST_HELPER_GLOBS: &[&str] = &[
    "**/common/**",
    "common/**",
    "**/helpers/**",
    "helpers/**",
    "**/fixtures/**",
    "fixtures/**",
];

const fn is_executable_test_language(lang: Language) -> bool {
    matches!(
        lang,
        Language::Rust
            | Language::Python
            | Language::TypeScript
            | Language::Tsx
            | Language::JavaScript
            | Language::Jsx
    )
}

fn is_shared_test_helper(path: &RepoPath) -> bool {
    let raw = path.as_str();
    matches!(path.file_name(), "__init__.py" | "conftest.py")
        || raw.ends_with(".d.ts")
        || SHARED_TEST_HELPER_GLOBS
            .iter()
            .any(|pat| glob_matches(pat, raw))
}

fn evaluate_test_case(tc: &TestCaseSummary, path: &RepoPath) -> Option<Diagnostic> {
    let loc = SourceLoc::new(path, tc.span);
    if tc.assertion_count == 0 {
        Some(rule_error(
            "GF076",
            format!(
                "test `{}` in `{path}` contains zero real assertions",
                tc.name
            ),
            "Assert observable outputs, invariants, or error states explicitly (`assert_eq!`, `expect(x).toEqual(y)`, `prop_assert!`).",
            loc,
        ))
    } else if tc.has_tautology {
        Some(rule_error(
            "GF076",
            format!(
                "test `{}` in `{path}` contains a tautological assertion (e.g. `assert!(true)`)",
                tc.name
            ),
            "Replace constant or reflexive tautologies (`assert!(true)`, `assert_eq!(x, x)`, `expect(x).toBe(x)`) with assertions over actual system behavior.",
            loc,
        ))
    } else if tc.has_weak_only_assertion {
        Some(rule_error(
            "GF076",
            format!(
                "test `{}` in `{path}` relies exclusively on weak existence assertions (e.g. `.is_ok()` / `.toBeDefined()` / `is not None`)",
                tc.name
            ),
            "Assert the concrete value or domain invariant produced instead of only checking `.is_ok()`, `.is_some()`, `.toBeDefined()`, or `is not None`.",
            loc,
        ))
    } else {
        None
    }
}

/// Evaluates single-file test vacuity, assertion presence, tautologies, and weak-only checks (`GF076`).
pub fn check_local_test_assertions(
    doc: &UastDocument,
    config: &Config,
) -> Validation<(), Diagnostic> {
    if !config.tdd.forbid_assertion_free_tests {
        return Validation::Valid(());
    }
    if doc.test_cases.is_empty()
        && classify_path(&doc.path, &config.domains) == DomainKind::Tests
        && is_executable_test_language(doc.language)
        && !is_shared_test_helper(&doc.path)
    {
        return Validation::invalid_one(rule_error(
            "GF076",
            format!(
                "test file `{}` in `Tests` domain defines zero executable test cases",
                doc.path
            ),
            "Define real test functions (`#[test]`, `test(...)`, `def test_*`) with explicit value assertions instead of leaving empty or helper-only `.test` files.",
            SourceLoc::new(&doc.path, Span::FIRST),
        ));
    }
    let diags = doc
        .test_cases
        .iter()
        .filter_map(|tc| evaluate_test_case(tc, &doc.path))
        .collect();
    Validation::<(), Diagnostic>::from_errors(diags)
}
