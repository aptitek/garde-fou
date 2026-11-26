//! Pure CST helpers for Kotlin modifiers, parameters, function headers, and `@Test` cases.

use gf_core::{KotlinViolation, LangRuleFinding, Span, UastDocument};
use tree_sitter::Node;

use crate::complexity::ComplexityRules;
use crate::cst::{node_span, node_text};

pub const KT_COMPLEXITY: ComplexityRules = ComplexityRules {
    nesting_branches: &[
        "if_expression",
        "for_statement",
        "while_statement",
        "do_while_statement",
    ],
    flat_branches: &[],
    switch_nodes: &["when_expression"],
    case_arm_nodes: &["when_entry"],
    subtract_first_arm: true,
    bool_expr_nodes: &["conjunction_expression", "disjunction_expression"],
    bool_ops: &["&&", "||"],
    nested_fn_nodes: &["function_declaration", "lambda_literal"],
    else_wrapper_nodes: &[],
};

pub fn find_child_of_kind<'a>(node: Node<'a>, kind: &str) -> Option<Node<'a>> {
    let mut c = node.walk();
    node.named_children(&mut c).find(|ch| ch.kind() == kind)
}

pub fn find_child_text<'a>(node: Node<'_>, kind: &str, source: &'a str) -> Option<&'a str> {
    find_child_of_kind(node, kind).map(|ch| node_text(ch, source).trim())
}

pub fn has_modifier(node: Node<'_>, source: &str, kw: &str) -> bool {
    let mut c = node.walk();
    node.named_children(&mut c).any(|ch| {
        ch.kind() == "modifiers"
            && node_text(ch, source)
                .split_whitespace()
                .any(|tok| tok == kw)
    })
}

pub fn has_annotation(node: Node<'_>, source: &str, ann: &str) -> bool {
    let mut c = node.walk();
    node.named_children(&mut c)
        .any(|ch| ch.kind() == "modifiers" && node_text(ch, source).contains(ann))
}

pub fn is_kt_public(node: Node<'_>, source: &str) -> bool {
    !["private", "internal", "protected"]
        .iter()
        .any(|kw| has_modifier(node, source, kw))
}

pub fn count_kt_params(node: Node<'_>, source: &str, doc: &mut UastDocument) -> (u32, u32) {
    let Some(params) = find_child_of_kind(node, "function_value_parameters") else {
        return (0, 0);
    };
    let (mut total, mut bools) = (0_u32, 0_u32);
    let mut pc = params.walk();
    for p in params.named_children(&mut pc) {
        if p.kind() == "parameter" {
            total += 1;
            let ptxt = node_text(p, source);
            if ptxt.contains(": Boolean") {
                bools += 1;
            }
            if ptxt.contains(": Any") {
                doc.lang_rule_findings.push(LangRuleFinding::new(
                    KotlinViolation::SwallowedExceptionAndAny,
                    node_span(p),
                    String::from("avoid `: Any` / `: Any?` domain parameter type; use a sealed or generic type"),
                ));
            }
        }
    }
    (total, bools)
}

pub fn check_kt_fn_header(span: Span, header_txt: &str, is_suspend: bool, doc: &mut UastDocument) {
    if is_suspend && (header_txt.contains("Flow<") || header_txt.contains(": Flow")) {
        doc.lang_rule_findings.push(LangRuleFinding::new(
            KotlinViolation::CoroutineStructuredSafety,
            span,
            String::from(
                "`suspend fun` returns `Flow`; remove `suspend` because `Flow` is a cold stream",
            ),
        ));
    }
    if header_txt.contains(": Any") {
        doc.lang_rule_findings.push(LangRuleFinding::new(
            KotlinViolation::SwallowedExceptionAndAny,
            span,
            String::from("function returns `: Any`; use a typed domain model"),
        ));
    }
}

pub fn record_kt_test_case(span: Span, body_text: &str, name: &str, doc: &mut UastDocument) {
    const MARKERS: &[&str] = &[
        "assertEquals",
        "assertTrue",
        "assertFalse",
        "assertFailsWith",
        "assertNotNull",
        "shouldBe",
        "assertThrows",
    ];
    doc.test_cases.push(crate::cst::make_test_case(
        String::from(name),
        span,
        body_text,
        MARKERS,
    ));
}

pub fn push_kt_import(imp: Node<'_>, source: &str, doc: &mut UastDocument) {
    if !matches!(imp.kind(), "import_header" | "import") {
        return;
    }
    let raw = node_text(imp, source)
        .trim()
        .trim_start_matches("import")
        .trim()
        .trim_end_matches(';')
        .trim();
    if !raw.is_empty() {
        doc.imports.push(gf_core::ImportDecl {
            module_source: String::from(raw),
            symbols: Vec::new(),
            span: node_span(imp),
            is_type_only: false,
        });
    }
}
