//! Pure `#![no_std]` workspace `Asset` reference/orphan checks (`GF064`) and
//! `Scripts` boundary + test-coverage checks (`GF065`).

use alloc::collections::BTreeSet;
use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;

use gf_core::{
    Config, Diagnostic, DomainKind, RepoPath, Span, UastDocument, Validation, classify_path,
    glob_matches,
};

use crate::checks::rule_error as domain_diag;

const IMPLICIT_PUBLIC_ASSETS: &[&str] = &[
    "public/favicon.*",
    "public/robots.txt",
    "static/favicon.*",
    "assets/favicon.*",
];

fn normalize_relative_join(from_file: &str, raw_ref: &str) -> String {
    let base_dir = from_file.rsplit_once('/').map_or("", |(dir, _)| dir);
    let combined = if raw_ref.starts_with('/') {
        raw_ref.trim_start_matches('/')
    } else if base_dir.is_empty() {
        raw_ref
    } else {
        return collapse_segments(&format!("{base_dir}/{raw_ref}"));
    };
    collapse_segments(combined)
}

fn collapse_segments(path: &str) -> String {
    let mut stack: Vec<&str> = Vec::new();
    for seg in path.split('/') {
        match seg {
            "" | "." => {}
            ".." => {
                let _ = stack.pop();
            }
            other => stack.push(other),
        }
    }
    stack.join("/")
}

fn resolve_asset_target<'a>(
    from_path: &RepoPath,
    raw_ref: &str,
    known_paths: &'a BTreeSet<&'a str>,
) -> Option<&'a str> {
    let candidates = if raw_ref.starts_with('.') {
        alloc::vec![normalize_relative_join(from_path.as_str(), raw_ref)]
    } else {
        let trimmed = raw_ref.trim_start_matches('/');
        alloc::vec![
            String::from(trimmed),
            normalize_relative_join(from_path.as_str(), raw_ref),
            format!("public/{trimmed}"),
            format!("assets/{trimmed}"),
            format!("static/{trimmed}"),
        ]
    };
    for cand in candidates {
        if let Some(&found) = known_paths.get(cand.as_str()) {
            return Some(found);
        }
    }
    None
}

fn is_exempt_public_asset(path: &str, config: &Config) -> bool {
    IMPLICIT_PUBLIC_ASSETS
        .iter()
        .any(|pat| glob_matches(pat, path))
        || config.entrypoints.iter().any(|pat| glob_matches(pat, path))
}

fn collect_referenced_assets<'a>(
    docs: &'a [UastDocument],
    known_paths: &'a BTreeSet<&'a str>,
    out: &mut Vec<Diagnostic>,
) -> BTreeSet<&'a str> {
    let mut referenced_assets = BTreeSet::new();
    for doc in docs {
        for raw_ref in &doc.asset_refs {
            if let Some(resolved) = resolve_asset_target(&doc.path, raw_ref, known_paths) {
                referenced_assets.insert(resolved);
            } else {
                out.push(domain_diag(
                    "GF064",
                    format!("broken asset reference `{raw_ref}` in `{}`", doc.path),
                    "Ensure every referenced image, icon, or font exists inside `assets/` (`asset_globs`).",
                    (&doc.path, Span::FIRST),
                ));
            }
        }
    }
    referenced_assets
}

fn check_asset_links_and_orphans(
    docs: &[UastDocument],
    config: &Config,
    out: &mut Vec<Diagnostic>,
) {
    let known_paths: BTreeSet<&str> = docs.iter().map(|d| d.path.as_str()).collect();
    let referenced_assets = collect_referenced_assets(docs, &known_paths, out);

    for doc in docs {
        let is_orphan = classify_path(&doc.path, &config.domains) == DomainKind::Asset
            && doc.asset.is_some()
            && !is_exempt_public_asset(doc.path.as_str(), config)
            && !referenced_assets.contains(doc.path.as_str());
        if is_orphan {
            out.push(domain_diag(
                "GF064",
                format!("orphan asset `{}` is never referenced across the workspace", doc.path),
                "Delete unused static assets or reference them explicitly from `Code`, `Forme`, or `Contenus`.",
                (&doc.path, Span::FIRST),
            ));
        }
    }
}

