//! Tier 1 TypeScript & async safety AST rules (`TS-NO-EXPLICIT-ANY`, `TS-NO-DOUBLE-ASSERTION`,
//! `TS-NO-NON-NULL-OPTIONAL-CHAIN`, `TS-NO-ASYNC-PROMISE-EXECUTOR`, `TS-NO-ASYNC-EFFECT`,
//! `TS-NO-CONSOLE-DEBUG`) and `console.*` effect sightings (`GF078`).

use gf_core::{
    AsyncHazardKind, EffectSighting, EffectSightingKind, LangRuleFinding, TsDebugKind,
    TypescriptViolation, UastDocument,
};
use tree_sitter::Node;

use super::routing::is_in_ambient_context;
use crate::cst::{node_span, node_text};

fn classify_console_callee(callee: &str) -> (bool, bool) {
    let Some(method) = callee.strip_prefix("console.") else {
        return (false, false);
    };
    let is_effect = matches!(
        method,
        "log" | "error" | "warn" | "info" | "debug" | "trace"
    );
    let is_debug = matches!(method, "log" | "debug" | "info" | "trace");
    (is_effect, is_debug)
}

fn is_react_effect_hook(callee: &str) -> bool {
    matches!(
        callee.strip_prefix("React.").unwrap_or(callee),
        "useEffect" | "useLayoutEffect" | "useInsertionEffect"
    )
}

fn peel_parens(mut node: Node<'_>) -> Node<'_> {
    while node.kind() == "parenthesized_expression" {
        let Some(inner) = node.named_child(0) else {
            break;
        };
        node = inner;
    }
    node
}

fn has_active_optional_chain(node: Node<'_>) -> bool {
    if !matches!(
        node.kind(),
        "member_expression" | "subscript_expression" | "call_expression"
    ) {
        return false;
    }
    let mut cursor = node.walk();
    if node
        .children(&mut cursor)
        .any(|ch| matches!(ch.kind(), "optional_chain" | "?."))
    {
        return true;
    }
    let receiver = if node.kind() == "call_expression" {
        node.child_by_field_name("function")
    } else {
        node.child_by_field_name("object")
    };
    receiver.is_some_and(has_active_optional_chain)
}

fn inspect_non_null_expr(node: Node<'_>, source: &str, out: &mut Vec<LangRuleFinding>) {
    let Some(mut raw_inner) = node.named_child(0) else {
        return;
    };
    while raw_inner.kind() == "binary_expression" {
        let Some(rhs) = raw_inner.child_by_field_name("right") else {
            break;
        };
        raw_inner = rhs;
    }
    let inner = peel_parens(raw_inner);
    if inner.kind() == "non_null_expression" {
        out.push(LangRuleFinding::new(
            TypescriptViolation::NonNullOptionalChain,
            node_span(node),
            format!(
                "redundant double non-null assertion `{}` masks nullable flow",
                node_text(node, source).trim()
            ),
        ));
    } else if has_active_optional_chain(inner) {
        out.push(LangRuleFinding::new(
            TypescriptViolation::NonNullOptionalChain,
            node_span(node),
            format!(
                "non-null assertion on optional chain `{}` throws `TypeError` when short-circuited to `undefined`",
                node_text(node, source).trim()
            ),
        ));
    }
}

fn inspect_as_expr(node: Node<'_>, source: &str, out: &mut Vec<LangRuleFinding>) {
    let Some(lhs) = node.named_child(0).map(peel_parens) else {
        return;
    };
    if lhs.kind() != "as_expression" {
        return;
    }
    let Some(mid_type) = lhs.named_child(1) else {
        return;
    };
    let mid_text = node_text(mid_type, source).trim();
    if matches!(mid_text, "unknown" | "any") {
        out.push(LangRuleFinding::new(
            TypescriptViolation::DoubleOrAngleAssertion,
            node_span(node),
            format!(
                "double type assertion `as {mid_text} as ...` completely bypasses TypeScript assignability checks"
            ),
        ));
    }
}

fn is_async_callable_arg(arg: Node<'_>) -> bool {
    let target = peel_parens(arg);
    if !matches!(target.kind(), "arrow_function" | "function_expression") {
        return false;
    }
    let mut cursor = target.walk();
    target.children(&mut cursor).any(|ch| ch.kind() == "async")
}

fn compact_expr_text(node: Node<'_>, source: &str) -> String {
    node_text(node, source)
        .chars()
        .filter(|c| !c.is_whitespace())
        .collect()
}

fn inspect_new_promise(node: Node<'_>, source: &str, out: &mut Vec<LangRuleFinding>) {
    let Some(ctor) = node.child_by_field_name("constructor") else {
        return;
    };
    let ctor_name = compact_expr_text(ctor, source);
    if !matches!(
        ctor_name.as_str(),
        "Promise" | "globalThis.Promise" | "window.Promise"
    ) {
        return;
    }
    if node
        .child_by_field_name("arguments")
        .and_then(|a| a.named_child(0))
        .is_some_and(is_async_callable_arg)
    {
        out.push(LangRuleFinding::new(
            TypescriptViolation::AsyncHazard(AsyncHazardKind::AsyncPromiseExecutor),
            node_span(node),
            "`new Promise(async (...) => ...)` swallows thrown errors and rejected promises",
        ));
    }
}

fn inspect_call_safety(node: Node<'_>, source: &str, in_code_domain: bool, doc: &mut UastDocument) {
    let Some(func) = node.child_by_field_name("function") else {
        return;
    };
    let callee = compact_expr_text(func, source);
    let span = node_span(node);

    let (is_effect_console, is_debug_console) = classify_console_callee(&callee);
    if is_effect_console {
        doc.effect_sightings.push(EffectSighting {
            kind: EffectSightingKind::ConsoleOutput,
            detail: callee.clone(),
            span,
        });
    }
    if in_code_domain && is_debug_console {
        doc.lang_rule_findings.push(LangRuleFinding::new(
            TypescriptViolation::ConsoleOrDebugger(TsDebugKind::ConsoleCall),
            span,
            format!("`{callee}(...)` debug output is forbidden in `Code` modules"),
        ));
    }
    if is_react_effect_hook(&callee)
        && node
            .child_by_field_name("arguments")
            .and_then(|a| a.named_child(0))
            .is_some_and(is_async_callable_arg)
    {
        doc.lang_rule_findings.push(LangRuleFinding::new(
            TypescriptViolation::AsyncHazard(AsyncHazardKind::AsyncEffectCallback),
            span,
            format!("`{callee}(async () => ...)` returns a `Promise` instead of a synchronous React cleanup function"),
        ));
    }
}

#[derive(Clone, Copy)]
pub(super) struct TsSafetyCtx<'a> {
    pub source: &'a str,
    pub in_code: bool,
    pub is_dts: bool,
}

