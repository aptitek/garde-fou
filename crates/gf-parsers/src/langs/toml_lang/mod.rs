//! Specification-grade `toml_edit` TOML 1.0/1.1 extractor (`GF000` & `GF080`).

mod manifests;
mod span_util;

use std::collections::BTreeSet;

use gf_core::{
    LangRuleFinding, Language, RepoPath, Span, SyntaxErrorNode, TomlViolation, UastDocument,
};
use toml_edit::{ArrayOfTables, ImDocument, Item, Table, Value};

use crate::cst::line_metrics;
use crate::types::{LanguageExtractor, ParseError};
use manifests::check_toml_manifests;
use span_util::col_from_byte_offset;
pub use span_util::span_from_byte_offset;

const MAX_INLINE_TABLE_KEYS: usize = 4;

fn check_inline_tables_in_value(source: &str, val: &Value, out: &mut Vec<LangRuleFinding>) {
    match val {
        Value::InlineTable(it) => {
            if it.len() > MAX_INLINE_TABLE_KEYS {
                let sp = it
                    .span()
                    .map_or(Span::FIRST, |r| span_from_byte_offset(source, r.start));
                out.push(LangRuleFinding::new(
                    TomlViolation::ConfusingTableReopen,
                    sp,
                    format!(
                        "inline table has {} keys (max {MAX_INLINE_TABLE_KEYS}); use a standard `[table]`",
                        it.len()
                    ),
                ));
            }
            for (_, v) in it {
                check_inline_tables_in_value(source, v, out);
            }
        }
        Value::Array(arr) => {
            for elem in arr {
                check_inline_tables_in_value(source, elem, out);
            }
        }
        _ => {}
    }
}

struct TomlHeaderPos {
    pos: usize,
    full_name: String,
    byte_off: usize,
}

fn collect_aot_headers(aot: &ArrayOfTables, full: &str, headers: &mut Vec<TomlHeaderPos>) {
    for entry in aot {
        if let Some(pos) = entry.position() {
            let byte_off = entry.span().map_or(0, |r| r.start);
            headers.push(TomlHeaderPos {
                pos,
                full_name: String::from(full),
                byte_off,
            });
        }
        collect_explicit_headers(entry, full, headers);
    }
}

fn collect_explicit_headers(tbl: &Table, prefix: &str, headers: &mut Vec<TomlHeaderPos>) {
    for (k, item) in tbl {
        let full = if prefix.is_empty() {
            String::from(k)
        } else {
            format!("{prefix}.{k}")
        };
        match item {
            Item::Table(sub) => {
                if !sub.is_dotted()
                    && let Some(pos) = sub.position()
                {
                    headers.push(TomlHeaderPos {
                        pos,
                        full_name: full.clone(),
                        byte_off: sub.span().map_or(0, |r| r.start),
                    });
                }
                collect_explicit_headers(sub, &full, headers);
            }
            Item::ArrayOfTables(aot) => collect_aot_headers(aot, &full, headers),
            Item::Value(_) | Item::None => {}
        }
    }
}

fn collect_table_events<'a>(
    tbl: &'a Table,
    inherited_top: Option<&'a str>,
    events: &mut Vec<(usize, Option<&'a str>)>,
) {
    for (k, item) in tbl {
        let off = tbl
            .key(k)
            .and_then(toml_edit::Key::span)
            .or_else(|| item.span())
            .map_or(0, |r| r.start);
        match item {
            Item::Table(sub) if sub.is_dotted() => {
                collect_table_events(sub, Some(inherited_top.unwrap_or(k)), events);
            }
            Item::Value(_) => events.push((off, inherited_top)),
            Item::Table(_) | Item::ArrayOfTables(_) | Item::None => {}
        }
    }
}

