//! Safe `Tree-sitter` extractor for Python (`.py`).

use gf_core::{
    CallableDecl, CallableKind, ExportDecl, ImportDecl, Language, RepoPath, TypeDecl, TypeKind,
    UastDocument,
};
use tree_sitter::Node;

use crate::complexity::{ComplexityRules, compute_metrics};
use crate::cst::{
    collect_syntax_errors, line_metrics, node_span, node_text, parse_with_language,
    span_effective_lines,
};
use crate::{LanguageExtractor, ParseError};

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

impl LanguageExtractor for PythonExtractor {
    fn extract(&self, path: &RepoPath, source: &str) -> Result<UastDocument, ParseError> {
        let ts_lang = tree_sitter_python::LANGUAGE.into();
        let tree = parse_with_language(&ts_lang, source)?;
        let (total_lines, effective_lines) = line_metrics(source);
        let mut doc =
            UastDocument::new(path.clone(), Language::Python, total_lines, effective_lines)?;
        let root = tree.root_node();
        doc.syntax_errors = collect_syntax_errors(root);
        walk_python(root, source, false, &mut doc);
        Ok(doc)
    }
}

fn extract_py_import(node: Node<'_>, source: &str) -> ImportDecl {
    let module_source = if node.kind() == "import_from_statement" {
        node.child_by_field_name("module_name").map_or_else(
            || String::from(node_text(node, source)),
            |m| String::from(node_text(m, source)),
        )
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
    let names: Vec<&str> = node
        .named_children(&mut cursor)
        .filter(|c| c.kind() != "comment")
        .map(|c| node_text(c, source))
        .collect();
    let skip = usize::from(
        is_method
            && names
                .first()
                .is_some_and(|first| *first == "self" || *first == "cls"),
    );
    u32::try_from(names.len().saturating_sub(skip)).unwrap_or(u32::MAX)
}

fn extract_py_class(node: Node<'_>, source: &str) -> TypeDecl {
    let name = node.child_by_field_name("name").map_or_else(
        || String::from("<class>"),
        |n| String::from(node_text(n, source)),
    );
    let mut field_count = 0_u32;
    let mut method_count = 0_u32;

    if let Some(body) = node.child_by_field_name("body") {
        let mut cursor = body.walk();
        for stmt in body.named_children(&mut cursor) {
            match stmt.kind() {
                "function_definition" => method_count = method_count.saturating_add(1),
                "decorated_definition"
                    if stmt
                        .child_by_field_name("definition")
                        .is_some_and(|d| d.kind() == "function_definition") =>
                {
                    method_count = method_count.saturating_add(1);
                }
                "expression_statement"
                    if stmt
                        .named_child(0)
                        .is_some_and(|c| c.kind() == "assignment") =>
                {
                    field_count = field_count.saturating_add(1);
                }
                _ => {}
            }
        }
    }

    TypeDecl {
        name,
        kind: TypeKind::Class,
        span: node_span(node),
        field_count,
        method_count,
    }
}

fn extract_py_fn(node: Node<'_>, source: &str, in_class: bool) -> CallableDecl {
    let name = node.child_by_field_name("name").map_or_else(
        || String::from("<def>"),
        |n| String::from(node_text(n, source)),
    );
    let span = node_span(node);
    let param_count = count_py_params(node.child_by_field_name("parameters"), source, in_class);
    let metrics = node.child_by_field_name("body").map_or(
        crate::complexity::ComputedMetrics {
            cyclomatic: 1,
            cognitive: 0,
            max_nesting: 0,
        },
        |body| compute_metrics(body, source, &PY_COMPLEXITY),
    );

    CallableDecl {
        name,
        kind: if in_class {
            CallableKind::Method
        } else {
            CallableKind::Function
        },
        span,
        effective_lines: span_effective_lines(span, source),
        param_count,
        cyclomatic_complexity: metrics.cyclomatic,
        cognitive_complexity: metrics.cognitive,
        max_nesting_depth: metrics.max_nesting,
    }
}

fn maybe_push_py_export(node: Node<'_>, name: &str, span: gf_core::Span, doc: &mut UastDocument) {
    let is_module_child = node.parent().is_some_and(|p| p.kind() == "module");
    if is_module_child && !name.starts_with('_') {
        doc.exports.push(ExportDecl {
            symbol: String::from(name),
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
