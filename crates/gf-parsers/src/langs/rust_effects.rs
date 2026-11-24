//! Rust `[architecture]` effect-sighting extractor (`GF078`) and module boundary
//! (`use`, inline qualified path, and `pub`/`pub(crate)` export) resolver (`GF021`, `GF071`, `GF072`).

use gf_core::{EffectSighting, EffectSightingKind, ExportDecl, ImportDecl, Span, UastDocument};
use tree_sitter::Node;

use crate::cst::{field_name_string, node_span, node_text};

const IO_STD_MODULES: &[&str] = &["fs", "net", "process", "env"];

fn is_cfg_test_attr(attr_node: Node<'_>, source: &str) -> bool {
    let raw = node_text(attr_node, source).trim();
    let inner = raw
        .trim_start_matches("#!")
        .trim_start_matches('#')
        .trim()
        .strip_prefix('[')
        .and_then(|s| s.strip_suffix(']'))
        .map_or("", str::trim);
    let compact: String = inner.chars().filter(|c| !c.is_whitespace()).collect();
    if !compact.starts_with("cfg(") || compact.contains("not(test)") {
        return false;
    }
    inner
        .split(|c: char| !c.is_ascii_alphanumeric() && c != '_')
        .any(|tok| tok == "test")
}

/// Returns `true` when `node` has a preceding outer `#[cfg(test)]` sibling attribute.
pub fn has_preceding_cfg_test_attr(node: Node<'_>, source: &str) -> bool {
    std::iter::successors(node.prev_sibling(), Node::prev_sibling)
        .take_while(|s| {
            matches!(
                s.kind(),
                "attribute_item" | "line_comment" | "block_comment"
            )
        })
        .any(|s| s.kind() == "attribute_item" && is_cfg_test_attr(s, source))
}

fn adjust_super_for_inline_mods(raw: &str, node: Node<'_>) -> String {
    let mod_depth = std::iter::successors(node.parent(), Node::parent)
        .filter(|p| p.kind() == "mod_item")
        .count();
    if mod_depth == 0 || !raw.starts_with("super::") {
        return String::from(raw);
    }
    let mut rest = raw;
    for _ in 0..mod_depth {
        let Some(after) = rest.strip_prefix("super::") else {
            break;
        };
        rest = after;
    }
    if rest.starts_with("super::") {
        String::from(rest)
    } else {
        format!("self::{rest}")
    }
}

/// Extracts a Rust `use` declaration into an [`ImportDecl`].
pub fn extract_use(node: Node<'_>, source: &str) -> ImportDecl {
    let raw = node
        .child_by_field_name("argument")
        .map_or_else(|| node_text(node, source), |arg| node_text(arg, source))
        .trim();
    let adjusted = adjust_super_for_inline_mods(raw, node);
    let symbols = raw
        .split_once('{')
        .and_then(|(_, rest)| rest.rsplit_once('}'))
        .map_or_else(Vec::new, |(inner, _)| {
            inner
                .split(',')
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .map(String::from)
                .collect()
        });
    ImportDecl {
        module_source: adjusted,
        symbols,
        is_type_only: false,
        span: node_span(node),
    }
}

/// Pushes a top-level `ExportDecl` (`is_crate_internal = true` for `pub(crate)` / `pub(super)`).
pub fn maybe_push_export(node: Node<'_>, symbol: &str, source: &str, doc: &mut UastDocument) {
    if node.parent().is_none_or(|p| p.kind() != "source_file") {
        return;
    }
    let mut cursor = node.walk();
    let Some(vis_node) = node
        .children(&mut cursor)
        .find(|c| c.kind() == "visibility_modifier")
    else {
        return;
    };
    let vis_text: String = node_text(vis_node, source)
        .chars()
        .filter(|c| !c.is_whitespace())
        .collect();
    let is_crate_internal = match vis_text.as_str() {
        "pub" => false,
        "pub(self)" => return,
        s if s.starts_with("pub(") => true,
        _ => return,
    };
    doc.exports.push(ExportDecl {
        symbol: String::from(symbol),
        is_crate_internal,
        is_type_only: false,
        span: node_span(node),
    });
}