fn import_targets_scripts(from_path: &RepoPath, spec: &str, config: &Config) -> bool {
    let candidates = if spec.starts_with('.') {
        alloc::vec![normalize_relative_join(from_path.as_str(), spec)]
    } else {
        alloc::vec![String::from(spec.trim_start_matches('/'))]
    };
    for base in candidates {
        for probe in [
            base.clone(),
            format!("{base}.ts"),
            format!("{base}.py"),
            format!("{base}.sh"),
        ] {
            if let Ok(rp) = RepoPath::new(&probe)
                && classify_path(&rp, &config.domains) == DomainKind::Scripts
            {
                return true;
            }
        }
    }
    false
}

fn collect_test_script_mentions(docs: &[UastDocument], config: &Config) -> BTreeSet<String> {
    let mut covered = BTreeSet::new();
    for doc in docs {
        if classify_path(&doc.path, &config.domains) != DomainKind::Tests {
            continue;
        }
        for r in &doc.script_refs {
            covered.insert(r.clone());
        }
        for imp in &doc.imports {
            let norm = normalize_relative_join(doc.path.as_str(), &imp.module_source);
            let fname = norm.rsplit('/').next().unwrap_or(&norm);
            covered.insert(String::from(fname));
            covered.insert(norm);
        }
    }
    covered
}

fn check_code_script_imports(doc: &UastDocument, config: &Config, out: &mut Vec<Diagnostic>) {
    for imp in &doc.imports {
        if import_targets_scripts(&doc.path, &imp.module_source, config) {
            out.push(domain_diag(
                "GF065",
                format!(
                    "`Code` file `{}` must not import from `Scripts` (`{}`)",
                    doc.path, imp.module_source
                ),
                "Extract shared domain logic into `src/` (`Code` domain); keep `scripts/` strictly standalone.",
                (&doc.path, imp.span),
            ));
        }
    }
}

fn is_script_covered(path: &RepoPath, covered: &BTreeSet<String>) -> bool {
    let fname = path.file_name();
    let stem = fname.rsplit_once('.').map_or(fname, |(s, _)| s);
    covered.contains(path.as_str())
        || covered.contains(fname)
        || covered.contains(&format!("scripts/{stem}"))
        || covered.contains(&format!("tools/{stem}"))
}

fn check_scripts_boundary_and_tests(
    docs: &[UastDocument],
    config: &Config,
    out: &mut Vec<Diagnostic>,
) {
    let covered = collect_test_script_mentions(docs, config);

    for doc in docs {
        match classify_path(&doc.path, &config.domains) {
            DomainKind::Code => check_code_script_imports(doc, config, out),
            DomainKind::Scripts if !is_script_covered(&doc.path, &covered) => {
                out.push(domain_diag(
                    "GF065",
                    format!(
                        "script `{}` in `Scripts` domain has no automated test coverage",
                        doc.path
                    ),
                    "Add an automated unit/smoke test under `tests/` exercising this script and referencing its path.",
                    (&doc.path, Span::FIRST),
                ));
            }
            _ => {}
        }
    }
}

/// Evaluates workspace-wide `Asset` (`GF064`) and `Scripts` (`GF065`) segregation rules.
pub fn check_workspace_assets_and_scripts(
    docs: &[UastDocument],
    config: &Config,
) -> Validation<(), Diagnostic> {
    let mut diags = Vec::new();
    check_asset_links_and_orphans(docs, config, &mut diags);
    check_scripts_boundary_and_tests(docs, config, &mut diags);
    Validation::<(), Diagnostic>::from_errors(diags)
}
