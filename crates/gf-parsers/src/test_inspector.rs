//! Rust `#[test]` assertion counter & tautology detector, plus polyglot CST test dispatcher (`GF076`).

use gf_core::{TestCaseSummary, UastDocument};
use tree_sitter::Node;

use crate::cst::{field_name_string, node_span, node_text};
use crate::test_inspector_script::{try_extract_py_test_fn, try_extract_ts_test_call};

fn is_rust_assert_macro(short: &str) -> bool {
    short == "assert"
        || short.starts_with("assert_")
        || short.starts_with("debug_assert")
        || short.starts_with("prop_assert")
}

fn parse_rust_attr_flags(sib: Node<'_>, source: &str) -> (bool, bool) {
    let compact: String = node_text(sib, source)
        .chars()
        .filter(|c| !c.is_whitespace())
        .collect();
    let is_test = compact == "#[test]"
        || compact.ends_with("::test]")
        || compact.starts_with("#[test(")
        || compact.starts_with("#[rstest");
    (is_test, compact.contains("should_panic"))
}

fn rust_test_attrs(fn_node: Node<'_>, source: &str) -> (bool, bool) {
    let mut flags = (false, false);
    let mut prev = fn_node.prev_sibling();
    while let Some(sib) = prev {
        match sib.kind() {
            "attribute_item" => {
                let (t, sp) = parse_rust_attr_flags(sib, source);
                flags.0 |= t;
                flags.1 |= sp;
                prev = sib.prev_sibling();
            }
            "line_comment" | "block_comment" => prev = sib.prev_sibling(),
            _ => break,
        }
    }
    flags
}

fn is_identical_trivial_pair(args_inner: &str) -> bool {
    let Some((lhs, rhs)) = args_inner.split_once(',') else {
        return false;
    };
    let a = lhs.trim();
    let b = rhs.trim().trim_end_matches(',');
    !b.contains(',') && matches!(a, "true" | "false" | "0" | "1") && a == b
}

fn is_rust_macro_tautology(short: &str, full_text: &str) -> bool {
    let Some((_, tail)) = full_text.split_once('!') else {
        return false;
    };
    let inner = tail
        .trim()
        .trim_matches(['(', ')', '[', ']', '{', '}'])
        .trim();
    (matches!(short, "assert" | "debug_assert" | "prop_assert") && inner == "true")
        || (matches!(short, "assert_eq" | "prop_assert_eq") && is_identical_trivial_pair(inner))
}

fn inspect_single_rust_node(node: Node<'_>, source: &str, stats: &mut (u32, bool)) {
    match node.kind() {
        "macro_invocation" => {
            let Some(mac) = node.child_by_field_name("macro") else {
                return;
            };
            let raw_mac = node_text(mac, source).trim();
            let short = raw_mac.rsplit("::").next().unwrap_or(raw_mac);
            if is_rust_assert_macro(short) {
                stats.0 = stats.0.saturating_add(1);
                stats.1 |= is_rust_macro_tautology(short, node_text(node, source));
            }
        }
        "call_expression" => {
            let Some(callee) = node.child_by_field_name("function") else {
                return;
            };
            let leaf = node_text(callee, source)
                .rsplit([':', '.'])
                .next()
                .unwrap_or("")
                .trim();
            if leaf.starts_with("assert") || leaf.starts_with("verify") || leaf.starts_with("check")
            {
                stats.0 = stats.0.saturating_add(1);
            }
        }
        "return_expression" if node_text(node, source).contains("Err(") => {
            stats.0 = stats.0.saturating_add(1);
        }
        _ => {}
    }
}

fn inspect_rust_test_body(node: Node<'_>, source: &str, stats: &mut (u32, bool)) {
    inspect_single_rust_node(node, source, stats);
    let mut c = node.walk();
    for ch in node.children(&mut c) {
        inspect_rust_test_body(ch, source, stats);
    }
}

fn try_extract_rust_test_fn(node: Node<'_>, source: &str) -> Option<TestCaseSummary> {
    let (is_test, should_panic) = rust_test_attrs(node, source);
    if !is_test {
        return None;
    }
    let mut stats = (u32::from(should_panic), false);
    if let Some(body) = node.child_by_field_name("body") {
        inspect_rust_test_body(body, source, &mut stats);
    }
    Some(TestCaseSummary {
        name: field_name_string(node, "name", "<test>", source),
        assertion_count: stats.0,
        has_tautology: stats.1,
        span: node_span(node),
    })
}

/// Extracts [`TestCaseSummary`] records across Rust, TypeScript/JS, and Python CSTs (`GF076`).
pub fn extract_test_cases(root: Node<'_>, source: &str, doc: &mut UastDocument) {
    walk_tests(root, source, doc);
}

fn walk_tests(node: Node<'_>, source: &str, doc: &mut UastDocument) {
    let extracted = match node.kind() {
        "function_item" => try_extract_rust_test_fn(node, source),
        "function_definition" => try_extract_py_test_fn(node, source),
        "call_expression" => try_extract_ts_test_call(node, source),
        _ => None,
    };
    if let Some(tc) = extracted {
        doc.test_cases.push(tc);
    }
    let mut c = node.walk();
    for ch in node.children(&mut c) {
        walk_tests(ch, source, doc);
    }
}
