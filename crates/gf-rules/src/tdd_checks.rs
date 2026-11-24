//! Pure `#![no_std]` `garde.lock` integrity check (`GF070`) and TDD Fortress (`GF076`):
//! assertion-free/tautological test blocking, `1:1` companion parity, and `.stories.tsx` parity.

use alloc::collections::BTreeSet;
use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;

use gf_core::{
    AgenticSyncStatus, Config, Diagnostic, DomainKind, FeatureId, Language, LockStatus,
    ProjectContext, RepoPath, Span, UastDocument, Validation, classify_path, glob_matches,
};

use crate::checks::rule_error;
use crate::workspace_checks::is_entrypoint_file;

const ATOMIC_UI_COMPONENT_GLOBS: &[&str] = &[
    "**/components/atoms/**/*.tsx",
    "**/components/molecules/**/*.tsx",
    "**/components/organisms/**/*.tsx",
    "**/components/atoms/**/*.jsx",
    "**/components/molecules/**/*.jsx",
    "**/components/organisms/**/*.jsx",
];

/// Evaluates cryptographic `garde.lock` synchronization against `garde-fou.toml` (`GF070`).
#[must_use]
pub fn check_lock_integrity(lock_status: &LockStatus) -> Validation<(), Diagnostic> {
    let manifest = RepoPath::manifest();
    match lock_status {
        LockStatus::NotRequired | LockStatus::Verified => Validation::Valid(()),
        LockStatus::Missing => Validation::Invalid(alloc::vec![rule_error(
            "GF070",
            String::from("`garde.lock` is missing while `[integrity] require_lock = true`"),
            "Run `garde lock` and commit `garde.lock` alongside `garde-fou.toml` to seal analyzer thresholds.",
            (&manifest, Span::FIRST),
        )]),
        LockStatus::Tampered { expected, actual } => Validation::Invalid(alloc::vec![rule_error(
            "GF070",
            format!(
                "`garde-fou.toml` integrity mismatch against `garde.lock` (locked={expected}, actual={actual})"
            ),
            "Never weaken `garde-fou.toml` thresholds to silence violations; if a config change is authorized by a human, regenerate `garde.lock` via `garde lock`.",
            (&manifest, Span::FIRST),
        )]),
    }
}

/// Evaluates `AGENTS.md` managed-block and `.agents/skills/garde-fou/SKILL.md` governance (`GF070`).
#[must_use]
pub fn check_agentic_governance(status: &AgenticSyncStatus) -> Validation<(), Diagnostic> {
    let agents_path = RepoPath::agents_md();
    match status {
        AgenticSyncStatus::NotApplicable | AgenticSyncStatus::Synced => Validation::Valid(()),
        AgenticSyncStatus::MissingAgentsMd => Validation::Invalid(alloc::vec![rule_error(
            "GF070",
            String::from(
                "managed `AGENTS.md` block is missing while `[agents] sync_policy = \"required\"`"
            ),
            "Run `garde sync` to generate the contextual `AGENTS.md` managed block and `.agents/skills/`.",
            (&agents_path, Span::FIRST),
        )]),
        AgenticSyncStatus::DriftedAgentsMd => Validation::Invalid(alloc::vec![rule_error(
            "GF070",
            String::from(
                "`AGENTS.md` managed block is out of sync with workspace context, `garde-fou.toml`, or installed skills"
            ),
            "Run `garde sync` to refresh `AGENTS.md` fragments and thresholds without altering custom instructions outside managed markers.",
            (&agents_path, Span::FIRST),
        )]),
        AgenticSyncStatus::MissingOrOutdatedCoreSkill => {
            Validation::Invalid(alloc::vec![rule_error(
                "GF070",
                String::from("embedded `.agents/skills/garde-fou/SKILL.md` is missing or outdated"),
                "Run `garde sync` to install or update `.agents/skills/garde-fou/SKILL.md`.",
                (&agents_path, Span::FIRST),
            )])
        }
    }
}

/// Evaluates single-file test assertion presence and non-tautology (`GF076`).
pub fn check_local_test_assertions(
    doc: &UastDocument,
    config: &Config,
) -> Validation<(), Diagnostic> {
    if !config.tdd.forbid_assertion_free_tests {
        return Validation::Valid(());
    }
    let mut diags = Vec::new();
    for tc in &doc.test_cases {
        if tc.assertion_count == 0 {
            diags.push(rule_error(
                "GF076",
                format!(
                    "test `{}` in `{}` contains zero real assertions",
                    tc.name, doc.path
                ),
                "Assert observable outputs, invariants, or error states explicitly (`assert_eq!`, `expect(x).toEqual(y)`, `prop_assert!`).",
                (&doc.path, tc.span),
            ));
        } else if tc.has_tautology {
            diags.push(rule_error(
                "GF076",
                format!(
                    "test `{}` in `{}` contains a tautological assertion (e.g. `assert!(true)`)",
                    tc.name, doc.path
                ),
                "Replace constant tautologies (`assert!(true)`, `expect(true).toBe(true)`) with assertions over actual system behavior.",
                (&doc.path, tc.span),
            ));
        }
    }
    Validation::<(), Diagnostic>::from_errors(diags)
}

