//! Declarative architectural `[[boundaries]]` and Atomic Design hierarchy evaluator (`GF072`).

use alloc::collections::BTreeMap;
use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;

use gf_core::{
    BoundaryRule, Config, Diagnostic, FeatureId, ImportDecl, ProjectContext, UastDocument,
    Validation, default_atomic_design_boundaries, glob_matches,
};

use crate::checks::rule_error;
use crate::import_resolver::{normalize_join, resolve_import_indices};

fn dir_of(path: &str) -> &str {
    path.rsplit_once('/').map_or("", |(d, _)| d)
}

fn candidate_target_strings(
    doc: &UastDocument,
    imp: &ImportDecl,
    docs: &[UastDocument],
    path_index: &BTreeMap<&str, usize>,
) -> Vec<String> {
    let mut targets = Vec::new();
    for idx in resolve_import_indices(&doc.path, doc.language, &imp.module_source, path_index) {
        targets.push(String::from(docs[idx].path.as_str()));
    }
    let raw = imp.module_source.trim();
    if raw.starts_with('.') {
        targets.push(normalize_join(dir_of(doc.path.as_str()), raw));
    } else if let Some(rest) = raw.strip_prefix("@/").or_else(|| raw.strip_prefix("~/")) {
        targets.push(format!("src/{}", rest.trim_start_matches('/')));
    }
    let root_tok = raw
        .split("::")
        .next()
        .and_then(|s| s.split('/').next())
        .unwrap_or(raw)
        .trim();
    if !root_tok.is_empty() {
        targets.push(String::from(root_tok));
    }
    targets.push(String::from(raw));
    targets
}

fn pattern_matches_target(pattern: &str, target: &str) -> bool {
    if pattern == target {
        return true;
    }
    if let Some(after_prefix) = target.strip_prefix(pattern)
        && (after_prefix.starts_with("::") || after_prefix.starts_with('/'))
    {
        return true;
    }
    glob_matches(pattern, target)
}

fn violates_boundary(rule: &BoundaryRule, targets: &[String]) -> Option<String> {
    for deny_pat in &rule.deny {
        if targets.iter().any(|t| pattern_matches_target(deny_pat, t)) {
            return Some(format!("matches denied boundary `{deny_pat}`"));
        }
    }
    if !rule.allow.is_empty() {
        let allowed = rule
            .allow
            .iter()
            .any(|allow_pat| targets.iter().any(|t| pattern_matches_target(allow_pat, t)));
        if !allowed {
            return Some(format!("not permitted by allow-list {:?}", rule.allow));
        }
    }
    None
}

fn check_doc_boundaries(
    doc: &UastDocument,
    matching_rules: &[&BoundaryRule],
    ctx: (&[UastDocument], &BTreeMap<&str, usize>),
    diags: &mut Vec<Diagnostic>,
) {
    let (docs, path_index) = ctx;
    for imp in &doc.imports {
        let targets = candidate_target_strings(doc, imp, docs, path_index);
        for rule in matching_rules {
            if let Some(why) = violates_boundary(rule, &targets) {
                diags.push(rule_error(
                    "GF072",
                    format!(
                        "boundary violation in `{}` importing `{}` ({why}): {}",
                        doc.path, imp.module_source, rule.reason
                    ),
                    &rule.reason,
                    (&doc.path, imp.span),
                ));
            }
        }
    }
}

/// Evaluates `[[boundaries]]` and built-in `FeatureId::Ui` Atomic Design boundaries (`GF072`).
pub fn check_architecture_boundaries(
    docs: &[UastDocument],
    ctx: &ProjectContext,
    config: &Config,
) -> Validation<(), Diagnostic> {
    let mut active_rules: Vec<BoundaryRule> = config.boundaries.clone();
    if ctx.features.contains(&FeatureId::Ui) {
        active_rules.extend(default_atomic_design_boundaries());
    }
    if active_rules.is_empty() {
        return Validation::Valid(());
    }

    let path_index: BTreeMap<&str, usize> = docs
        .iter()
        .enumerate()
        .map(|(idx, d)| (d.path.as_str(), idx))
        .collect();

    let mut diags = Vec::new();
    for doc in docs {
        let matching_rules: Vec<&BoundaryRule> = active_rules
            .iter()
            .filter(|r| glob_matches(&r.from, doc.path.as_str()))
            .collect();
        if !matching_rules.is_empty() {
            check_doc_boundaries(doc, &matching_rules, (docs, &path_index), &mut diags);
        }
    }
    Validation::<(), Diagnostic>::from_errors(diags)
}
