//! TypeScript/JavaScript (`test(...)` / `it(...)`) and Python (`def test_*`)
//! test-body assertion counters and tautology detectors (`GF076`).

use gf_core::TestCaseSummary;
use tree_sitter::Node;

use crate::cst::{field_name_string, node_span, node_text};

fn inspect_single_ts_call(node: Node<'_>, source: &str, stats: &mut (u32, bool)) {
    let Some(callee) = node.child_by_field_name("function") else {
        return;
    };
    let callee_txt: String = node_text(callee, source)
        .chars()
        .filter(|c| !c.is_whitespace())
        .collect();
    let is_expect = callee_txt.starts_with("expect(") && callee_txt.contains(").");
    let is_assert = callee_txt == "assert"
        || callee_txt.starts_with("assert.")
        || callee_txt.starts_with("assert_");
    if !(is_expect || is_assert) {
        return;
    }
    stats.0 = stats.0.saturating_add(1);
    let full_txt: String = node_text(node, source)
        .chars()
        .filter(|c| !c.is_whitespace())
        .collect();
    stats.1 |= matches!(
        full_txt.as_str(),
        "expect(true).toBe(true)"
            | "expect(true).toEqual(true)"
            | "expect(true).toBeTruthy()"
            | "expect(1).toBe(1)"
            | "assert(true)"
            | "assert.ok(true)"
    );
}

fn inspect_ts_test_body(node: Node<'_>, source: &str, stats: &mut (u32, bool)) {
    if node.kind() == "call_expression" {
        inspect_single_ts_call(node, source, stats);
    }
    let mut c = node.walk();
    for ch in node.children(&mut c) {
        inspect_ts_test_body(ch, source, stats);
    }
}

fn inspect_single_py_node(node: Node<'_>, source: &str, stats: &mut (u32, bool)) {
    if node.kind() == "assert_statement" {
        stats.0 = stats.0.saturating_add(1);
        let expr = node_text(node, source)
            .trim()
            .strip_prefix("assert")
            .map_or("", str::trim);
        stats.1 |= matches!(expr, "True" | "1 == 1" | "1==1");
    } else if node.kind() == "call"
        && let Some(fn_node) = node.child_by_field_name("function")
    {
        let callee = node_text(fn_node, source).trim();
        if callee.starts_with("self.assert")
            || callee.starts_with("pytest.raises")
            || callee.contains(".assert_")
        {
            stats.0 = stats.0.saturating_add(1);
            stats.1 |= node_text(node, source).replace(' ', "") == "self.assertTrue(True)";
        }
    }
}

fn inspect_py_test_body(node: Node<'_>, source: &str, stats: &mut (u32, bool)) {
    inspect_single_py_node(node, source, stats);
    let mut c = node.walk();
    for ch in node.children(&mut c) {
        inspect_py_test_body(ch, source, stats);
    }
}

/// Extracts a TypeScript/JS `test(...)` or `it(...)` [`TestCaseSummary`].
pub fn try_extract_ts_test_call(node: Node<'_>, source: &str) -> Option<TestCaseSummary> {
    let callee = node_text(node.child_by_field_name("function")?, source).trim();
    if !matches!(
        callee,
        "test" | "it" | "Deno.test" | "test.concurrent" | "it.concurrent"
    ) {
        return None;
    }
    let args = node.child_by_field_name("arguments")?;
    let mut cursor = args.walk();
    let named: Vec<Node<'_>> = args.named_children(&mut cursor).collect();
    let cb = named
        .iter()
        .copied()
        .find(|n| matches!(n.kind(), "arrow_function" | "function_expression"))?;
    let title = named
        .first()
        .map_or("<test>", |n| node_text(*n, source))
        .trim_matches(['"', '\'', '`']);
    let mut stats = (0_u32, false);
    inspect_ts_test_body(cb, source, &mut stats);
    Some(TestCaseSummary {
        name: String::from(title),
        assertion_count: stats.0,
        has_tautology: stats.1,
        span: node_span(node),
    })
}

/// Extracts a Python `def test_*` [`TestCaseSummary`].
pub fn try_extract_py_test_fn(node: Node<'_>, source: &str) -> Option<TestCaseSummary> {
    let name = field_name_string(node, "name", "", source);
    if !name.starts_with("test_") {
        return None;
    }
    let mut stats = (0_u32, false);
    if let Some(body) = node.child_by_field_name("body") {
        inspect_py_test_body(body, source, &mut stats);
    }
    Some(TestCaseSummary {
        name,
        assertion_count: stats.0,
        has_tautology: stats.1,
        span: node_span(node),
    })
}