pub fn push_effect(doc: &mut UastDocument, kind: EffectSightingKind, detail: &str, span: Span) {
    doc.effect_sightings.push(EffectSighting {
        kind,
        detail: String::from(detail),
        span,
    });
}

fn record_std_and_io_effects(clean_path: &str, span: Span, doc: &mut UastDocument) {
    if clean_path == "std" || clean_path.starts_with("std::") || clean_path.starts_with("std{") {
        push_effect(doc, EffectSightingKind::StdUsage, clean_path, span);
        let hits_io = IO_STD_MODULES.iter().any(|m| {
            clean_path.contains(&format!("std::{m}"))
                || (clean_path.starts_with("std::{") && clean_path.contains(m))
        });
        if hits_io {
            push_effect(doc, EffectSightingKind::IoImportOrCall, clean_path, span);
        }
    }
}

fn has_use_ancestor(mut node: Node<'_>) -> bool {
    while let Some(p) = node.parent() {
        if p.kind() == "use_declaration" {
            return true;
        }
        node = p;
    }
    false
}

fn extract_inline_module_source(trimmed: &str, node: Node<'_>) -> Option<String> {
    let adjusted = adjust_super_for_inline_mods(trimmed, node);
    let parts: Vec<&str> = adjusted.split("::").collect();
    let &head = parts.first()?;
    if parts.len() < 2 || !matches!(head, "crate" | "super") {
        return None;
    }
    let mod_segs: Vec<&str> = parts
        .iter()
        .copied()
        .take_while(|s| s.bytes().next().is_some_and(|b| b.is_ascii_lowercase()))
        .collect();
    Some(if mod_segs.len() >= 2 {
        mod_segs.join("::")
    } else {
        format!("{head}::{}", parts[1])
    })
}

fn inspect_scoped_path(node: Node<'_>, source: &str, in_cfg_test: bool, doc: &mut UastDocument) {
    if node
        .parent()
        .is_some_and(|p| matches!(p.kind(), "scoped_identifier" | "scoped_type_identifier"))
        || has_use_ancestor(node)
    {
        return;
    }
    let clean: String = node_text(node, source)
        .chars()
        .filter(|c| !c.is_whitespace())
        .collect();
    let trimmed = clean.trim_start_matches("::");
    let span = node_span(node);
    if !in_cfg_test {
        record_std_and_io_effects(trimmed, span, doc);
    }
    if let Some(mod_source) = extract_inline_module_source(trimmed, node)
        && !doc
            .imports
            .iter()
            .any(|imp| imp.module_source == mod_source)
    {
        doc.imports.push(ImportDecl {
            module_source: mod_source,
            symbols: Vec::new(),
            is_type_only: false,
            span,
        });
    }
}

/// Records `#![no_std]`, `std::` / I/O usage, and inline qualified imports on `node`.
pub fn inspect_rust_effect_node(
    node: Node<'_>,
    source: &str,
    in_cfg_test: bool,
    doc: &mut UastDocument,
) {
    match node.kind() {
        "inner_attribute_item" => {
            let compact: String = node_text(node, source)
                .chars()
                .filter(|c| !c.is_whitespace())
                .collect();
            if compact == "#![no_std]" {
                push_effect(
                    doc,
                    EffectSightingKind::HasNoStdAttr,
                    "#![no_std]",
                    node_span(node),
                );
            }
        }
        "extern_crate_declaration"
            if !in_cfg_test
                && !has_preceding_cfg_test_attr(node, source)
                && field_name_string(node, "name", "", source) == "std" =>
        {
            push_effect(
                doc,
                EffectSightingKind::StdUsage,
                "extern crate std",
                node_span(node),
            );
        }
        "use_declaration" if !in_cfg_test && !has_preceding_cfg_test_attr(node, source) => {
            if let Some(arg) = node.child_by_field_name("argument") {
                let compact: String = node_text(arg, source)
                    .chars()
                    .filter(|c| !c.is_whitespace())
                    .collect();
                record_std_and_io_effects(compact.trim_start_matches("::"), node_span(node), doc);
            }
        }
        "scoped_identifier" | "scoped_type_identifier" => {
            inspect_scoped_path(node, source, in_cfg_test, doc);
        }
        _ => {}
    }
}
