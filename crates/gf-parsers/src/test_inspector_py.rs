//! Python (`def test_*`) `assert` and `unittest`/`pytest` assertion counter,
//! reflexivity tautology detector, and weak-assertion classifier (`GF076`).

use gf_core::TestCaseSummary;
use tree_sitter::Node;

use crate::cst::{field_name_string, node_span, node_text};
use crate::test_inspector::{TestAssertionStats, compact_expr, compact_named_arg};

fn try_inspect_py_comparison(cond: Node<'_>, source: &str, stats: &mut TestAssertionStats) -> bool {
    if cond.kind() != "comparison_operator" {
        return false;
    }
    let mut c = cond.walk();
    let operands: Vec<Node<'_>> = cond.named_children(&mut c).collect();
    let [lhs, rhs] = operands.as_slice() else {
        return false;
    };
    let op: String = source
        .get(lhs.end_byte()..rhs.start_byte())
        .unwrap_or("")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");
    if op == "is not" && node_text(*rhs, source).trim() == "None" {
        stats.record_weak();
        return true;
    }
    if matches!(op.as_str(), "==" | "is") {
        let cl = compact_expr(node_text(*lhs, source));
        stats.record_strong(!cl.is_empty() && cl == compact_expr(node_text(*rhs, source)));
        return true;
    }
    false
}

fn inspect_py_assert_stmt(node: Node<'_>, source: &str, stats: &mut TestAssertionStats) {
    let Some(cond) = node.named_child(0) else {
        return;
    };
    if !try_inspect_py_comparison(cond, source, stats) {
        let compact = compact_expr(node_text(cond, source));
        stats.record_strong(matches!(compact.as_str(), "True" | "notFalse"));
    }
}

fn inspect_py_assert_call(node: Node<'_>, source: &str, stats: &mut TestAssertionStats) {
    let Some(fn_node) = node.child_by_field_name("function") else {
        return;
    };
    let callee = compact_expr(node_text(fn_node, source));
    if !callee.starts_with("self.assert")
        && !callee.starts_with("pytest.raises")
        && !callee.contains(".assert_")
    {
        return;
    }
    if callee == "self.assertIsNotNone" {
        stats.record_weak();
        return;
    }
    let arg0 = compact_named_arg(node, 0, source).unwrap_or_default();
    let arg1 = compact_named_arg(node, 1, source);
    let is_taut = if matches!(callee.as_str(), "self.assertEqual" | "self.assertIs") {
        !arg0.is_empty() && arg1.as_deref() == Some(arg0.as_str())
    } else {
        callee == "self.assertTrue" && arg0 == "True"
    };
    stats.record_strong(is_taut);
}

fn inspect_py_test_body(node: Node<'_>, source: &str, stats: &mut TestAssertionStats) {
    match node.kind() {
        "assert_statement" => inspect_py_assert_stmt(node, source, stats),
        "call" => inspect_py_assert_call(node, source, stats),
        _ => {}
    }
    let mut c = node.walk();
    for ch in node.children(&mut c) {
        inspect_py_test_body(ch, source, stats);
    }
}

/// Extracts a Python `def test_*` [`TestCaseSummary`].
pub fn try_extract_py_test_fn(node: Node<'_>, source: &str) -> Option<TestCaseSummary> {
    let name = field_name_string(node, "name", "", source);
    if !name.starts_with("test_") {
        return None;
    }
    let mut stats = TestAssertionStats::default();
    if let Some(body) = node.child_by_field_name("body") {
        inspect_py_test_body(body, source, &mut stats);
    }
    Some(stats.into_summary(name, node_span(node)))
}
