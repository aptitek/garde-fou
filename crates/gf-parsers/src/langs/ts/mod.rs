//! Modular TypeScript, TSX, JavaScript, JSX, and React 19 CST extractor (`langs/ts/*`).

mod callables;
mod jsx_dangerous;
mod jsx_keys;
mod jsx_semantics;
mod modules_and_types;
mod react_classify;
mod react_hooks;
pub mod routing;
mod style_and_i18n;
mod ts_safety;
pub mod tsconfig_jsonc;

use gf_core::{
    DomainKind, DomainLayoutConfig, RepoPath, UastDocument, classify_path, is_ts_declaration_path,
};

pub use react_classify::is_hook_identifier;
pub use routing::TsDialect;

use super::sql::embedded_sql::extract_ts_embedded_sql;
use crate::anti_cheat::populate_cst_fortress;
use crate::cst::{collect_syntax_errors, line_metrics, parse_with_language};
use crate::fingerprint::collect_referenced_symbols;
use crate::types::{LanguageExtractor, ParseError};
use callables::record_ts_callable_or_type;
use jsx_keys::extract_jsx_iterator_keys;
use jsx_semantics::extract_jsx_semantics;
use modules_and_types::{
    extract_ts_import, merge_declaration_exports, record_dynamic_import_call, record_ts_export_stmt,
};
use react_hooks::extract_react_hooks;
use style_and_i18n::extract_ts_style_and_i18n;
use ts_safety::{TsSafetyCtx, extract_ts_safety};

/// `Tree-sitter` AST extractor for TypeScript, TSX, JavaScript, and JSX files.
#[derive(Clone, Copy, Debug)]
pub struct TypeScriptExtractor {
    dialect: TsDialect,
}

impl TypeScriptExtractor {
    /// Creates an extractor configured for `dialect`.
    #[must_use]
    pub const fn new(dialect: TsDialect) -> Self {
        Self { dialect }
    }

    /// Extracts a TypeScript/React [`UastDocument`] with explicit [`DomainKind`] (`GF080`).
    ///
    /// # Errors
    ///
    /// Returns [`ParseError`] if `Tree-sitter` initialization or span creation fails.
    pub fn extract_in_domain(
        self,
        path: &RepoPath,
        source: &str,
        domain: DomainKind,
    ) -> Result<UastDocument, ParseError> {
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
        let is_dts = is_ts_declaration_path(path.as_str());
        doc.syntax_errors = collect_syntax_errors(root);
        doc.referenced_symbols = collect_referenced_symbols(root, source);
        walk_ts_structure(root, source, is_dts, &mut doc);
        merge_declaration_exports(&mut doc.exports);
        populate_cst_fortress(root, source, &mut doc);
        extract_ts_style_and_i18n(root, source, &mut doc);
        extract_ts_safety(
            root,
            TsSafetyCtx {
                source,
                in_code: domain == DomainKind::Code,
                is_dts,
            },
            &mut doc,
        );
        if !is_dts {
            extract_react_hooks(root, source, &mut doc.lang_rule_findings);
            if domain == DomainKind::Code {
                extract_ts_embedded_sql(root, source, &mut doc.lang_rule_findings);
            }
        }
        if self.dialect.supports_jsx() {
            extract_jsx_semantics(root, source, false, &mut doc);
            extract_jsx_iterator_keys(root, source, &mut doc.lang_rule_findings);
        }
        doc.lang_rule_findings
            .sort_by(|a, b| a.span.cmp(&b.span).then_with(|| a.code().cmp(b.code())));
        Ok(doc)
    }
}

impl LanguageExtractor for TypeScriptExtractor {
    fn extract(&self, path: &RepoPath, source: &str) -> Result<UastDocument, ParseError> {
        let domain = classify_path(path, &DomainLayoutConfig::strict_defaults());
        self.extract_in_domain(path, source, domain)
    }
}

fn walk_ts_structure(
    node: tree_sitter::Node<'_>,
    source: &str,
    is_dts: bool,
    doc: &mut UastDocument,
) {
    match node.kind() {
        "import_statement" => doc.imports.push(extract_ts_import(node, source)),
        "export_statement" => record_ts_export_stmt(node, source, is_dts, doc),
        "call_expression" => record_dynamic_import_call(node, source, doc),
        _ => record_ts_callable_or_type(node, source, is_dts, doc),
    }
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        walk_ts_structure(child, source, is_dts, doc);
    }
}
