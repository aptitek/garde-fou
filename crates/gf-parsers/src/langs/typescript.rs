//! Safe `Tree-sitter` extractor for TypeScript, TSX, JavaScript, and JSX.

use gf_core::{
    CallableDecl, CallableKind, ExportDecl, ImportDecl, Language, RepoPath, TypeDecl, TypeKind,
    UastDocument,
};
use tree_sitter::Node;

use crate::complexity::ComplexityRules;
use crate::cst::{
    CallableSpec, assemble_callable, collect_syntax_errors, count_named_kinds, field_name_string,
    line_metrics, node_span, node_text, parse_with_language,
};
use crate::fingerprint::collect_referenced_symbols;
use crate::{LanguageExtractor, ParseError};

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
        Ok(doc)
    }
}

fn count_ts_params(params_node: Option<Node<'_>>) -> u32 {
    let Some(node) = params_node else {
        return 0;
    };
    let mut cursor = node.walk();
    u32::try_from(node.named_children(&mut cursor).count()).unwrap_or(u32::MAX)
}

fn extract_import(node: Node<'_>, source: &str) -> ImportDecl {
    let module_source = node
        .child_by_field_name("source")
        .map_or("", |s| node_text(s, source))
        .trim_matches(['"', '\'', '`'])
        .to_string();
    ImportDecl {
        module_source,
        symbols: Vec::new(),
        span: node_span(node),
    }
}

fn extract_exports(node: Node<'_>, source: &str, out: &mut Vec<ExportDecl>) {
    let span = node_span(node);
    let Some(decl) = node.child_by_field_name("declaration") else {
        return;
    };
    if let Some(name) = decl.child_by_field_name("name") {
        out.push(ExportDecl {
            symbol: String::from(node_text(name, source)),
            span,
        });
        return;
    }
    if decl.kind() == "lexical_declaration" {
        let mut cursor = decl.walk();
        for child in decl.named_children(&mut cursor) {
            if child.kind() == "variable_declarator"
                && let Some(name) = child.child_by_field_name("name")
            {
                out.push(ExportDecl {
                    symbol: String::from(node_text(name, source)),
                    span,
                });
            }
        }
    }
}

fn extract_ts_type(node: Node<'_>, source: &str) -> Option<TypeDecl> {
    let span = node_span(node);
    let body = node.child_by_field_name("body");
    match node.kind() {
        "class_declaration" => Some(TypeDecl {
            name: field_name_string(node, "name", "<class>", source),
            kind: TypeKind::Class,
            span,
            field_count: count_named_kinds(body, &["public_field_definition", "field_definition"]),
            method_count: count_named_kinds(body, &["method_definition"]),
        }),
        "interface_declaration" => Some(TypeDecl {
            name: field_name_string(node, "name", "<interface>", source),
            kind: TypeKind::Interface,
            span,
            field_count: count_named_kinds(body, &["property_signature"]),
            method_count: count_named_kinds(body, &["method_signature"]),
        }),
        _ => None,
    }
}

fn build_ts_callable(
    name: String,
    kind: CallableKind,
    target: Node<'_>,
    source: &str,
) -> CallableDecl {
    let param_count = if target.child_by_field_name("parameter").is_some() {
        1
    } else {
        count_ts_params(target.child_by_field_name("parameters"))
    };
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
        "import_statement" => doc.imports.push(extract_import(node, source)),
        "export_statement" => extract_exports(node, source, &mut doc.exports),
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
