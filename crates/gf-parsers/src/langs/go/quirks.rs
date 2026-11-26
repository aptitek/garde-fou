//! Pure CST quirk analyzers for Go (`SA4023`, `SA5001`, `defer` in loops, `//nolint`, `context.Context`).

use std::collections::BTreeSet;

use gf_core::{BypassFinding, GoViolation, LangRuleFinding, UastDocument};
use tree_sitter::Node;

pub(crate) use super::quirks_calls::{
    GO_FLAG_HAS_CTX, GO_FLAG_IFACE_RET, GO_FLAG_IS_CMD, GO_FLAG_IS_TEST, GoFnCtx,
};
use super::quirks_calls::{check_call_node, check_errcheck_stmt};
use crate::cst::{node_span, node_text};

fn push_go(doc: &mut UastDocument, v: GoViolation, node: Node<'_>, msg: String) {
    doc.lang_rule_findings
        .push(LangRuleFinding::new(v, node_span(node), msg));
}

fn check_go_comment_bypass(node: Node<'_>, source: &str, doc: &mut UastDocument) {
    let txt = node_text(node, source).trim();
    let invalid_nolint = txt.strip_prefix("//nolint").is_some_and(|rest| {
        !(rest.starts_with(':')
            && rest
                .split_once("//")
                .is_some_and(|(_, reason)| reason.trim().len() >= 10))
    });
    let invalid_lint = txt
        .strip_prefix("//lint:ignore")
        .is_some_and(|rest| rest.split_whitespace().count() < 3);
    if invalid_nolint || invalid_lint {
        doc.bypass_findings.push(BypassFinding {
            directive: String::from(txt),
            span: node_span(node),
        });
    }
}

pub(crate) fn inspect_go_comments(root: Node<'_>, source: &str, doc: &mut UastDocument) {
    let mut stack = vec![root];
    while let Some(node) = stack.pop() {
        if node.kind() == "comment" {
            check_go_comment_bypass(node, source, doc);
        }
        let mut cursor = node.walk();
        stack.extend(node.children(&mut cursor));
    }
}

pub(crate) fn check_go_context_params(
    params_node: Option<Node<'_>>,
    source: &str,
    doc: &mut UastDocument,
) -> bool {
    let Some(params) = params_node else {
        return false;
    };
    let mut cursor = params.walk();
    let decls: Vec<Node<'_>> = params
        .named_children(&mut cursor)
        .filter(|n| n.kind() == "parameter_declaration")
        .collect();
    let mut has_ctx = false;
    for (idx, decl) in decls.iter().enumerate() {
        let is_ctx = decl
            .child_by_field_name("type")
            .is_some_and(|t| node_text(t, source).trim() == "context.Context");
        if is_ctx {
            has_ctx = true;
            if idx > 0 {
                push_go(
                    doc,
                    GoViolation::ContextMisuse,
                    *decl,
                    String::from("`context.Context` parameter must be the first parameter"),
                );
            }
        }
    }
    has_ctx
}

fn collect_typed_nil_pointer_vars(body: Node<'_>, source: &str) -> BTreeSet<String> {
    let mut typed_ptrs = BTreeSet::new();
    let mut stack = vec![body];
    while let Some(n) = stack.pop() {
        if n.kind() == "var_spec"
            && let Some(ty) = n.child_by_field_name("type")
            && node_text(ty, source).trim().starts_with('*')
            && let Some(name_n) = n.child_by_field_name("name")
        {
            let val_ok = n
                .child_by_field_name("value")
                .is_none_or(|v| node_text(v, source).trim() == "nil");
            if val_ok {
                typed_ptrs.insert(String::from(node_text(name_n, source).trim()));
            }
        }
        let mut cursor = n.walk();
        for ch in n.named_children(&mut cursor) {
            stack.push(ch);
        }
    }
    typed_ptrs
}

fn check_sa5001_defer_before_err(block: Node<'_>, source: &str, doc: &mut UastDocument) {
    let mut cursor = block.walk();
    let stmts: Vec<Node<'_>> = block
        .named_children(&mut cursor)
        .filter(|n| n.kind() != "comment")
        .collect();
    for pair in stmts.windows(2) {
        let (first, second) = (pair[0], pair[1]);
        if !matches!(
            first.kind(),
            "short_var_declaration" | "assignment_statement"
        ) || second.kind() != "defer_statement"
        {
            continue;
        }
        let Some(lhs) = first.child_by_field_name("left") else {
            continue;
        };
        let lhs_parts: Vec<&str> = node_text(lhs, source).split(',').map(str::trim).collect();
        if lhs_parts.len() < 2 || lhs_parts.last().copied() == Some("_") {
            continue;
        }
        let res_var = lhs_parts[0];
        let defer_txt = node_text(second, source);
        if defer_txt.contains(&format!("{res_var}.")) && defer_txt.contains(".Close()") {
            push_go(
                doc,
                GoViolation::DeferBeforeErrCheck,
                second,
                format!(
                    "`defer` on `{res_var}` registered before checking `if err != nil` (`SA5001`)"
                ),
            );
        }
    }
}

pub(crate) fn inspect_go_body(
    body: Node<'_>,
    source: &str,
    ctx: GoFnCtx<'_>,
    doc: &mut UastDocument,
) {
    let typed_nil_vars = if ctx.returns_iface() {
        collect_typed_nil_pointer_vars(body, source)
    } else {
        BTreeSet::new()
    };
    let mut walk = GoBodyWalk {
        source,
        ctx,
        typed_nil_vars: &typed_nil_vars,
        doc,
    };
    walk.visit(body, false);
}

struct GoBodyWalk<'a> {
    source: &'a str,
    ctx: GoFnCtx<'a>,
    typed_nil_vars: &'a BTreeSet<String>,
    doc: &'a mut UastDocument,
}

impl GoBodyWalk<'_> {
    fn visit(&mut self, node: Node<'_>, in_loop: bool) {
        match node.kind() {
            "block" | "statement_list" => {
                check_sa5001_defer_before_err(node, self.source, self.doc);
            }
            "defer_statement" if in_loop => push_go(
                self.doc,
                GoViolation::DeferInLoop,
                node,
                String::from("`defer` inside a `for` loop delays cleanup until function exit"),
            ),
            "return_statement" if !self.typed_nil_vars.is_empty() => {
                let ret_txt = node_text(node, self.source)
                    .trim()
                    .trim_start_matches("return")
                    .trim();
                for item in ret_txt.split(',').map(str::trim) {
                    if self.typed_nil_vars.contains(item) {
                        push_go(
                            self.doc,
                            GoViolation::TypedNilInterface,
                            node,
                            format!(
                                "returning typed-nil pointer `{item}` as interface/error (`SA4023`); return untyped `nil`"
                            ),
                        );
                    }
                }
            }
            "call_expression" => check_call_node(node, self.source, self.ctx, self.doc),
            "short_var_declaration" | "assignment_statement" | "expression_statement" => {
                check_errcheck_stmt(node, self.source, self.doc);
            }
            _ => {}
        }
        let child_loop = in_loop || node.kind() == "for_statement";
        let mut cursor = node.walk();
        for ch in node.named_children(&mut cursor) {
            self.visit(ch, child_loop);
        }
    }
}
