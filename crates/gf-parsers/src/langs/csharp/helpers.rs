//! Pure CST helpers for C# modifiers, parameters, C# 12 primary ctor `CS9124`, and tests.

use std::collections::BTreeSet;

use gf_core::{BypassFinding, CSharpViolation, LangRuleFinding, Span, UastDocument};
use tree_sitter::Node;

use crate::complexity::ComplexityRules;
use crate::cst::{node_span, node_text};

pub const CS_COMPLEXITY: ComplexityRules = ComplexityRules {
    nesting_branches: &[
        "if_statement",
        "for_statement",
        "for_each_statement",
        "while_statement",
        "do_statement",
    ],
    flat_branches: &["catch_clause"],
    switch_nodes: &["switch_statement", "switch_expression"],
    case_arm_nodes: &["switch_section", "switch_expression_arm"],
    subtract_first_arm: true,
    bool_expr_nodes: &["binary_expression"],
    bool_ops: &["&&", "||"],
    nested_fn_nodes: &["local_function_statement", "lambda_expression"],
    else_wrapper_nodes: &[],
};

fn check_cs_bypass_node(n: Node<'_>, source: &str, doc: &mut UastDocument) {
    let txt = node_text(n, source).trim();
    let is_pragma = n.kind() == "preproc_pragma" && txt.contains("warning disable");
    let is_unjustified_suppress = n.kind() == "attribute"
        && txt.contains("SuppressMessage")
        && txt
            .split("Justification")
            .nth(1)
            .and_then(|s| s.split('"').nth(1))
            .is_none_or(|j| j.trim().len() < 10);
    if is_pragma || is_unjustified_suppress {
        doc.bypass_findings.push(BypassFinding {
            directive: String::from(txt),
            span: node_span(n),
        });
    }
}

pub fn inspect_cs_bypasses(root: Node<'_>, source: &str, doc: &mut UastDocument) {
    let mut stack = vec![root];
    while let Some(n) = stack.pop() {
        check_cs_bypass_node(n, source, doc);
        let mut c = n.walk();
        stack.extend(n.children(&mut c));
    }
}

pub fn check_cs_ct_propagation(
    method_span: Span,
    body_text: &str,
    ct_name: &str,
    doc: &mut UastDocument,
) {
    let mut saw_await = false;
    let mut forwarded = false;
    for line in body_text.lines() {
        if let Some((_, after)) = line.split_once("await ") {
            saw_await = true;
            if after.contains(ct_name) {
                forwarded = true;
            }
        }
    }
    if saw_await && !forwarded {
        doc.lang_rule_findings.push(LangRuleFinding::new(
            CSharpViolation::DisposeAndCancellationSafety,
            method_span,
            format!(
                "`CancellationToken {ct_name}` is not forwarded to any awaited call (`MA0040`)"
            ),
        ));
    }
}

pub fn has_cs_modifier(node: Node<'_>, source: &str, kw: &str) -> bool {
    let mut c = node.walk();
    node.named_children(&mut c)
        .any(|ch| ch.kind() == "modifier" && node_text(ch, source).trim() == kw)
}

pub fn has_cs_test_attr(node: Node<'_>, source: &str) -> bool {
    let mut c = node.walk();
    node.named_children(&mut c).any(|ch| {
        ch.kind() == "attribute_list" && {
            let t = node_text(ch, source);
            t.contains("Fact") || t.contains("Theory") || t.contains("Test")
        }
    })
}

fn inspect_cs_param_type(p: Node<'_>, source: &str) -> (bool, Option<String>) {
    let Some(ty) = p.child_by_field_name("type") else {
        return (false, None);
    };
    let ttxt = node_text(ty, source).trim();
    let ct = (ttxt == "CancellationToken")
        .then(|| p.child_by_field_name("name"))
        .flatten()
        .map(|nm| String::from(node_text(nm, source).trim()));
    (ttxt == "bool", ct)
}

