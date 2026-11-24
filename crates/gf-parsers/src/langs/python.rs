//! Safe `Tree-sitter` extractor for Python (`.py`).

use gf_core::{
    CallableDecl, CallableKind, DomainKind, DomainLayoutConfig, ExportDecl, ImportDecl, Language,
    RepoPath, TypeDecl, TypeKind, UastDocument, classify_path,
};
use tree_sitter::Node;

use super::python_guardrails::extract_python_guardrails;
use crate::anti_cheat::populate_cst_fortress;
use crate::complexity::ComplexityRules;
use crate::cst::{
    CallableSpec, assemble_callable, collect_syntax_errors, field_name_string, line_metrics,
    node_span, node_text, parse_with_language, single_param_arity,
};
use crate::fingerprint::collect_referenced_symbols;
use crate::types::{LanguageExtractor, ParseError};

const PY_TUPLE_KINDS: &[&str] = &["tuple_pattern", "tuple"];

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
        walk_python(root, source, false, &mut doc);
        populate_cst_fortress(root, source, &mut doc);
        extract_python_guardrails(root, source, domain, &mut doc);
        Ok(doc)
    }
}

impl LanguageExtractor for PythonExtractor {
    fn extract(&self, path: &RepoPath, source: &str) -> Result<UastDocument, ParseError> {
        let domain = classify_path(path, &DomainLayoutConfig::strict_defaults());
        Self::extract_in_domain(path, source, domain)
    }
}

fn extract_py_import(node: Node<'_>, source: &str) -> ImportDecl {
    let module_source = if node.kind() == "import_from_statement" {
        field_name_string(node, "module_name", node_text(node, source), source)
    } else {
        String::from(node_text(node, source).trim_start_matches("import ").trim())
    };
    ImportDecl {
        module_source,
        symbols: Vec::new(),
        span: node_span(node),
    }
}

fn count_py_params(params_node: Option<Node<'_>>, source: &str, is_method: bool) -> u32 {
    let Some(node) = params_node else {
        return 0;
    };
    let mut cursor = node.walk();
    let params: Vec<Node<'_>> = node
        .named_children(&mut cursor)
        .filter(|c| {
            !matches!(
                c.kind(),
                "comment" | "positional_separator" | "keyword_separator"
            )
        })
        .collect();
    let skip = usize::from(
        is_method
            && params.first().is_some_and(|first| {
                let head = node_text(*first, source)
                    .split(':')
                    .next()
                    .unwrap_or("")
                    .trim();
                head == "self" || head == "cls"
            }),
    );
    params
        .into_iter()
        .skip(skip)
        .map(|p| single_param_arity(p, Some(source), PY_TUPLE_KINDS))
        .fold(0_u32, u32::saturating_add)
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
    let kind = if in_class {
        CallableKind::Method
    } else {
        CallableKind::Function
    };
    assemble_callable(
        CallableSpec {
            name: field_name_string(node, "name", "<def>", source),
            kind,
            decl_node: node,
            body_node: node.child_by_field_name("body"),
            param_count: count_py_params(node.child_by_field_name("parameters"), source, in_class),
        },
        source,
        &PY_COMPLEXITY,
    )
}

fn maybe_push_py_export(node: Node<'_>, name: &str, span: gf_core::Span, doc: &mut UastDocument) {
    let is_module_child = node.parent().is_some_and(|p| p.kind() == "module");
    if is_module_child && !name.starts_with('_') {
        doc.exports.push(ExportDecl {
            symbol: String::from(name),
            is_crate_internal: false,
            span,
        });
    }
}

fn walk_python(node: Node<'_>, source: &str, in_class: bool, doc: &mut UastDocument) {
    let mut next_in_class = in_class;
    match node.kind() {
        "import_statement" | "import_from_statement" => {
            doc.imports.push(extract_py_import(node, source));
        }
        "class_definition" => {
            let ty = extract_py_class(node, source);
            maybe_push_py_export(node, &ty.name, ty.span, doc);
            doc.types.push(ty);
            next_in_class = true;
        }
        "function_definition" => {
            let callable = extract_py_fn(node, source, in_class);
            maybe_push_py_export(node, &callable.name, callable.span, doc);
            doc.callables.push(callable);
        }
        _ => {}
    }

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        walk_python(child, source, next_in_class, doc);
    }
}