fn file_stem_and_dir(path: &str) -> (&str, &str) {
    let (dir, fname) = path.rsplit_once('/').unwrap_or(("", path));
    let stem = fname.split('.').next().unwrap_or(fname);
    (dir, stem)
}

fn has_companion_test_file(
    doc_path: &str,
    test_stems: &BTreeSet<String>,
    known: &BTreeSet<&str>,
) -> bool {
    let (dir, stem) = file_stem_and_dir(doc_path);
    for ext in ["ts", "tsx", "js", "jsx", "py", "rs"] {
        let colocated_test = if dir.is_empty() {
            format!("{stem}.test.{ext}")
        } else {
            format!("{dir}/{stem}.test.{ext}")
        };
        let colocated_spec = if dir.is_empty() {
            format!("{stem}.spec.{ext}")
        } else {
            format!("{dir}/{stem}.spec.{ext}")
        };
        if known.contains(colocated_test.as_str()) || known.contains(colocated_spec.as_str()) {
            return true;
        }
    }
    test_stems.contains(stem)
}

fn collect_test_stems(docs: &[UastDocument], config: &Config) -> BTreeSet<String> {
    let mut stems = BTreeSet::new();
    for doc in docs {
        if classify_path(&doc.path, &config.domains) != DomainKind::Tests
            && doc.test_cases.is_empty()
        {
            continue;
        }
        let (_, raw_stem) = file_stem_and_dir(doc.path.as_str());
        let clean = raw_stem
            .strip_prefix("test_")
            .or_else(|| raw_stem.strip_suffix("_test"))
            .unwrap_or(raw_stem);
        stems.insert(String::from(clean));
    }
    stems
}

fn requires_companion_test(doc: &UastDocument, ctx: &ProjectContext, config: &Config) -> bool {
    let (_, stem) = file_stem_and_dir(doc.path.as_str());
    let exempt_stem = matches!(stem, "lib" | "main" | "mod" | "index" | "__init__");
    classify_path(&doc.path, &config.domains) == DomainKind::Code
        && doc.language.is_code_analyzed()
        && doc.language != Language::Dockerfile
        && !doc.callables.is_empty()
        && !exempt_stem
        && !is_entrypoint_file(&doc.path, ctx, config)
}

fn check_companion_tests(
    docs: &[UastDocument],
    ctx: &ProjectContext,
    config: &Config,
    out: &mut Vec<Diagnostic>,
) {
    let known: BTreeSet<&str> = docs.iter().map(|d| d.path.as_str()).collect();
    let test_stems = collect_test_stems(docs, config);
    for doc in docs {
        if requires_companion_test(doc, ctx, config)
            && doc.test_cases.is_empty()
            && !has_companion_test_file(doc.path.as_str(), &test_stems, &known)
        {
            out.push(rule_error(
                "GF076",
                format!(
                    "`Code` module `{}` has executable logic without inline tests or a `1:1` companion test file",
                    doc.path
                ),
                "Add inline unit tests (`#[cfg(test)]`) or create a companion test file (`<name>.test.ts`, `test_<name>.py`, `tests/<name>.rs`).",
                (&doc.path, Span::FIRST),
            ));
        }
    }
}

fn is_story_target_component(path: &str) -> Option<(&str, &str)> {
    if path.contains(".stories.") || path.contains(".test.") || path.contains(".spec.") {
        return None;
    }
    let is_atomic = ATOMIC_UI_COMPONENT_GLOBS
        .iter()
        .any(|pat| glob_matches(pat, path));
    let (dir, stem) = file_stem_and_dir(path);
    (is_atomic && stem != "index").then_some((dir, stem))
}

fn check_component_stories(docs: &[UastDocument], out: &mut Vec<Diagnostic>) {
    let known: BTreeSet<&str> = docs.iter().map(|d| d.path.as_str()).collect();
    for doc in docs {
        let Some((dir, stem)) = is_story_target_component(doc.path.as_str()) else {
            continue;
        };
        let has_story = ["stories.tsx", "stories.ts", "stories.jsx", "stories.js"]
            .iter()
            .any(|ext| known.contains(format!("{dir}/{stem}.{ext}").as_str()));
        if !has_story {
            out.push(rule_error(
                "GF076",
                format!(
                    "Atomic Design component `{}` is missing its companion `{dir}/{stem}.stories.tsx`",
                    doc.path
                ),
                "Create a colocated `.stories.tsx` documenting the component's visual states alongside its unit tests.",
                (&doc.path, Span::FIRST),
            ));
        }
    }
}

/// Evaluates workspace-level TDD companion tests and Storybook `.stories` parity (`GF076`).
pub fn check_workspace_tdd(
    docs: &[UastDocument],
    ctx: &ProjectContext,
    config: &Config,
) -> Validation<(), Diagnostic> {
    let mut diags = Vec::new();
    if config.tdd.parity.require_companion || ctx.features.contains(&FeatureId::Tdd) {
        check_companion_tests(docs, ctx, config, &mut diags);
    }
    if config.tdd.parity.require_stories || ctx.features.contains(&FeatureId::Storybook) {
        check_component_stories(docs, &mut diags);
    }
    Validation::<(), Diagnostic>::from_errors(diags)
}
