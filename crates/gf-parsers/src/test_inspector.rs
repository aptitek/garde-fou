//! Rust `#[test]` assertion counter, AST reflexivity tautology detector, and weak-assertion
//! classifier, plus polyglot CST test dispatcher (`GF076`).

use gf_core::{Span, TestCaseSummary};
use tree_sitter::Node;

use crate::cst::{field_name_string, node_span, node_text};

/// Per-test accumulator separating strong behavioral assertions from weak existence checks.
#[derive(Clone, Copy, Debug, Default)]
pub struct TestAssertionStats {
    pub strong_assertions: u32,
    pub weak_assertions: u32,
    pub has_tautology: bool,
}

impl TestAssertionStats {
    pub const fn record_strong(&mut self, is_tautology: bool) {
        self.strong_assertions = self.strong_assertions.saturating_add(1);
        self.has_tautology |= is_tautology;
    }

    pub const fn record_weak(&mut self) {
        self.weak_assertions = self.weak_assertions.saturating_add(1);
    }

    pub const fn into_summary(self, name: String, span: Span) -> TestCaseSummary {
        let assertion_count = self.strong_assertions.saturating_add(self.weak_assertions);
        TestCaseSummary {
            name,
            assertion_count,
            has_tautology: self.has_tautology,
            has_weak_only_assertion: assertion_count > 0 && self.strong_assertions == 0,
            span,
        }
    }
}

pub fn compact_expr(raw: &str) -> String {
    raw.chars().filter(|c| !c.is_whitespace()).collect()
}

pub fn compact_named_arg(call_node: Node<'_>, idx: usize, source: &str) -> Option<String> {
    let args = call_node.child_by_field_name("arguments")?;
    let mut c = args.walk();
    args.named_children(&mut c)
        .nth(idx)
        .map(|n| compact_expr(node_text(n, source)))
}

fn is_rust_assert_macro(short: &str) -> bool {
    short == "assert"
        || short.starts_with("assert_")
        || short.starts_with("debug_assert")
        || short.starts_with("prop_assert")
}

#[derive(Clone, Copy, Default)]
struct RustTestAttrFlags {
    is_test: bool,
    should_panic: bool,
}

fn rust_test_attrs(fn_node: Node<'_>, source: &str) -> RustTestAttrFlags {
    let mut flags = RustTestAttrFlags::default();
    let mut prev = fn_node.prev_sibling();
    while let Some(sib) = prev {
        match sib.kind() {
            "attribute_item" => {
                let compact = compact_expr(node_text(sib, source));
                flags.is_test |= compact == "#[test]"
                    || compact.ends_with("::test]")
                    || compact.starts_with("#[test(")
                    || compact.starts_with("#[rstest");
                flags.should_panic |= compact.contains("should_panic");
                prev = sib.prev_sibling();
            }
            "line_comment" | "block_comment" => prev = sib.prev_sibling(),
            _ => break,
        }
    }
    flags
}

fn strip_single_outer_delim(raw: &str) -> &str {
    let t = raw.trim();
    for (open, close) in [('(', ')'), ('[', ']'), ('{', '}')] {
        if let Some(inner) = t.strip_prefix(open).and_then(|s| s.strip_suffix(close)) {
            return inner.trim();
        }
    }
    t
}

#[derive(Default)]
struct ArgSplitState {
    in_str: bool,
    prev_esc: bool,
    depth: u32,
}

impl ArgSplitState {
    const fn step(&mut self, ch: char) {
        if self.in_str {
            if ch == '"' && !self.prev_esc {
                self.in_str = false;
            }
            self.prev_esc = ch == '\\' && !self.prev_esc;
        } else if ch == '"' {
            self.in_str = true;
        } else if matches!(ch, '(' | '[' | '{') {
            self.depth = self.depth.saturating_add(1);
        } else if matches!(ch, ')' | ']' | '}') {
            self.depth = self.depth.saturating_sub(1);
        }
    }
}

fn split_top_level_args(inner: &str) -> Vec<&str> {
    let mut out = Vec::new();
    let mut start = 0_usize;
    let mut state = ArgSplitState::default();
    for (idx, ch) in inner.char_indices() {
        if !state.in_str && state.depth == 0 && ch == ',' {
            out.push(inner.get(start..idx).unwrap_or("").trim());
            start = idx + 1;
        } else {
            state.step(ch);
        }
    }
    let tail = inner.get(start..).unwrap_or("").trim();
    if !tail.is_empty() {
        out.push(tail);
    }
    out
}

fn is_reflexive_eq_condition(cond: &str) -> bool {
    let Some((lhs, rhs)) = cond.split_once("==") else {
        return false;
    };
    if rhs.contains('=') {
        return false;
    }
    let cl = compact_expr(lhs);
    !cl.is_empty() && cl == compact_expr(rhs)
}

fn classify_rust_macro(short: &str, full_text: &str, stats: &mut TestAssertionStats) {
    let Some((_, tail)) = full_text.split_once('!') else {
        stats.record_strong(false);
        return;
    };
    let args = split_top_level_args(strip_single_outer_delim(tail));
    if matches!(short, "assert_eq" | "prop_assert_eq" | "debug_assert_eq") {
        let eq_taut = args.first().zip(args.get(1)).is_some_and(|(a, b)| {
            let ca = compact_expr(a);
            !ca.is_empty() && ca == compact_expr(b)
        });
        stats.record_strong(eq_taut);
    } else if matches!(short, "assert" | "prop_assert" | "debug_assert") {
        let cond = args.first().copied().unwrap_or("");
        let compact = compact_expr(cond);
        let is_taut =
            matches!(compact.as_str(), "true" | "!false") || is_reflexive_eq_condition(cond);
        let is_weak = !is_taut
            && !compact.contains("&&")
            && (compact.ends_with(".is_ok()") || compact.ends_with(".is_some()"));
        if is_weak {
            stats.record_weak();
        } else {
            stats.record_strong(is_taut);
        }
    } else {
        stats.record_strong(false);
    }
}

fn inspect_single_rust_node(node: Node<'_>, source: &str, stats: &mut TestAssertionStats) {
    match node.kind() {
        "macro_invocation" => {
            let Some(mac) = node.child_by_field_name("macro") else {
                return;
            };
            let raw_mac = node_text(mac, source).trim();
            let short = raw_mac.rsplit("::").next().unwrap_or(raw_mac);
            if is_rust_assert_macro(short) {
                classify_rust_macro(short, node_text(node, source), stats);
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
            if leaf.starts_with("assert") {
                stats.record_strong(false);
            }
        }
        "return_expression" if node_text(node, source).contains("Err(") => {
            stats.record_strong(false);
        }
        _ => {}
    }
}

fn inspect_rust_test_body(node: Node<'_>, source: &str, stats: &mut TestAssertionStats) {
    inspect_single_rust_node(node, source, stats);
    let mut c = node.walk();
    for ch in node.children(&mut c) {
        inspect_rust_test_body(ch, source, stats);
    }
}

pub fn try_extract_rust_test_fn(node: Node<'_>, source: &str) -> Option<TestCaseSummary> {
    let flags = rust_test_attrs(node, source);
    if !flags.is_test {
        return None;
    }
    let mut stats = TestAssertionStats {
        strong_assertions: u32::from(flags.should_panic),
        weak_assertions: 0,
        has_tautology: false,
    };
    if let Some(body) = node.child_by_field_name("body") {
        inspect_rust_test_body(body, source, &mut stats);
    }
    Some(stats.into_summary(
        field_name_string(node, "name", "<test>", source),
        node_span(node),
    ))
}
