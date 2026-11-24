//! Pure `#![no_std]` workspace `1:1` companion test and Storybook `.stories` parity (`GF076`).

use alloc::collections::BTreeSet;
use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;

use gf_core::{
    Config, Diagnostic, DomainKind, FeatureId, Language, ProjectContext, Span, UastDocument,
    Validation, classify_path, glob_matches, is_ts_declaration_path,
};

use crate::checks::{SourceLoc, rule_error};
use crate::workspace_checks::is_entrypoint_file;

const ATOMIC_UI_COMPONENT_GLOBS: &[&str] = &[
    "**/components/atoms/**/*.tsx",
    "**/components/molecules/**/*.tsx",
    "**/components/organisms/**/*.tsx",
    "**/components/atoms/**/*.jsx",
    "**/components/molecules/**/*.jsx",
    "**/components/organisms/**/*.jsx",
];

#[derive(Clone, Copy)]
struct StemLocation<'a> {
    dir: &'a str,
    stem: &'a str,
}

fn split_stem_location(path: &str) -> StemLocation<'_> {
    let (dir, fname) = path.rsplit_once('/').unwrap_or(("", path));
    let stem = fname.split('.').next().unwrap_or(fname);
    StemLocation { dir, stem }
}

fn has_companion_test_file(
    doc_path: &str,
    test_stems: &BTreeSet<String>,
    non_empty_test_paths: &BTreeSet<&str>,
) -> bool {
    let loc = split_stem_location(doc_path);
    for ext in ["ts", "tsx", "js", "jsx", "py", "rs"] {
        let colocated_test = if loc.dir.is_empty() {
            format!("{}.test.{ext}", loc.stem)
        } else {
            format!("{}/{}.test.{ext}", loc.dir, loc.stem)
        };
        let colocated_spec = if loc.dir.is_empty() {
            format!("{}.spec.{ext}", loc.stem)
        } else {
            format!("{}/{}.spec.{ext}", loc.dir, loc.stem)
        };
        if non_empty_test_paths.contains(colocated_test.as_str())
            || non_empty_test_paths.contains(colocated_spec.as_str())
        {
            return true;
        }
    }
    test_stems.contains(loc.stem)
}

fn collect_test_stems(docs: &[UastDocument]) -> BTreeSet<String> {
    let mut stems = BTreeSet::new();
    for doc in docs {
        if doc.test_cases.is_empty() {
            continue;
        }
        let raw_stem = split_stem_location(doc.path.as_str()).stem;
        let clean = raw_stem
            .strip_prefix("test_")
            .or_else(|| raw_stem.strip_suffix("_test"))
            .unwrap_or(raw_stem);
        stems.insert(String::from(clean));
    }
    stems
}

fn requires_companion_test(doc: &UastDocument, ctx: &ProjectContext, config: &Config) -> bool {
    let raw_path = doc.path.as_str();
    let stem = split_stem_location(raw_path).stem;
    let exempt_stem = matches!(stem, "lib" | "main" | "mod" | "index" | "__init__");
    classify_path(&doc.path, &config.domains) == DomainKind::Code
        && doc.language.is_code_analyzed()
        && doc.language != Language::Dockerfile
        && !is_ts_declaration_path(raw_path)
        && !raw_path.contains(".stories.")
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
    let non_empty_test_paths: BTreeSet<&str> = docs
        .iter()
        .filter(|d| !d.test_cases.is_empty())
        .map(|d| d.path.as_str())
        .collect();
    let test_stems = collect_test_stems(docs);
    for doc in docs {
        if requires_companion_test(doc, ctx, config)
            && doc.test_cases.is_empty()
            && !has_companion_test_file(doc.path.as_str(), &test_stems, &non_empty_test_paths)
        {
            out.push(rule_error(
                "GF076",
                format!(
                    "`Code` module `{}` has executable logic without inline tests or a `1:1` companion test file",
                    doc.path
                ),
                "Add inline unit tests (`#[cfg(test)]`) or create a companion test file (`<name>.test.ts`, `test_<name>.py`, `tests/<name>.rs`).",
                SourceLoc::new(&doc.path, Span::FIRST),
            ));
        }
    }
}

fn story_target_component(path: &str) -> Option<StemLocation<'_>> {
    if path.contains(".stories.") || path.contains(".test.") || path.contains(".spec.") {
        return None;
    }
    let is_atomic = ATOMIC_UI_COMPONENT_GLOBS
        .iter()
        .any(|pat| glob_matches(pat, path));
    let loc = split_stem_location(path);
    (is_atomic && loc.stem != "index").then_some(loc)
}

fn check_component_stories(docs: &[UastDocument], out: &mut Vec<Diagnostic>) {
    let known: BTreeSet<&str> = docs.iter().map(|d| d.path.as_str()).collect();
    for doc in docs {
        let Some(loc) = story_target_component(doc.path.as_str()) else {
            continue;
        };
        let has_story = ["stories.tsx", "stories.ts", "stories.jsx", "stories.js"]
            .iter()
            .any(|ext| known.contains(format!("{}/{}.{ext}", loc.dir, loc.stem).as_str()));
        if !has_story {
            out.push(rule_error(
                "GF076",
                format!(
                    "Atomic Design component `{}` is missing its companion `{}/{}.stories.tsx`",
                    doc.path, loc.dir, loc.stem
                ),
                "Create a colocated `.stories.tsx` documenting the component's visual states alongside its unit tests.",
                SourceLoc::new(&doc.path, Span::FIRST),
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
    if config.tdd.require_companion() || ctx.features.contains(&FeatureId::Tdd) {
        check_companion_tests(docs, ctx, config, &mut diags);
    }
    if config.tdd.require_stories() || ctx.features.contains(&FeatureId::Storybook) {
        check_component_stories(docs, &mut diags);
    }
    Validation::<(), Diagnostic>::from_errors(diags)
}
