//! Modular TypeScript, TSX, JavaScript, JSX, and React 19 CST extractor (`langs/ts/*`).

mod callables;
mod cloudflare_workers;
mod drizzle_orm;
mod jsx_dangerous;
mod jsx_keys;
mod jsx_semantics;
mod modules_and_types;
pub(crate) mod panda_m3e;
mod react_classify;
mod react_hooks;
mod react_router;
pub mod routing;
mod style_and_i18n;
mod ts_safety;
pub mod tsconfig_jsonc;
pub(crate) mod wrangler_config;

use gf_core::{
    DomainKind, DomainLayoutConfig, LangRuleFinding, PandaViolation, RepoPath, Span,
    SqlDialectKind, UastDocument, classify_path, is_ts_declaration_path,
};

pub use react_classify::is_hook_identifier;
pub use routing::TsDialect;

use super::html_w3c::audit_jsx_tree_w3c_a11y;
use super::sql::embedded_sql::extract_ts_embedded_sql;
use crate::anti_cheat::populate_cst_fortress;
use crate::cst::{collect_syntax_errors, line_metrics, parse_with_language};
use crate::fingerprint::collect_referenced_symbols;
use crate::types::{LanguageExtractor, ParseError};
use {
    callables::record_ts_callable_or_type,
    cloudflare_workers::extract_cloudflare_workers_rules,
    drizzle_orm::extract_drizzle_orm_rules,
    jsx_keys::extract_jsx_iterator_keys,
    jsx_semantics::extract_jsx_semantics,
    modules_and_types::{
        extract_ts_import, merge_declaration_exports, record_dynamic_import_call,
        record_ts_export_stmt,
    },
    panda_m3e::extract_panda_and_m3e,
    react_hooks::extract_react_hooks,
    react_router::extract_react_router_rules,
    style_and_i18n::extract_ts_style_and_i18n,
    ts_safety::{TsSafetyCtx, extract_ts_safety},
};

fn is_committed_styled_system_path(path: &str) -> bool {
    path.starts_with("styled-system/") || path.contains("/styled-system/")
}

struct TsSemanticPassCtx<'a> {
    path: &'a RepoPath,
    source: &'a str,
    domain: DomainKind,
    is_dts: bool,
    supports_jsx: bool,
    sql_dialect_hint: Option<SqlDialectKind>,
}

fn run_ts_semantic_passes(
    root: tree_sitter::Node<'_>,
    ctx: &TsSemanticPassCtx<'_>,
    doc: &mut UastDocument,
) {
    extract_ts_style_and_i18n(root, ctx.source, doc);
    extract_ts_safety(
        root,
        TsSafetyCtx {
            source: ctx.source,
            in_code: ctx.domain == DomainKind::Code,
            is_dts: ctx.is_dts,
        },
        doc,
    );
    if !ctx.is_dts {
        extract_react_hooks(root, ctx.source, &mut doc.lang_rule_findings);
        if ctx.domain == DomainKind::Code {
            extract_ts_embedded_sql(root, ctx.source, &mut doc.lang_rule_findings);
            extract_panda_and_m3e(root, ctx.source, &mut doc.lang_rule_findings);
        }
        extract_react_router_rules(root, ctx.path, ctx.source, doc);
        extract_cloudflare_workers_rules(root, ctx.source, doc);
        extract_drizzle_orm_rules(root, ctx.source, ctx.sql_dialect_hint, doc);
    }
    if ctx.supports_jsx {
        extract_jsx_semantics(root, ctx.source, false, doc);
        extract_jsx_iterator_keys(root, ctx.source, &mut doc.lang_rule_findings);
        audit_jsx_tree_w3c_a11y(root, ctx.source, &mut doc.lang_rule_findings);
    }
}

/// `Tree-sitter` AST extractor for TypeScript, TSX, JavaScript, and JSX files.
#[derive(Clone, Copy, Debug)]
pub struct TypeScriptExtractor {
    dialect: TsDialect,
    sql_dialect_hint: Option<SqlDialectKind>,
}

impl TypeScriptExtractor {
    /// Creates an extractor configured for `dialect`.
    #[must_use]
    pub const fn new(dialect: TsDialect) -> Self {
        Self {
            dialect,
            sql_dialect_hint: None,
        }
    }

    /// Attaches an optional workspace [`SqlDialectKind`] hint (`DRIZZLE-DIALECT-MISMATCH`).
    #[must_use]
    pub const fn with_sql_dialect(mut self, sql_dialect_hint: Option<SqlDialectKind>) -> Self {
        self.sql_dialect_hint = sql_dialect_hint;
        self
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
        if is_committed_styled_system_path(path.as_str()) {
            doc.lang_rule_findings.push(LangRuleFinding::new(
                PandaViolation::CommittedStyledSystem,
                Span::FIRST,
                format!("generated Panda CSS file `{path}` is tracked in version control; add `styled-system/` to `.gitignore`"),
            ));
            return Ok(doc);
        }
        let root = tree.root_node();
        let is_dts = is_ts_declaration_path(path.as_str());
        doc.syntax_errors = collect_syntax_errors(root);
        doc.referenced_symbols = collect_referenced_symbols(root, source);
        walk_ts_structure(root, source, is_dts, &mut doc);
        merge_declaration_exports(&mut doc.exports);
        populate_cst_fortress(root, source, &mut doc);
        run_ts_semantic_passes(
            root,
            &TsSemanticPassCtx {
                path,
                source,
                domain,
                is_dts,
                supports_jsx: self.dialect.supports_jsx(),
                sql_dialect_hint: self.sql_dialect_hint,
            },
            &mut doc,
        );
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
