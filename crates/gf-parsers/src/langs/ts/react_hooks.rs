//! React 19 Rules of Hooks (`REACT-RULES-OF-HOOKS`, including React 19 `use()` exemption and
//! early-`return` detection) and unstable nested component detector (`REACT-NO-UNSTABLE-NESTED-COMPONENT`).

use gf_core::{LangRuleFinding, ReactViolation};
use tree_sitter::Node;

use super::react_classify::{
    EnclosingFnKind, FN_NODE_KINDS, classify_fn_node, classify_hook_call, contains_own_early_exit,
    is_pascal_case_name,
};
use crate::cst::{node_span, node_text};

#[derive(Clone, Copy)]
struct HookWalkScope {
    enclosing: EnclosingFnKind,
    in_conditional_or_loop: bool,
    after_early_return: bool,
}

fn inspect_hook_call(
    call_node: Node<'_>,
    scope: HookWalkScope,
    source: &str,
    out: &mut Vec<LangRuleFinding>,
) {
    let Some((hook_name, is_react19_use)) = classify_hook_call(call_node, source) else {
        return;
    };
    if !scope.enclosing.is_hook_host() {
        out.push(LangRuleFinding::new(
            ReactViolation::RulesOfHooks,
            node_span(call_node),
            format!(
                "React Hook `{hook_name}` called outside a React function component or custom `use*` Hook"
            ),
        ));
    } else if !is_react19_use && (scope.in_conditional_or_loop || scope.after_early_return) {
        let reason = if scope.after_early_return {
            "after a conditional early `return`"
        } else {
            "inside a condition, loop, or `try`/`catch` block"
        };
        out.push(LangRuleFinding::new(
            ReactViolation::RulesOfHooks,
            node_span(call_node),
            format!("React Hook `{hook_name}` is called {reason}, breaking Hook call order"),
        ));
    }
}

const FLOW_BRANCH_KINDS: &[&str] = &[
    "for_statement",
    "for_in_statement",
    "while_statement",
    "do_statement",
    "switch_case",
    "switch_default",
    "try_statement",
    "catch_clause",
    "finally_clause",
];

fn is_short_circuit_binary(node: Node<'_>, source: &str) -> bool {
    node.kind() == "binary_expression"
        && node
            .child_by_field_name("operator")
            .is_some_and(|op| matches!(node_text(op, source).trim(), "&&" | "||" | "??"))
}

fn walk_conditional_node(
    node: Node<'_>,
    scope: HookWalkScope,
    source: &str,
    out: &mut Vec<LangRuleFinding>,
) {
    if let Some(cond) = node.child_by_field_name("condition") {
        walk_react_hooks(cond, scope, source, out);
    }
    let branched = HookWalkScope {
        in_conditional_or_loop: true,
        ..scope
    };
    for field in ["consequence", "alternative"] {
        if let Some(arm) = node.child_by_field_name(field) {
            walk_react_hooks(arm, branched, source, out);
        }
    }
}

fn walk_children_with_flow(
    node: Node<'_>,
    scope: HookWalkScope,
    source: &str,
    out: &mut Vec<LangRuleFinding>,
) {
    if matches!(node.kind(), "if_statement" | "ternary_expression") {
        walk_conditional_node(node, scope, source, out);
        return;
    }
    if is_short_circuit_binary(node, source) {
        if let Some(lhs) = node.child_by_field_name("left") {
            walk_react_hooks(lhs, scope, source, out);
        }
        if let Some(rhs) = node.child_by_field_name("right") {
            let rhs_scope = HookWalkScope {
                in_conditional_or_loop: true,
                ..scope
            };
            walk_react_hooks(rhs, rhs_scope, source, out);
        }
        return;
    }
    let child_scope = HookWalkScope {
        in_conditional_or_loop: scope.in_conditional_or_loop
            || FLOW_BRANCH_KINDS.contains(&node.kind()),
        ..scope
    };
    let mut cursor = node.walk();
    for ch in node.children(&mut cursor) {
        walk_react_hooks(ch, child_scope, source, out);
    }
}

fn walk_hook_host_stmts(
    body: Node<'_>,
    mut scope: HookWalkScope,
    source: &str,
    out: &mut Vec<LangRuleFinding>,
) {
    let mut cursor = body.walk();
    for stmt in body.named_children(&mut cursor) {
        walk_react_hooks(stmt, scope, source, out);
        if contains_own_early_exit(stmt) {
            scope.after_early_return = true;
        }
    }
}

fn walk_fn_boundary(
    fn_node: Node<'_>,
    parent_scope: HookWalkScope,
    source: &str,
    out: &mut Vec<LangRuleFinding>,
) {
    let (fn_kind, fn_name) = classify_fn_node(fn_node, source);
    if fn_kind == EnclosingFnKind::Component
        && is_pascal_case_name(&fn_name)
        && parent_scope.enclosing.is_hook_host()
    {
        out.push(LangRuleFinding::new(
            ReactViolation::UnstableNestedComponent,
            node_span(fn_node),
            format!(
                "component `{fn_name}` declared inside another component/Hook remounts its subtree on every render"
            ),
        ));
    }
    let inner_scope = HookWalkScope {
        enclosing: fn_kind,
        in_conditional_or_loop: false,
        after_early_return: false,
    };
    if let Some(params) = fn_node.child_by_field_name("parameters") {
        walk_react_hooks(params, inner_scope, source, out);
    }
    let Some(body) = fn_node.child_by_field_name("body") else {
        return;
    };
    if fn_kind.is_hook_host() && body.kind() == "statement_block" {
        walk_hook_host_stmts(body, inner_scope, source, out);
    } else {
        walk_react_hooks(body, inner_scope, source, out);
    }
}

fn walk_react_hooks(
    node: Node<'_>,
    scope: HookWalkScope,
    source: &str,
    out: &mut Vec<LangRuleFinding>,
) {
    if FN_NODE_KINDS.contains(&node.kind()) {
        walk_fn_boundary(node, scope, source, out);
        return;
    }
    if node.kind() == "call_expression" {
        inspect_hook_call(node, scope, source, out);
    }
    walk_children_with_flow(node, scope, source, out);
}

/// Extracts React 19 Hook rules (`REACT-RULES-OF-HOOKS`) and nested component violations (`GF080`).
pub(super) fn extract_react_hooks(root: Node<'_>, source: &str, out: &mut Vec<LangRuleFinding>) {
    let root_scope = HookWalkScope {
        enclosing: EnclosingFnKind::ModuleRoot,
        in_conditional_or_loop: false,
        after_early_return: false,
    };
    walk_react_hooks(root, root_scope, source, out);
}