fn check_interleaved_dotted_in_table(source: &str, tbl: &Table, out: &mut Vec<LangRuleFinding>) {
    let mut events = Vec::new();
    collect_table_events(tbl, None, &mut events);
    events.sort_by_key(|(off, _)| *off);

    let mut closed_dotted: BTreeSet<&str> = BTreeSet::new();
    let mut active_dotted: Option<&str> = None;
    for (off, dotted_opt) in events {
        let Some(key) = dotted_opt else {
            if let Some(prev) = active_dotted.take() {
                closed_dotted.insert(prev);
            }
            continue;
        };
        if closed_dotted.contains(key) {
            out.push(LangRuleFinding::new(
                TomlViolation::ConfusingTableReopen,
                span_from_byte_offset(source, off),
                format!("dotted subtable `{key}.*` reopened out of order"),
            ));
        }
        if let Some(prev) = active_dotted.replace(key)
            && prev != key
        {
            closed_dotted.insert(prev);
        }
    }
}

fn walk_table_dotted_and_inline(source: &str, tbl: &Table, out: &mut Vec<LangRuleFinding>) {
    check_interleaved_dotted_in_table(source, tbl, out);
    for (_, item) in tbl {
        match item {
            Item::Table(sub) => walk_table_dotted_and_inline(source, sub, out),
            Item::ArrayOfTables(aot) => {
                for entry in aot {
                    walk_table_dotted_and_inline(source, entry, out);
                }
            }
            Item::Value(val) => check_inline_tables_in_value(source, val, out),
            Item::None => {}
        }
    }
}

fn check_out_of_order_headers(source: &str, root: &Table, out: &mut Vec<LangRuleFinding>) {
    let mut headers = Vec::new();
    collect_explicit_headers(root, "", &mut headers);
    headers.sort_by_key(|h| h.pos);

    let mut closed_roots: BTreeSet<String> = BTreeSet::new();
    let mut current_root: Option<String> = None;
    for header in headers {
        let full_name = header.full_name;
        let top_seg = full_name.split('.').next().unwrap_or(&full_name);
        if closed_roots.contains(top_seg) {
            out.push(LangRuleFinding::new(
                TomlViolation::ConfusingTableReopen,
                span_from_byte_offset(source, header.byte_off),
                format!(
                    "table `[{full_name}]` reopens top-level hierarchy `{top_seg}` after switching sections"
                ),
            ));
        }
        if let Some(prev) = current_root.replace(String::from(top_seg))
            && prev != top_seg
        {
            closed_roots.insert(prev);
        }
    }
}

/// Enriches `doc` in place with `toml_edit` syntax errors (`GF000`), table structure (`GF080`),
/// and supply-chain manifest checks (`GF080`).
pub fn analyze_toml_into_document(path: &RepoPath, source: &str, doc: &mut UastDocument) {
    match ImDocument::parse(source) {
        Ok(im_doc) => {
            let root = im_doc.as_table();
            walk_table_dotted_and_inline(source, root, &mut doc.lang_rule_findings);
            check_out_of_order_headers(source, root, &mut doc.lang_rule_findings);
            doc.toml_manifest =
                check_toml_manifests(path, source, &im_doc, &mut doc.lang_rule_findings);
        }
        Err(err) => {
            let byte_off = err.span().map_or(0, |r| r.start);
            let span = span_from_byte_offset(source, byte_off);
            let col = col_from_byte_offset(source, byte_off);
            doc.syntax_errors.push(SyntaxErrorNode {
                span,
                message: format!("col {col}: {}", err.message()),
            });
        }
    }
}

/// Specification-grade extractor for `.toml` documents and manifests.
#[derive(Clone, Copy, Debug, Default)]
pub struct TomlExtractor;

impl LanguageExtractor for TomlExtractor {
    fn extract(&self, path: &RepoPath, source: &str) -> Result<UastDocument, ParseError> {
        let (total_lines, effective_lines) = line_metrics(source);
        let mut doc =
            UastDocument::new(path.clone(), Language::Toml, total_lines, effective_lines)?;
        analyze_toml_into_document(path, source, &mut doc);
        Ok(doc)
    }
}
