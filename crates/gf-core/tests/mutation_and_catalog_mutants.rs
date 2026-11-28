//! Unit tests killing 100% of `cargo-mutants` in `gf-core` (`mutation_config_issues`,
//! `skills_catalog::is_catalog_skill_id`, and `ToolConfigViolation::domain_descriptor`).

use std::collections::BTreeSet;

use gf_core::{
    CompiledConfigIssue, LanguageId, ProjectContext, ToolConfigViolation, ToolchainViolation,
    WorkspaceArchetype, is_catalog_skill_id, mutation_stack_for, render_mutation_stack_markdown,
    supports_mutation_testing,
};

#[test]
fn mutation_stack_and_markdown_kill_all_core_mutants() {
    let expected = [
        (LanguageId::Rust, "cargo-mutants"),
        (LanguageId::TypeScript, "Stryker"),
        (LanguageId::Python, "mutmut"),
        (LanguageId::Go, "gremlins"),
        (LanguageId::Kotlin, "PITest"),
        (LanguageId::CSharp, "Stryker.NET"),
        (LanguageId::Php, "Infection"),
        (LanguageId::Dart, "mutation_test"),
    ];
    for (lang, tool) in expected {
        assert!(supports_mutation_testing(lang));
        let spec = mutation_stack_for(lang);
        assert_eq!(spec.map(|s| s.tool), Some(tool));
    }
    assert!(!supports_mutation_testing(LanguageId::Markdown));
    assert_eq!(mutation_stack_for(LanguageId::Markdown), None);

    let sw_ctx = ProjectContext {
        archetype: WorkspaceArchetype::Software,
        languages: BTreeSet::from([LanguageId::Rust, LanguageId::Python]),
        features: BTreeSet::new(),
        frameworks: BTreeSet::new(),
        dockerignore_entries: None,
    };
    let md = render_mutation_stack_markdown(&sw_ctx);
    assert!(md.contains("## Mutation Testing Stack (`GF052`)"));
    assert!(md.contains("cargo-mutants"));
    assert!(md.contains("mutmut"));

    let non_sw = ProjectContext {
        archetype: WorkspaceArchetype::Minimal,
        ..sw_ctx
    };
    assert_eq!(render_mutation_stack_markdown(&non_sw), "");

    let no_mut_langs = ProjectContext {
        archetype: WorkspaceArchetype::Software,
        languages: BTreeSet::from([LanguageId::Markdown]),
        features: BTreeSet::new(),
        frameworks: BTreeSet::new(),
        dockerignore_entries: None,
    };
    assert_eq!(render_mutation_stack_markdown(&no_mut_langs), "");
}

#[test]
fn catalog_skill_id_and_domain_descriptors_kill_all_mutants() {
    assert!(is_catalog_skill_id("garde-fou"));
    assert!(is_catalog_skill_id("ast-grep"));
    assert!(!is_catalog_skill_id("nonexistent-skill-xyz"));

    let compiled = ToolConfigViolation::Compiled(CompiledConfigIssue::GoModLocalReplace);
    assert_eq!(compiled.key(), CompiledConfigIssue::GoModLocalReplace.key());
    assert_eq!(
        compiled.tool(),
        CompiledConfigIssue::GoModLocalReplace.tool()
    );
    assert_ne!(compiled.remediation(), "");

    let toolchain = ToolConfigViolation::Toolchain(ToolchainViolation::PnpmOnly);
    assert_eq!(toolchain.key(), ToolchainViolation::PnpmOnly.code());
    assert_eq!(toolchain.tool(), ToolchainViolation::PnpmOnly.tool());
    assert_ne!(toolchain.remediation(), "");
}
