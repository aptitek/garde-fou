//! Safe `Tree-sitter` extractor for Python (`.py`).

use gf_core::{
    CallableDecl, CallableKind, DomainKind, DomainLayoutConfig, Language, RepoPath, TypeDecl,
    TypeKind, UastDocument, classify_path,
};
use tree_sitter::Node;

use super::python_exports::{extract_py_exports, extract_py_imports};
use super::python_guardrails::extract_python_guardrails;
use super::python_pep723::check_pep723_metadata;
use super::python_typing::count_callable_py_params;
use super::sql::embedded_sql::extract_python_embedded_sql;
use crate::anti_cheat::populate_cst_fortress;
use crate::complexity::ComplexityRules;
use crate::cst::{
    CallableSpec, assemble_callable, collect_syntax_errors, field_name_string, line_metrics,
    node_span, node_text, parse_with_language,
};
use crate::fingerprint::collect_referenced_symbols;
use crate::types::{LanguageExtractor, ParseError};

const PY_COMPLEXITY: ComplexityRules = ComplexityRules {
    nesting_branches: &[
        "if_statement",
        "for_statement",
        "while_statement",
        "except_clause",
        "conditional_expression",
        "if_clause",
    ],
    flat_branches: &["elif_clause"],
    switch_nodes: &["match_statement"],
    case_arm_nodes: &["case_clause"],
    subtract_first_arm: true,
    bool_expr_nodes: &["boolean_operator"],
    bool_ops: &["and", "or"],
    nested_fn_nodes: &["function_definition"],
    else_wrapper_nodes: &[],
};

/// `Tree-sitter` AST extractor for Python source files.
#[derive(Clone, Copy, Debug, Default)]
pub struct PythonExtractor;

impl PythonExtractor {
    /// Extracts a Python [`UastDocument`] with explicit [`DomainKind`] classification (`GF080`).
    ///
    /// # Errors
    ///
    /// Returns [`ParseError`] if Tree-sitter Python grammar initialization or span creation fails.
    pub fn extract_in_domain(
        path: &RepoPath,
        source: &str,
        domain: DomainKind,
    ) -> Result<UastDocument, ParseError> {
        let ts_lang = tree_sitter_python::LANGUAGE.into();
        let tree = parse_with_language(&ts_lang, source)?;
        let (total_lines, effective_lines) = line_metrics(source);
        let mut doc =
            UastDocument::new(path.clone(), Language::Python, total_lines, effective_lines)?;
        let root = tree.root_node();
        doc.syntax_errors = collect_syntax_errors(root);
        doc.referenced_symbols = collect_referenced_symbols(root, source);
        doc.exports = extract_py_exports(root, path, source);
        walk_python(root, source, false, &mut doc);
        populate_cst_fortress(root, source, &mut doc);
        extract_python_guardrails(root, source, domain, &mut doc);
        check_pep723_metadata(source, &mut doc.lang_rule_findings);
        if matches!(
            domain,
            DomainKind::Code | DomainKind::Scripts | DomainKind::Contenus
        ) {
            extract_python_embedded_sql(root, source, &mut doc.lang_rule_findings);
        }
        doc.lang_rule_findings
            .sort_by(|a, b| a.span.cmp(&b.span).then_with(|| a.code().cmp(b.code())));
        Ok(doc)
    }
}

impl LanguageExtractor for PythonExtractor {
    fn extract(&self, path: &RepoPath, source: &str) -> Result<UastDocument, ParseError> {
        let domain = classify_path(path, &DomainLayoutConfig::strict_defaults());
        Self::extract_in_domain(path, source, domain)
    }
}

fn is_py_bool_field(assign_node: Node<'_>, source: &str) -> bool {
    assign_node
        .child_by_field_name("type")
        .is_some_and(|ty| node_text(ty, source).trim().trim_start_matches(':').trim() == "bool")
}

fn tally_py_class_stmt(stmt: Node<'_>, source: &str, ty: &mut TypeDecl) {
    match stmt.kind() {
        "function_definition" => ty.method_count = ty.method_count.saturating_add(1),
        "decorated_definition"
            if stmt
                .child_by_field_name("definition")
                .is_some_and(|d| d.kind() == "function_definition") =>
        {
            ty.method_count = ty.method_count.saturating_add(1);
        }
        "expression_statement" => {
            if let Some(assign) = stmt.named_child(0).filter(|c| c.kind() == "assignment") {
                ty.field_count = ty.field_count.saturating_add(1);
                if is_py_bool_field(assign, source) {
                    ty.bool_field_count = ty.bool_field_count.saturating_add(1);
                }
            }
        }
        _ => {}
    }
}

fn extract_py_class(node: Node<'_>, source: &str) -> TypeDecl {
    let mut ty = TypeDecl {
        name: field_name_string(node, "name", "<class>", source),
        kind: TypeKind::Class,
        span: node_span(node),
        field_count: 0,
        bool_field_count: 0,
        method_count: 0,
    };
    if let Some(body) = node.child_by_field_name("body") {
        let mut cursor = body.walk();
        for stmt in body.named_children(&mut cursor) {
            tally_py_class_stmt(stmt, source, &mut ty);
        }
    }
    ty
}

fn extract_py_fn(node: Node<'_>, source: &str, in_class: bool) -> CallableDecl {
    let name = field_name_string(node, "name", "<def>", source);
    let param_count = count_callable_py_params(node, &name, in_class, source);
    let kind = if in_class {
        CallableKind::Method
    } else {
        CallableKind::Function
    };
    assemble_callable(
        CallableSpec {
            name,
            kind,
            decl_node: node,
            body_node: node.child_by_field_name("body"),
            param_count,
        },
        source,
        &PY_COMPLEXITY,
    )
}

fn walk_python(node: Node<'_>, source: &str, in_class: bool, doc: &mut UastDocument) {
    let mut next_in_class = in_class;
    match node.kind() {
        "import_statement" | "import_from_statement" => {
            extract_py_imports(node, source, &mut doc.imports);
        }
        "class_definition" => {
            doc.types.push(extract_py_class(node, source));
            next_in_class = true;
        }
        "function_definition" => {
            doc.callables.push(extract_py_fn(node, source, in_class));
            next_in_class = false;
        }
        _ => {}
    }

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        walk_python(child, source, next_in_class, doc);
    }
}
