//! Exact React 19 Hook identifier (`use` / `use[A-Z0-9]*`) and React Component
//! (`PascalCase` returning JSX/`null` or `memo`/`forwardRef`) classifiers.

use tree_sitter::Node;

use crate::cst::{field_name_string, node_text};

pub(super) const FN_NODE_KINDS: &[&str] = &[
    "function_declaration",
    "arrow_function",
    "function_expression",
    "method_definition",
];

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum EnclosingFnKind {
    ModuleRoot,
    Component,
    CustomHook,
    PlainOrMethod,
}

impl EnclosingFnKind {
    pub(super) const fn is_hook_host(self) -> bool {
        matches!(self, Self::Component | Self::CustomHook)
    }
}

/// Exact React Hook name predicate (`use` or `use[A-Z0-9].*`).
#[must_use]
pub fn is_hook_identifier(name: &str) -> bool {
    if name == "use" {
        return true;
    }
    name.strip_prefix("use")
        .and_then(|rest| rest.chars().next())
        .is_some_and(|ch| ch.is_ascii_uppercase() || ch.is_ascii_digit())
}

pub(super) fn is_pascal_case_name(name: &str) -> bool {
    name.chars().next().is_some_and(|c| c.is_ascii_uppercase())
}

pub(super) fn classify_hook_call<'a>(
    call_node: Node<'_>,
    source: &'a str,
) -> Option<(&'a str, bool)> {
    let func = call_node.child_by_field_name("function")?;
    match func.kind() {
        "identifier" => {
            let name = node_text(func, source).trim();
            is_hook_identifier(name).then_some((name, name == "use"))
        }
        "member_expression" => {
            let obj = func.child_by_field_name("object")?;
            let prop = func.child_by_field_name("property")?;
            if obj.kind() != "identifier" || prop.kind() != "property_identifier" {
                return None;
            }
            let obj_name = node_text(obj, source).trim();
            let prop_name = node_text(prop, source).trim();
            if prop_name == "use" {
                (obj_name == "React").then_some(("React.use", true))
            } else {
                is_hook_identifier(prop_name).then(|| (node_text(func, source).trim(), false))
            }
        }
        _ => None,
    }
}

fn contains_jsx_in_own_fn(node: Node<'_>) -> bool {
    if matches!(
        node.kind(),
        "jsx_element" | "jsx_self_closing_element" | "jsx_fragment"
    ) {
        return true;
    }
    let mut cursor = node.walk();
    node.children(&mut cursor)
        .filter(|c| !FN_NODE_KINDS.contains(&c.kind()))
        .any(contains_jsx_in_own_fn)
}

fn has_own_null_return(node: Node<'_>) -> bool {
    if node.kind() == "return_statement" {
        return node.named_child(0).is_some_and(|v| v.kind() == "null");
    }
    let mut cursor = node.walk();
    node.children(&mut cursor)
        .filter(|c| !FN_NODE_KINDS.contains(&c.kind()))
        .any(has_own_null_return)
}

fn returns_jsx_or_null(fn_node: Node<'_>) -> bool {
    let Some(body) = fn_node.child_by_field_name("body") else {
        return false;
    };
    if body.kind() != "statement_block" {
        return body.kind() == "null" || contains_jsx_in_own_fn(body);
    }
    contains_jsx_in_own_fn(body) || has_own_null_return(body)
}

fn enclosing_hoc_or_hook_tester<'a>(
    fn_node: Node<'a>,
    source: &'a str,
) -> Option<(Node<'a>, &'a str)> {
    let args = fn_node.parent().filter(|p| p.kind() == "arguments")?;
    let call = args.parent().filter(|p| p.kind() == "call_expression")?;
    let callee = call.child_by_field_name("function")?;
    let leaf = node_text(callee, source).trim().rsplit('.').next()?;
    Some((call, leaf))
}

fn infer_callable_name(fn_node: Node<'_>, source: &str) -> String {
    let own = field_name_string(fn_node, "name", "", source);
    if !own.is_empty() {
        return own;
    }
    let holder = enclosing_hoc_or_hook_tester(fn_node, source)
        .filter(|(_, leaf)| matches!(*leaf, "memo" | "forwardRef"))
        .map_or(fn_node, |(call, _)| call);
    if let Some(parent) = holder.parent() {
        if parent.kind() == "variable_declarator" {
            return field_name_string(parent, "name", "", source);
        }
        if parent.kind() == "assignment_expression" {
            return field_name_string(parent, "left", "", source);
        }
    }
    String::new()
}

pub(super) fn classify_fn_node(fn_node: Node<'_>, source: &str) -> (EnclosingFnKind, String) {
    if fn_node.kind() == "method_definition" {
        return (EnclosingFnKind::PlainOrMethod, String::new());
    }
    if enclosing_hoc_or_hook_tester(fn_node, source).is_some_and(|(_, leaf)| leaf == "renderHook") {
        return (EnclosingFnKind::CustomHook, String::from("renderHook"));
    }
    let wrapped_hoc = enclosing_hoc_or_hook_tester(fn_node, source)
        .is_some_and(|(_, leaf)| matches!(leaf, "memo" | "forwardRef"));
    let name = infer_callable_name(fn_node, source);
    if is_hook_identifier(&name) {
        (EnclosingFnKind::CustomHook, name)
    } else if wrapped_hoc || (is_pascal_case_name(&name) && returns_jsx_or_null(fn_node)) {
        (EnclosingFnKind::Component, name)
    } else {
        (EnclosingFnKind::PlainOrMethod, name)
    }
}

pub(super) fn contains_own_early_exit(stmt: Node<'_>) -> bool {
    if matches!(stmt.kind(), "return_statement" | "throw_statement") {
        return true;
    }
    let mut cursor = stmt.walk();
    stmt.children(&mut cursor)
        .filter(|c| !FN_NODE_KINDS.contains(&c.kind()))
        .any(contains_own_early_exit)
}
