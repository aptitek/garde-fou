//! Pure `#![no_std]` cross-file evaluators for Cargo workspace dependency inheritance
//! (`TOML-WORKSPACE-BYPASS` -> `GF080`) and Markdown relative link/`#anchor` resolution
//! (`MD-UNRESOLVED-REF-OR-ANCHOR` -> `GF063`).

use alloc::collections::{BTreeMap, BTreeSet};
use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;

use gf_core::{
    AssetHazard, AssetHazardKind, Diagnostic, MarkdownViolation, RepoPath, TomlViolation,
    UastDocument, Validation,
};

use crate::checks::{SourceLoc, rule_error};

pub fn emit_svg_hazards(path: &RepoPath, hazards: &[AssetHazard], out: &mut Vec<Diagnostic>) {
    for haz in hazards {
        let (msg, rem) = if let AssetHazardKind::XmlSvgSpec(v) = haz.kind {
            (
                format!("[{}] unsafe SVG asset `{path}`: {}", v.code(), haz.detail),
                v.remediation(),
            )
        } else {
            (
                format!("unsafe SVG asset `{path}`: {}", haz.detail),
                "Sanitize `.svg` files to remove `<script>`, `on*=` event handlers, and embedded `data:` URIs.",
            )
        };
        out.push(rule_error(
            "GF064",
            msg,
            rem,
            SourceLoc::new(path, haz.span),
        ));
    }
}

fn parent_dir(path: &str) -> &str {
    path.rsplit_once('/').map_or("", |(dir, _)| dir)
}

fn resolve_relative_repo_path(base_dir: &str, rel_target: &str) -> String {
    let normalized = rel_target.replace('\\', "/");
    let mut stack: Vec<&str> = if normalized.starts_with('/') {
        Vec::new()
    } else {
        base_dir.split('/').filter(|s| !s.is_empty()).collect()
    };
    for seg in normalized.split('/') {
        match seg {
            "" | "." => {}
            ".." => {
                stack.pop();
            }
            part => stack.push(part),
        }
    }
    stack.join("/")
}

fn check_cargo_workspace_bypass(docs: &[UastDocument], out: &mut Vec<Diagnostic>) {
    let Some(root_manifest) = docs
        .iter()
        .find(|d| d.path.as_str() == "Cargo.toml")
        .and_then(|d| d.toml_manifest.as_ref())
        .filter(|m| m.is_workspace_root && !m.workspace_deps.is_empty())
    else {
        return;
    };

    for doc in docs {
        if doc.path.as_str() == "Cargo.toml"
            || !doc.path.file_name().eq_ignore_ascii_case("cargo.toml")
        {
            continue;
        }
        let Some(member) = &doc.toml_manifest else {
            continue;
        };
        for (dep_name, span) in &member.hardcoded_member_deps {
            if root_manifest.workspace_deps.contains(dep_name) {
                let v = TomlViolation::WorkspaceBypass;
                out.push(rule_error(
                    "GF080",
                    format!(
                        "[{}] crate `{}` hardcodes dependency `{dep_name}` instead of `{dep_name}.workspace = true`",
                        v.code(),
                        doc.path
                    ),
                    v.remediation(),
                    SourceLoc::new(&doc.path, *span),
                ));
            }
        }
    }
}

fn check_markdown_cross_links(docs: &[UastDocument], out: &mut Vec<Diagnostic>) {
    let mut slug_index: BTreeMap<&str, &BTreeSet<String>> = BTreeMap::new();
    for doc in docs {
        if let Some(contenus) = &doc.contenus {
            slug_index.insert(doc.path.as_str(), &contenus.heading_slugs);
        }
    }

    let v = MarkdownViolation::UnresolvedRefOrAnchor;
    for doc in docs {
        let Some(contenus) = &doc.contenus else {
            continue;
        };
        let base_dir = parent_dir(doc.path.as_str());
        for link in &contenus.doc_links {
            let resolved = resolve_relative_repo_path(base_dir, &link.target_path);
            let loc = SourceLoc::new(&doc.path, link.span);
            let Some(target_slugs) = slug_index.get(resolved.as_str()) else {
                out.push(rule_error(
                    "GF063",
                    format!(
                        "[{}] broken Markdown link `{}` -> missing document `{resolved}`",
                        v.code(),
                        link.target_path
                    ),
                    v.remediation(),
                    loc,
                ));
                continue;
            };
            if let Some(frag) = &link.fragment
                && !frag.is_empty()
                && !target_slugs.contains(frag.as_str())
            {
                out.push(rule_error(
                    "GF063",
                    format!(
                        "[{}] broken Markdown anchor `{}#{frag}` (no matching heading slug in `{resolved}`)",
                        v.code(),
                        link.target_path
                    ),
                    v.remediation(),
                    loc,
                ));
            }
        }
    }
}

pub fn check_workspace_formats(docs: &[UastDocument]) -> Validation<(), Diagnostic> {
    let mut diags = Vec::new();
    check_cargo_workspace_bypass(docs, &mut diags);
    check_markdown_cross_links(docs, &mut diags);
    Validation::<(), Diagnostic>::from_errors(diags)
}
