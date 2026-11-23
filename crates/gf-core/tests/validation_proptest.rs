//! Property-based algebraic tests for `Validation` and `RepoPath` invariants.

use gf_core::{RepoPath, RepoPathError, Validation};
use proptest::prelude::*;
use proptest::test_runner::{Config as ProptestConfig, TestRunner};

fn arb_unit_validation() -> impl Strategy<Value = Validation<(), u16>> {
    prop_oneof![
        Just(Validation::Valid(())),
        prop::collection::vec(any::<u16>(), 1..6).prop_map(Validation::Invalid),
    ]
}

fn runner() -> TestRunner {
    TestRunner::new(ProptestConfig {
        cases: 128,
        failure_persistence: None,
        ..ProptestConfig::default()
    })
}

#[test]
fn validation_collect_all_is_associative_and_identity_preserving() -> Result<(), String> {
    let mut tr = runner();
    let triple = (
        arb_unit_validation(),
        arb_unit_validation(),
        arb_unit_validation(),
    );
    tr.run(&triple, |(a, b, c)| {
        let left_assoc = a
            .clone()
            .zip_with(b.clone(), |(), ()| ())
            .zip_with(c.clone(), |(), ()| ());
        let right_assoc = a
            .clone()
            .zip_with(b.clone().zip_with(c.clone(), |(), ()| ()), |(), ()| ());
        let collected = Validation::<(), u16>::collect_all([a.clone(), b, c]);

        prop_assert_eq!(&left_assoc, &right_assoc);
        prop_assert_eq!(left_assoc, collected);
        prop_assert_eq!(&Validation::valid(()).zip_with(a.clone(), |(), ()| ()), &a);
        prop_assert_eq!(a.clone().zip_with(Validation::valid(()), |(), ()| ()), a);
        Ok(())
    })
    .map_err(|err| err.to_string())
}

#[test]
fn validation_functor_and_into_result_preserve_errors() -> Result<(), String> {
    let mut tr = runner();
    let strat = (-1000i32..1000i32, prop::collection::vec(0u8..100, 0..5));
    tr.run(&strat, |(x, errs)| {
        let val: Validation<i32, u8> = if errs.is_empty() {
            Validation::valid(x)
        } else {
            Validation::Invalid(errs.clone())
        };
        let f = |n: i32| n.saturating_add(7);
        let g = |n: i32| n.saturating_mul(3);

        prop_assert_eq!(&val.clone().map(|n| n), &val);
        prop_assert_eq!(val.clone().map(f).map(g), val.clone().map(|n| g(f(n))));
        prop_assert_eq!(val.is_valid(), errs.is_empty());
        prop_assert_eq!(
            Validation::<(), u8>::from_errors(errs.clone()).into_result(),
            if errs.is_empty() { Ok(()) } else { Err(errs) }
        );
        Ok(())
    })
    .map_err(|err| err.to_string())
}

#[test]
fn repo_path_normalizes_separators_and_rejects_traversal() -> Result<(), String> {
    let mut tr = runner();
    let strat = prop::collection::vec("[a-z0-9_]{1,8}", 1..5);
    tr.run(&strat, |segments| {
        let backslash_joined = segments.join("\\");
        let slash_joined = segments.join("/");
        let parsed = RepoPath::new(&backslash_joined);
        prop_assert_eq!(
            parsed.as_ref().map(RepoPath::as_str),
            Ok(slash_joined.as_str())
        );

        let traversal = format!("{slash_joined}/../leak");
        prop_assert_eq!(
            RepoPath::new(&traversal),
            Err(RepoPathError::ParentTraversal)
        );
        Ok(())
    })
    .map_err(|err| err.to_string())
}

#[test]
fn parses_and_formats_all_context_identifiers() {
    use gf_core::{FeatureId, FrameworkId, LanguageId, ProjectContext};

    for (raw, expected) in [
        ("rust", LanguageId::Rust),
        ("ts", LanguageId::TypeScript),
        ("py", LanguageId::Python),
    ] {
        assert_eq!(LanguageId::from_config_str(raw), Some(expected));
        assert_ne!(expected.to_string(), "");
    }
    assert_eq!(LanguageId::from_config_str("cobol"), None);
    for (raw, expected) in [
        ("docker", FeatureId::Docker),
        ("git-hooks", FeatureId::GitHooks),
        ("ci-github", FeatureId::CiGithub),
        ("task-runner", FeatureId::TaskRunner),
    ] {
        assert_eq!(FeatureId::from_config_str(raw), Some(expected));
        assert_ne!(expected.to_string(), "");
    }
    assert_eq!(FeatureId::from_config_str("unknown"), None);
    for (raw, fw) in [
        ("cargo", FrameworkId::CargoWorkspace),
        ("react", FrameworkId::React),
        ("astro", FrameworkId::Astro),
        ("nextjs", FrameworkId::NextJs),
        ("fastapi", FrameworkId::FastApi),
        ("django", FrameworkId::Django),
    ] {
        assert_eq!(FrameworkId::from_config_str(raw), Some(fw));
        assert_ne!(fw.implicit_entrypoints().len(), 0);
        assert_ne!(fw.to_string(), "");
    }
    assert_eq!(FrameworkId::from_config_str("rails"), None);
    assert!(
        ProjectContext::default()
            .summary_inline()
            .contains("langs=[]")
    );
}
