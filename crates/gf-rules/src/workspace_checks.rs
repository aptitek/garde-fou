//! Workspace-wide pure rules: Type-2 AST structural clones (`GF020`) & dead exports (`GF021`).

use alloc::collections::{BTreeMap, BTreeSet};
use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;

use gf_core::{
    Config, Diagnostic, DomainKind, Language, ProjectContext, RepoPath, Severity, Span,
    UastDocument, Validation, classify_path, glob_matches,
};

const UNIVERSAL_ENTRYPOINT_GLOBS: &[&str] = &[
    "src/lib.rs",
    "src/main.*",
    "src/index.*",
    "main.*",
    "index.*",
    "tests/**",
    "**/tests/**",
    "*.test.*",
    "**/*.test.*",
    "*.spec.*",
    "**/*.spec.*",
    "*_test.*",
    "**/*_test.*",
];

#[derive(Clone, Copy, Eq, Ord, PartialEq, PartialOrd)]
struct CloneSite<'a> {
    path: &'a RepoPath,
    span: Span,
    name: &'a str,
    effective_lines: u32,
}

/// Detects Type-2 AST clones (`GF020`) across all functions and methods in `docs`.
pub fn check_type2_clones(docs: &[UastDocument], config: &Config) -> Validation<(), Diagnostic> {
    let min_lines = config.limits.min_clone_lines;
    let min_nodes = config.limits.min_clone_nodes;
    let mut buckets: BTreeMap<u128, Vec<CloneSite<'_>>> = BTreeMap::new();

    for doc in docs {
        for callable in &doc.callables {
            if callable.effective_lines < min_lines || callable.ast_node_count < min_nodes {
                continue;
            }
            let Some(hash) = callable.structural_hash else {
                continue;
            };
            buckets.entry(hash).or_default().push(CloneSite {
                path: &doc.path,
                span: callable.span,
                name: &callable.name,
                effective_lines: callable.effective_lines,
            });
        }
    }

    let mut diags = Vec::new();
    for mut sites in buckets.into_values() {
        if sites.len() < 2 {
            continue;
        }
        sites.sort();
        emit_clone_group(&sites, min_lines, &mut diags);
    }
    Validation::<(), Diagnostic>::from_errors(diags)
}

fn emit_clone_group(sites: &[CloneSite<'_>], min_lines: u32, out: &mut Vec<Diagnostic>) {
    let origin = sites[0];
    for (idx, site) in sites.iter().enumerate() {
        let peer = if idx == 0 { sites[1] } else { origin };
        out.push(Diagnostic {
            rule_id: String::from("GF020"),
            severity: Severity::Error,
            message: format!(
                "Type-2 AST clone: callable `{}` duplicates `{}` at `{}:{}`",
                site.name,
                peer.name,
                peer.path,
                peer.span.start_line()
            ),
            remediation: String::from(
                "Extract the duplicated control-flow structure into a shared generic or parameterized function; never rename variables or literals cosmetically to mask copy-paste.",
            ),
            measured: site.effective_lines,
            threshold: min_lines,
            path: site.path.clone(),
            span: site.span,
        });
    }
}

fn is_entrypoint_file(path: &RepoPath, ctx: &ProjectContext, config: &Config) -> bool {
    let raw = path.as_str();
    if UNIVERSAL_ENTRYPOINT_GLOBS
        .iter()
        .any(|pat| glob_matches(pat, raw))
    {
        return true;
    }
    if config.entrypoints.iter().any(|pat| glob_matches(pat, raw)) {
        return true;
    }
    ctx.frameworks.iter().any(|fw| {
        fw.implicit_entrypoints()
            .iter()
            .any(|pat| glob_matches(pat, raw))
    })
}

fn build_reference_index(docs: &[UastDocument]) -> BTreeMap<&str, BTreeSet<&RepoPath>> {
    let mut index: BTreeMap<&str, BTreeSet<&RepoPath>> = BTreeMap::new();
    for doc in docs {
        for sym in &doc.referenced_symbols {
            index.entry(sym.as_str()).or_default().insert(&doc.path);
        }
        for imp in &doc.imports {
            for sym in &imp.symbols {
                index.entry(sym.as_str()).or_default().insert(&doc.path);
            }
        }
    }
    index
}

/// Detects exported symbols in non-entrypoint modules never referenced elsewhere (`GF021`).
pub fn check_dead_exports(
    docs: &[UastDocument],
    ctx: &ProjectContext,
    config: &Config,
) -> Validation<(), Diagnostic> {
    let ref_index = build_reference_index(docs);
    let mut diags = Vec::new();

    for doc in docs {
        let domain = classify_path(&doc.path, &config.domains);
        if domain != DomainKind::Code
            || doc.language == Language::Dockerfile
            || is_entrypoint_file(&doc.path, ctx, config)
        {
            continue;
        }
        for export in &doc.exports {
            let used_elsewhere = ref_index
                .get(export.symbol.as_str())
                .is_some_and(|paths| paths.iter().any(|p| *p != &doc.path));
            if !used_elsewhere {
                diags.push(Diagnostic {
                    rule_id: String::from("GF021"),
                    severity: Severity::Error,
                    message: format!(
                        "dead public export `{}` in `{}` is unused across the workspace",
                        export.symbol, doc.path
                    ),
                    remediation: String::from(
                        "Narrow symbol visibility to module-private (remove `pub` / `export`) or delete genuinely unused code instead of leaving dead public exports.",
                    ),
                    measured: 1,
                    threshold: 0,
                    path: doc.path.clone(),
                    span: export.span,
                });
            }
        }
    }
    Validation::<(), Diagnostic>::from_errors(diags)
}