fn inspect_predefined_any(node: Node<'_>, ctx: TsSafetyCtx<'_>, out: &mut Vec<LangRuleFinding>) {
    if ctx.in_code
        && !is_in_ambient_context(node, ctx.is_dts)
        && node_text(node, ctx.source).trim() == "any"
    {
        out.push(LangRuleFinding::new(
            TypescriptViolation::ExplicitAny,
            node_span(node),
            "explicit `any` type disables TypeScript type safety in `Code`",
        ));
    }
}

const MSG_ANGLE: &str =
    "legacy angle-bracket `<T>expr` type assertion is ambiguous with JSX and forbidden";
const MSG_DBG: &str = "leftover `debugger;` statement in `Code` module";

fn visit_ts_safety_node(node: Node<'_>, ctx: TsSafetyCtx<'_>, doc: &mut UastDocument) {
    let out = &mut doc.lang_rule_findings;
    match node.kind() {
        "predefined_type" => inspect_predefined_any(node, ctx, out),
        "type_assertion" => out.push(LangRuleFinding::new(
            TypescriptViolation::DoubleOrAngleAssertion,
            node_span(node),
            MSG_ANGLE,
        )),
        "as_expression" => inspect_as_expr(node, ctx.source, out),
        "non_null_expression" => inspect_non_null_expr(node, ctx.source, out),
        "new_expression" => inspect_new_promise(node, ctx.source, out),
        "call_expression" => inspect_call_safety(node, ctx.source, ctx.in_code, doc),
        "debugger_statement" if ctx.in_code => out.push(LangRuleFinding::new(
            TypescriptViolation::ConsoleOrDebugger(TsDebugKind::DebuggerStmt),
            node_span(node),
            MSG_DBG,
        )),
        _ => {}
    }
}

/// Extracts Tier 1 TypeScript type-safety, async, and console/debugger findings (`GF080`).
pub(super) fn extract_ts_safety(node: Node<'_>, ctx: TsSafetyCtx<'_>, doc: &mut UastDocument) {
    visit_ts_safety_node(node, ctx, doc);
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        extract_ts_safety(child, ctx, doc);
    }
}
