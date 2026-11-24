//! Python (`def test_*`, `async def test_*`, `class Test*`) `pytest` & `unittest`
//! assertion counter, tautology detector, and weak-assertion classifier (`GF076`).

use gf_core::TestCaseSummary;
use tree_sitter::Node;

use crate::cst::{field_name_string, node_span, node_text};
use crate::langs::python_typing::has_dataclass_decorator;
use crate::test_inspector::{TestAssertionStats, compact_expr, compact_named_arg};

fn normalize_py_scalar_expr(raw: &str) -> String {
    let c = compact_expr(raw);
    c.strip_prefix('\'')
        .and_then(|s| s.strip_suffix('\''))
        .filter(|s| !s.contains('\''))
        .map_or_else(|| c.clone(), |inner| format!("\"{inner}\""))
}

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
        let cl = normalize_py_scalar_expr(node_text(*lhs, source));
        let cr = normalize_py_scalar_expr(node_text(*rhs, source));
        stats.record_strong(cl == cr);
        return true;
    }
    false
}

fn is_constant_truthy_py_literal(cond: Node<'_>, compact: &str) -> bool {
    if matches!(compact, "True" | "notFalse") {
        return true;
    }
    match cond.kind() {
        "integer" => compact != "0",
        "string" => !matches!(compact, "\"\"" | "''"),
        "list" | "tuple" | "set" | "dictionary" => cond.named_child(0).is_some(),
        _ => false,
    }
}

fn inspect_py_assert_stmt(node: Node<'_>, source: &str, stats: &mut TestAssertionStats) {
    let Some(cond) = node.named_child(0) else {
        return;
    };
    if !try_inspect_py_comparison(cond, source, stats) {
        let compact = compact_expr(node_text(cond, source));
        stats.record_strong(is_constant_truthy_py_literal(cond, &compact));
    }
}

fn is_pytest_assertion_callee(callee: &str) -> bool {
    callee.starts_with("self.assert")
        || callee.contains(".assert_")
        || matches!(
            callee,
            "pytest.raises"
                | "raises"
                | "pytest.warns"
                | "warns"
                | "pytest.fail"
                | "fail"
                | "self.fail"
        )
}

fn is_vacuous_with_raises_call(call_node: Node<'_>, callee: &str) -> bool {
    if !matches!(
        callee,
        "pytest.raises" | "raises" | "pytest.warns" | "warns"
    ) {
        return false;
    }
    let mut cur = call_node.parent();
    while let Some(p) = cur {
        if p.kind() == "with_statement" {
            let Some(body) = p.child_by_field_name("body") else {
                return false;
            };
            let mut cursor = body.walk();
            return body.named_children(&mut cursor).all(|st| {
                st.kind() == "pass_statement"
                    || st.kind() == "comment"
                    || (st.kind() == "expression_statement"
                        && st.named_child(0).is_some_and(|c| c.kind() == "ellipsis"))
            });
        }
        cur = p.parent();
    }
    false
}

fn inspect_py_assert_call(node: Node<'_>, source: &str, stats: &mut TestAssertionStats) {
    let Some(fn_node) = node.child_by_field_name("function") else {
        return;
    };
    let callee = compact_expr(node_text(fn_node, source));
    if !is_pytest_assertion_callee(&callee) || is_vacuous_with_raises_call(node, &callee) {
        return;
    }
    if callee == "self.assertIsNotNone" {
        stats.record_weak();
        return;
    }
    let is_taut = if matches!(callee.as_str(), "self.assertEqual" | "self.assertIs") {
        if let (Some(lhs), Some(rhs)) = (
            compact_named_arg(node, 0, source),
            compact_named_arg(node, 1, source),
        ) {
            normalize_py_scalar_expr(&lhs) == normalize_py_scalar_expr(&rhs)
        } else {
            false
        }
    } else {
        callee == "self.assertTrue" && compact_named_arg(node, 0, source).as_deref() == Some("True")
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

fn enclosing_py_test_class(fn_node: Node<'_>, source: &str) -> Option<String> {
    let mut cur = fn_node.parent();
    while let Some(p) = cur {
        if p.kind() == "class_definition" {
            let cls = field_name_string(p, "name", "", source);
            return cls.starts_with("Test").then_some(cls);
        }
        cur = p.parent();
    }
    None
}

/// Extracts a Python `def test_*` or `async def test_*` [`TestCaseSummary`].
pub fn try_extract_py_test_fn(node: Node<'_>, source: &str) -> Option<TestCaseSummary> {
    let name = field_name_string(node, "name", "", source);
    if !name.starts_with("test_") {
        return None;
    }
    let full_name = enclosing_py_test_class(node, source)
        .map_or_else(|| name.clone(), |cls| format!("{cls}::{name}"));
    let mut stats = TestAssertionStats::default();
    if let Some(body) = node.child_by_field_name("body") {
        inspect_py_test_body(body, source, &mut stats);
    }
    Some(stats.into_summary(full_name, node_span(node)))
}

fn stmt_is_init_or_test_method(stmt: Node<'_>, source: &str) -> bool {
    let def = if stmt.kind() == "decorated_definition" {
        stmt.child_by_field_name("definition")
    } else {
        Some(stmt)
    };
    let Some(fn_node) = def.filter(|d| d.kind() == "function_definition") else {
        return false;
    };
    let m_name = field_name_string(fn_node, "name", "", source);
    m_name == "__init__" || m_name.starts_with("test_")
}

/// Flags vacuous `class Test*:` definitions containing zero `test_*` methods (`GF076`).
pub fn try_extract_vacuous_py_test_class(
    node: Node<'_>,
    source: &str,
    in_tests_file: bool,
) -> Option<TestCaseSummary> {
    if !in_tests_file || has_dataclass_decorator(node, source) {
        return None;
    }
    let cls_name = field_name_string(node, "name", "", source);
    if !cls_name.starts_with("Test") || node.child_by_field_name("superclasses").is_some() {
        return None;
    }
    let body = node.child_by_field_name("body")?;
    let mut cursor = body.walk();
    if body
        .named_children(&mut cursor)
        .any(|stmt| stmt_is_init_or_test_method(stmt, source))
    {
        return None;
    }
    Some(TestAssertionStats::default().into_summary(cls_name, node_span(node)))
}
