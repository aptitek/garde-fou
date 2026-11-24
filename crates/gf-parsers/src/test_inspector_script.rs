//! TypeScript/JavaScript (`test(...)` / `it(...)`) test-body assertion counter,
//! reflexivity tautology detector, and polyglot CST test dispatcher (`GF076`).

use gf_core::{TestCaseSummary, UastDocument};
use tree_sitter::Node;

use crate::cst::{field_name_string, node_span, node_text};
use crate::test_inspector::{
    TestAssertionStats, compact_expr, compact_named_arg, try_extract_rust_test_fn,
};
use crate::test_inspector_py::{try_extract_py_test_fn, try_extract_vacuous_py_test_class};

const TS_EQUALITY_MATCHERS: &[&str] = &["toBe", "toEqual", "toStrictEqual"];
const TS_ASSERT_EQUAL_FNS: &[&str] = &[
    "assert.equal",
    "assert.strictEqual",
    "assert.deepEqual",
    "assert.deepStrictEqual",
];

fn resolve_expect_call<'a>(obj: Node<'a>, source: &str) -> Option<(Node<'a>, bool)> {
    if obj.kind() == "call_expression"
        && obj
            .child_by_field_name("function")
            .is_some_and(|f| node_text(f, source).trim() == "expect")
    {
        return Some((obj, false));
    }
    if obj.kind() == "member_expression"
        && field_name_string(obj, "property", "", source) == "not"
        && let Some(inner) = obj.child_by_field_name("object")
        && inner.kind() == "call_expression"
        && inner
            .child_by_field_name("function")
            .is_some_and(|f| node_text(f, source).trim() == "expect")
    {
        return Some((inner, true));
    }
    None
}

#[derive(Clone, Copy)]
struct TsExpectArgs<'a> {
    matcher: &'a str,
    negated: bool,
    actual: &'a str,
    expected: Option<&'a str>,
}

fn is_ts_tautological_expect(args: TsExpectArgs<'_>) -> bool {
    if !args.negated
        && TS_EQUALITY_MATCHERS.contains(&args.matcher)
        && !args.actual.is_empty()
        && args.expected == Some(args.actual)
    {
        return true;
    }
    let target_bool = if args.negated {
        "toBeFalsy"
    } else {
        "toBeTruthy"
    };
    args.actual == "true" && args.matcher == target_bool
}

fn is_ts_weak_expect(args: TsExpectArgs<'_>) -> bool {
    if args.negated {
        args.matcher == "toBeUndefined"
    } else {
        matches!(args.matcher, "toBeDefined" | "toBeTruthy")
    }
}

fn record_ts_matcher_outcome(args: TsExpectArgs<'_>, stats: &mut TestAssertionStats) {
    let is_taut = is_ts_tautological_expect(args);
    if !is_taut && is_ts_weak_expect(args) {
        stats.record_weak();
    } else {
        stats.record_strong(is_taut);
    }
}

fn try_inspect_ts_expect_chain(
    call_node: Node<'_>,
    callee: Node<'_>,
    source: &str,
    stats: &mut TestAssertionStats,
) -> bool {
    if callee.kind() != "member_expression" {
        return false;
    }
    let Some(obj) = callee.child_by_field_name("object") else {
        return false;
    };
    let Some((expect_call, negated)) = resolve_expect_call(obj, source) else {
        return false;
    };
    let matcher = field_name_string(callee, "property", "", source);
    let actual = compact_named_arg(expect_call, 0, source).unwrap_or_default();
    let expected = compact_named_arg(call_node, 0, source);
    record_ts_matcher_outcome(
        TsExpectArgs {
            matcher: &matcher,
            negated,
            actual: &actual,
            expected: expected.as_deref(),
        },
        stats,
    );
    true
}

fn inspect_single_ts_call(node: Node<'_>, source: &str, stats: &mut TestAssertionStats) {
    let Some(callee) = node.child_by_field_name("function") else {
        return;
    };
    if try_inspect_ts_expect_chain(node, callee, source, stats) {
        return;
    }
    let callee_txt = compact_expr(node_text(callee, source));
    if callee_txt != "assert"
        && !callee_txt.starts_with("assert.")
        && !callee_txt.starts_with("assert_")
    {
        return;
    }
    let arg0 = compact_named_arg(node, 0, source).unwrap_or_default();
    let arg1 = compact_named_arg(node, 1, source);
    let is_taut = if TS_ASSERT_EQUAL_FNS.contains(&callee_txt.as_str()) {
        !arg0.is_empty() && arg1.as_deref() == Some(arg0.as_str())
    } else {
        matches!(callee_txt.as_str(), "assert" | "assert.ok")
            && matches!(arg0.as_str(), "true" | "!false")
    };
    stats.record_strong(is_taut);
}

fn inspect_ts_test_body(node: Node<'_>, source: &str, stats: &mut TestAssertionStats) {
    if node.kind() == "call_expression" {
        inspect_single_ts_call(node, source, stats);
    }
    let mut c = node.walk();
    for ch in node.children(&mut c) {
        inspect_ts_test_body(ch, source, stats);
    }
}

fn try_extract_ts_test_call(node: Node<'_>, source: &str) -> Option<TestCaseSummary> {
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
    let mut stats = TestAssertionStats::default();
    inspect_ts_test_body(cb, source, &mut stats);
    Some(stats.into_summary(String::from(title), node_span(node)))
}

/// Extracts [`TestCaseSummary`] records across Rust, TypeScript/JS, and Python CSTs (`GF076`).
pub fn extract_test_cases(root: Node<'_>, source: &str, doc: &mut UastDocument) {
    let fname = doc.path.file_name();
    let in_py_tests = fname.starts_with("test_")
        || fname.ends_with("_test.py")
        || doc.path.as_str().starts_with("tests/")
        || doc.path.as_str().contains("/tests/");
    walk_test_cases(root, source, in_py_tests, doc);
}

fn walk_test_cases(node: Node<'_>, source: &str, in_py_tests: bool, doc: &mut UastDocument) {
    let extracted = match node.kind() {
        "function_item" => try_extract_rust_test_fn(node, source),
        "function_definition" => try_extract_py_test_fn(node, source),
        "class_definition" => try_extract_vacuous_py_test_class(node, source, in_py_tests),
        "call_expression" => try_extract_ts_test_call(node, source),
        _ => None,
    };
    if let Some(tc) = extracted {
        doc.test_cases.push(tc);
    }
    let mut c = node.walk();
    for ch in node.children(&mut c) {
        walk_test_cases(ch, source, in_py_tests, doc);
    }
}
