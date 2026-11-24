//! Safe `Tree-sitter` extractor for TypeScript, TSX, JavaScript, and JSX.

mod module_graph;

use gf_core::{CallableDecl, CallableKind, Language, RepoPath, TypeDecl, TypeKind, UastDocument};
use tree_sitter::Node;

use crate::anti_cheat::populate_cst_fortress;
use crate::complexity::ComplexityRules;
use crate::cst::{
    CallableSpec, assemble_callable, collect_syntax_errors, count_named_kinds, field_name_string,
    line_metrics, node_span, node_text, parse_with_language, single_param_arity,
};
use crate::fingerprint::collect_referenced_symbols;
use crate::langs::{
    typescript_jsx::extract_jsx_ui_and_a11y, typescript_ui::extract_ts_style_and_i18n,
};
use crate::types::{LanguageExtractor, ParseError};
use module_graph::{extract_ts_import, record_ts_call_expr, record_ts_export_stmt};

const TS_TUPLE_KINDS: &[&str] = &["tuple_type", "array_pattern"];

const TS_COMPLEXITY: ComplexityRules = ComplexityRules {
    nesting_branches: &[
        "if_statement",
        "for_statement",
        "for_in_statement",
        "while_statement",
        "do_statement",
        "catch_clause",
        "ternary_expression",
    ],
    flat_branches: &[],
    switch_nodes: &["switch_statement"],
    case_arm_nodes: &["switch_case"],
    subtract_first_arm: false,
    bool_expr_nodes: &["binary_expression"],
    bool_ops: &["&&", "||", "??"],
    nested_fn_nodes: &["function_declaration", "method_definition"],
    else_wrapper_nodes: &["else_clause"],
};

/// Dialect selector choosing between pure TypeScript/JS and JSX-enabled TSX/JSX grammars.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TsDialect {
    /// Standard TypeScript (`.ts`).
    TypeScript,
    /// TypeScript with JSX syntax (`.tsx`).
    Tsx,
    /// Standard JavaScript (`.js`, `.mjs`, `.cjs`).
    JavaScript,
    /// JavaScript with JSX syntax (`.jsx`).
    Jsx,
}

impl TsDialect {
    const fn language(self) -> Language {
        match self {
            Self::TypeScript => Language::TypeScript,
            Self::Tsx => Language::Tsx,
            Self::JavaScript => Language::JavaScript,
            Self::Jsx => Language::Jsx,
        }
    }

    fn ts_grammar(self) -> tree_sitter::Language {
        match self {
            Self::TypeScript | Self::JavaScript => {
                tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into()
            }
            Self::Tsx | Self::Jsx => tree_sitter_typescript::LANGUAGE_TSX.into(),
        }
    }
}

/// `Tree-sitter` AST extractor for TypeScript/JavaScript dialects.
#[derive(Clone, Copy, Debug)]
pub struct TypeScriptExtractor {
    dialect: TsDialect,
}

impl TypeScriptExtractor {
    /// Creates an extractor configured for the given [`TsDialect`].
    #[must_use]
    pub const fn new(dialect: TsDialect) -> Self {
        Self { dialect }
    }
}

impl LanguageExtractor for TypeScriptExtractor {
    fn extract(&self, path: &RepoPath, source: &str) -> Result<UastDocument, ParseError> {
        let ts_lang = self.dialect.ts_grammar();
        let tree = parse_with_language(&ts_lang, source)?;
        let (total_lines, effective_lines) = line_metrics(source);
        let mut doc = UastDocument::new(
            path.clone(),
            self.dialect.language(),
            total_lines,
            effective_lines,
        )?;
        let root = tree.root_node();
        doc.syntax_errors = collect_syntax_errors(root);
        doc.referenced_symbols = collect_referenced_symbols(root, source);
        walk_ts(root, source, &mut doc);
        populate_cst_fortress(root, source, &mut doc);
        extract_ts_style_and_i18n(root, source, &mut doc);
        if matches!(self.dialect, TsDialect::Tsx | TsDialect::Jsx) {
            extract_jsx_ui_and_a11y(root, source, &mut doc);
        }
        Ok(doc)
    }
}

