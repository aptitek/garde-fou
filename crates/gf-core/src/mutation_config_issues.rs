//! Mutation-testing configuration issue enums (`GF081`) and polyglot stack specs (`GF052`).

use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;
use serde::{Deserialize, Serialize};

use crate::context::{LanguageId, ProjectContext, WorkspaceArchetype};

/// Default minimum mutation score enforced in generated `garde-fou.toml` (`GF052`).
pub const DEFAULT_MIN_MUTATION_SCORE: u32 = 100;

/// Canonical mutation-testing toolchain descriptor for a supported programming language (`GF052`).
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MutationStackSpec {
    /// Human-readable mutation runner name (e.g. `"cargo-mutants"`, `"Stryker"`, `"mutmut"`).
    pub tool: &'static str,
    /// Canonical command to execute mutation testing and produce a machine-readable report.
    pub command: &'static str,
    /// Default repository-relative report path ingested by `garde check --mutants`.
    pub report_path: &'static str,
    /// Configuration file hardened by `GF081`.
    pub config_file: &'static str,
}

const fn mut_spec(
    tool: &'static str,
    command: &'static str,
    report_path: &'static str,
    config_file: &'static str,
) -> MutationStackSpec {
    MutationStackSpec {
        tool,
        command,
        report_path,
        config_file,
    }
}

const MUTATION_STACKS: &[(LanguageId, MutationStackSpec)] = &[
    (
        LanguageId::Rust,
        mut_spec(
            "cargo-mutants",
            "cargo mutants --output mutants.out",
            "mutants.out/outcomes.json",
            "mutants.toml",
        ),
    ),
    (
        LanguageId::TypeScript,
        mut_spec(
            "Stryker",
            "pnpm exec stryker run",
            "reports/mutation/mutation.json",
            "stryker.config.json",
        ),
    ),
    (
        LanguageId::Python,
        mut_spec(
            "mutmut",
            "uv run mutmut run && uv run mutmut results --json > reports/mutation/mutmut.json",
            "reports/mutation/mutmut.json",
            "pyproject.toml",
        ),
    ),
    (
        LanguageId::Go,
        mut_spec(
            "gremlins",
            "gremlins unleash --output reports/mutation/gremlins.json",
            "reports/mutation/gremlins.json",
            ".gremlins.yml",
        ),
    ),
    (
        LanguageId::Kotlin,
        mut_spec(
            "PITest",
            "./gradlew pitest",
            "build/reports/pitest/mutations.xml",
            "build.gradle.kts",
        ),
    ),
    (
        LanguageId::CSharp,
        mut_spec(
            "Stryker.NET",
            "dotnet stryker --reporter json",
            "StrykerOutput/reports/mutation-report.json",
            "stryker-config.json",
        ),
    ),
    (
        LanguageId::Php,
        mut_spec(
            "Infection",
            "vendor/bin/infection --logger-stryker=report:reports/mutation/mutation.json",
            "reports/mutation/mutation.json",
            "infection.json5",
        ),
    ),
    (
        LanguageId::Dart,
        mut_spec(
            "mutation_test",
            "dart run mutation_test --output reports/mutation",
            "reports/mutation/mutation.json",
            "pubspec.yaml",
        ),
    ),
];

/// Returns `true` when `lang` has an enforced mutation-testing runner in the stack (`GF052`).
#[must_use]
pub const fn supports_mutation_testing(lang: LanguageId) -> bool {
    mutation_stack_for(lang).is_some()
}

/// Returns the canonical mutation-testing runner specification for `lang`, if applicable.
#[must_use]
pub const fn mutation_stack_for(lang: LanguageId) -> Option<MutationStackSpec> {
    let mut i = 0;
    while i < MUTATION_STACKS.len() {
        if MUTATION_STACKS[i].0 as u8 == lang as u8 {
            return Some(MUTATION_STACKS[i].1);
        }
        i += 1;
    }
    None
}

/// Renders the `AGENTS.md` mutation-testing enforcement table for all relevant languages in `ctx`.
#[must_use]
pub fn render_mutation_stack_markdown(ctx: &ProjectContext) -> String {
    if ctx.archetype != WorkspaceArchetype::Software {
        return String::new();
    }
    let rows: Vec<String> = ctx
        .languages
        .iter()
        .filter_map(|&lang| {
            let spec = mutation_stack_for(lang)?;
            Some(format!(
                "| `{}` | `{}` | `{}` | `{}` |",
                lang.as_str(),
                spec.tool,
                spec.command,
                spec.report_path
            ))
        })
        .collect();
    if rows.is_empty() {
        return String::new();
    }
    format!(
        "\n\n## Mutation Testing Stack (`GF052`)\n\n\
         | Language | Runner | Command | Report Path |\n\
         | :--- | :--- | :--- | :--- |\n\
         {}",
        rows.join("\n")
    )
}

/// Specific `mutants.toml` scope sabotage (`GF081`).
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
pub enum MutantsConfigIssue {
    /// `examine_globs = []`.
    EmptyExamineGlobs,
    /// `examine_re = []`.
    EmptyExamineRe,
    /// Broad wildcard `exclude_globs` entry (`*`, `**`, `src/**`, `crates/**`).
    BroadExcludeGlobs,
}

impl MutantsConfigIssue {
    /// Canonical key inside `mutants.toml`.
    #[must_use]
    pub const fn key(self) -> &'static str {
        match self {
            Self::EmptyExamineGlobs => "examine_globs",
            Self::EmptyExamineRe => "examine_re",
            Self::BroadExcludeGlobs => "exclude_globs",
        }
    }
}

/// Specific `stryker.config.json` mutation gate weakening (`GF081`).
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
pub enum StrykerConfigIssue {
    /// `"mutate": []`.
    EmptyMutate,
    /// `"thresholds.break": 0` or `null`.
    DisabledBreakThreshold,
}

impl StrykerConfigIssue {
    /// Canonical key inside `stryker.config.json`.
    #[must_use]
    pub const fn key(self) -> &'static str {
        match self {
            Self::EmptyMutate => "mutate",
            Self::DisabledBreakThreshold => "thresholds.break",
        }
    }
}