pub fn extract_cs_params(node: Node<'_>, source: &str) -> (u32, u32, Option<String>) {
    let Some(plist) = node.child_by_field_name("parameters") else {
        return (0, 0, None);
    };
    let (mut total, mut bools, mut ct_name) = (0_u32, 0_u32, None);
    let mut c = plist.walk();
    for (idx, p) in plist
        .named_children(&mut c)
        .filter(|n| n.kind() == "parameter")
        .enumerate()
    {
        let is_ext_this = idx == 0 && node_text(p, source).trim_start().starts_with("this ");
        total += u32::from(!is_ext_this);
        let (is_bool, ct) = inspect_cs_param_type(p, source);
        bools += u32::from(is_bool);
        if ct.is_some() {
            ct_name = ct;
        }
    }
    (total, bools, ct_name)
}

fn contains_ident(haystack: &str, ident: &str) -> bool {
    haystack
        .split(|c: char| !c.is_ascii_alphanumeric() && c != '_')
        .any(|tok| tok == ident)
}

fn check_ctor_param_mutation(p: &str, mtxt: &str, span: Span, doc: &mut UastDocument) {
    if mtxt.contains(&format!("{p} ="))
        || mtxt.contains(&format!("{p}++"))
        || mtxt.contains(&format!("{p}--"))
    {
        doc.lang_rule_findings.push(LangRuleFinding::new(
            CSharpViolation::PrimaryCtorDoubleCapture,
            span,
            format!("primary constructor parameter `{p}` mutated inside member body"),
        ));
    }
}

fn scan_ctor_member_uses<'a>(
    body: Node<'_>,
    ctor_params: &[&'a str],
    source: &str,
    doc: &mut UastDocument,
) -> (BTreeSet<&'a str>, Vec<(&'a str, Span)>) {
    let mut captured_in_init = BTreeSet::new();
    let mut in_method = Vec::new();
    let mut bc = body.walk();
    for member in body.named_children(&mut bc) {
        let mtxt = node_text(member, source);
        let is_init = matches!(member.kind(), "field_declaration" | "property_declaration")
            && mtxt.contains('=');
        let is_callable = matches!(member.kind(), "method_declaration" | "accessor_declaration");
        if !is_init && !is_callable {
            continue;
        }
        let span = node_span(member);
        for &p in ctor_params.iter().filter(|&&p| contains_ident(mtxt, p)) {
            if is_init {
                captured_in_init.insert(p);
            } else {
                in_method.push((p, span));
                check_ctor_param_mutation(p, mtxt, span, doc);
            }
        }
    }
    (captured_in_init, in_method)
}

pub fn check_primary_ctor_capture(type_node: Node<'_>, source: &str, doc: &mut UastDocument) {
    if type_node.kind() == "record_declaration" {
        return;
    }
    let plist = type_node.child_by_field_name("parameters").or_else(|| {
        let mut c = type_node.walk();
        type_node
            .named_children(&mut c)
            .find(|ch| ch.kind() == "parameter_list")
    });
    let Some(plist) = plist else {
        return;
    };
    let Some(body) = type_node.child_by_field_name("body") else {
        return;
    };
    let mut pc = plist.walk();
    let ctor_params: Vec<&str> = plist
        .named_children(&mut pc)
        .filter(|p| p.kind() == "parameter")
        .filter_map(|p| p.child_by_field_name("name"))
        .map(|nm| node_text(nm, source).trim())
        .collect();
    if ctor_params.is_empty() {
        return;
    }
    let (captured_in_init, in_method) = scan_ctor_member_uses(body, &ctor_params, source, doc);
    for (p, span) in in_method {
        if captured_in_init.contains(p) {
            doc.lang_rule_findings.push(LangRuleFinding::new(
                CSharpViolation::PrimaryCtorDoubleCapture,
                span,
                format!("primary constructor parameter `{p}` double-captured in both initializer and method body (`CS9124`)"),
            ));
        }
    }
}

pub fn record_cs_test_case(span: Span, body_text: &str, name: String, doc: &mut UastDocument) {
    const MARKERS: &[&str] = &["Assert.", ".Should(", "CollectionAssert.", "StringAssert."];
    doc.test_cases
        .push(crate::cst::make_test_case(name, span, body_text, MARKERS));
}