fn count_ts_params(params_node: Option<Node<'_>>, source: &str) -> u32 {
    let Some(node) = params_node else {
        return 0;
    };
    let mut cursor = node.walk();
    node.named_children(&mut cursor)
        .filter(|c| {
            c.kind() != "comment"
                && node_text(*c, source).split(':').next().unwrap_or("").trim() != "this"
        })
        .map(|p| single_param_arity(p, Some(source), TS_TUPLE_KINDS))
        .fold(0_u32, u32::saturating_add)
}

fn count_ts_bool_fields(body: Option<Node<'_>>, field_kinds: &[&str], source: &str) -> u32 {
    let Some(b) = body else {
        return 0;
    };
    let mut cursor = b.walk();
    let matched = b
        .named_children(&mut cursor)
        .filter(|ch| {
            field_kinds.contains(&ch.kind())
                && ch.child_by_field_name("type").is_some_and(|t| {
                    node_text(t, source).trim().trim_start_matches(':').trim() == "boolean"
                })
        })
        .count();
    u32::try_from(matched).unwrap_or(u32::MAX)
}

fn extract_ts_type(node: Node<'_>, source: &str) -> Option<TypeDecl> {
    let span = node_span(node);
    let body = node.child_by_field_name("body");
    match node.kind() {
        "class_declaration" => {
            let f_kinds = &["public_field_definition", "field_definition"];
            Some(TypeDecl {
                name: field_name_string(node, "name", "<class>", source),
                kind: TypeKind::Class,
                span,
                field_count: count_named_kinds(body, f_kinds),
                bool_field_count: count_ts_bool_fields(body, f_kinds, source),
                method_count: count_named_kinds(body, &["method_definition"]),
            })
        }
        "interface_declaration" => {
            let f_kinds = &["property_signature"];
            Some(TypeDecl {
                name: field_name_string(node, "name", "<interface>", source),
                kind: TypeKind::Interface,
                span,
                field_count: count_named_kinds(body, f_kinds),
                bool_field_count: count_ts_bool_fields(body, f_kinds, source),
                method_count: count_named_kinds(body, &["method_signature"]),
            })
        }
        _ => None,
    }
}

fn build_ts_callable(
    name: String,
    kind: CallableKind,
    target: Node<'_>,
    source: &str,
) -> CallableDecl {
    let param_count = target.child_by_field_name("parameter").map_or_else(
        || count_ts_params(target.child_by_field_name("parameters"), source),
        |p| single_param_arity(p, Some(source), TS_TUPLE_KINDS),
    );
    assemble_callable(
        CallableSpec {
            name,
            kind,
            decl_node: target,
            body_node: target.child_by_field_name("body"),
            param_count,
        },
        source,
        &TS_COMPLEXITY,
    )
}

fn record_ts_declarator(node: Node<'_>, source: &str, doc: &mut UastDocument) {
    let Some(val) = node.child_by_field_name("value") else {
        return;
    };
    if matches!(val.kind(), "arrow_function" | "function_expression") {
        let name = field_name_string(node, "name", "<arrow>", source);
        doc.callables
            .push(build_ts_callable(name, CallableKind::Closure, val, source));
    }
}

fn record_ts_node(node: Node<'_>, source: &str, doc: &mut UastDocument) {
    match node.kind() {
        "import_statement" => doc.imports.push(extract_ts_import(node, source)),
        "export_statement" => record_ts_export_stmt(node, source, doc),
        "call_expression" => record_ts_call_expr(node, source, doc),
        "function_declaration" => doc.callables.push(build_ts_callable(
            field_name_string(node, "name", "<fn>", source),
            CallableKind::Function,
            node,
            source,
        )),
        "method_definition" => doc.callables.push(build_ts_callable(
            field_name_string(node, "name", "<method>", source),
            CallableKind::Method,
            node,
            source,
        )),
        "variable_declarator" => record_ts_declarator(node, source, doc),
        _ => {
            if let Some(ty) = extract_ts_type(node, source) {
                doc.types.push(ty);
            }
        }
    }
}

fn walk_ts(node: Node<'_>, source: &str, doc: &mut UastDocument) {
    record_ts_node(node, source, doc);
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        walk_ts(child, source, doc);
    }
}
