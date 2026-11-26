//! Specification-grade Astro (`.astro` & Content Collections) 3-zone extractor.

mod splitter;

use crate::cst::{line_metrics, node_span, node_text, parse_with_language};
use crate::fingerprint::collect_referenced_symbols;
use crate::langs::css::CssExtractor;
use crate::langs::html_w3c::{HtmlElementNode, audit_html_element_tree, parse_html_template_tree};
use crate::langs::ts::{TsDialect, TypeScriptExtractor, is_hook_identifier};
use crate::types::{LanguageExtractor, ParseError};
use gf_core::{
    AstroViolation, DomainKind, FormeDocument, LangRuleFinding, Language, RepoPath, UastDocument,
};
use splitter::{AstroEmbeddedBlock, AstroParts, script_has_is_inline, split_astro_document};
use tree_sitter::Node;

#[rustfmt::skip]
const BROWSER_ONLY_GLOBALS: &[&str] = &["window", "document", "localStorage", "sessionStorage", "navigator"];
#[rustfmt::skip]
const CLIENT_HYDRATION_DIRECTIVES: &[&str] = &["client:load", "client:idle", "client:visible", "client:only", "client:media"];
#[rustfmt::skip]
const NON_CRITICAL_EAGER_ISLANDS: &[&str] = &["Card", "Footer", "Badge", "Banner", "Avatar", "Static", "Article"];

fn check_frontmatter_node(node: Node<'_>, source: &str, out: &mut Vec<LangRuleFinding>) {
    match node.kind() {
        "identifier" => {
            let text = node_text(node, source).trim();
            if BROWSER_ONLY_GLOBALS.contains(&text) {
                out.push(LangRuleFinding::new(
                    AstroViolation::FrontmatterClientApi,
                    node_span(node),
                    format!(
                        "browser global `{text}` is unavailable in Astro server `---` frontmatter"
                    ),
                ));
            }
        }
        "call_expression" => {
            if let Some(func) = node.child_by_field_name("function") {
                let callee = node_text(func, source).trim();
                let tail = callee.rsplit_once('.').map_or(callee, |(_, m)| m);
                if is_hook_identifier(tail) {
                    out.push(LangRuleFinding::new(
                        AstroViolation::FrontmatterClientApi,
                        node_span(node),
                        format!("React Hook `{callee}()` cannot execute in Astro server `---` frontmatter"),
                    ));
                }
            }
        }
        _ => {}
    }
}

fn walk_frontmatter_client_apis(node: Node<'_>, source: &str, out: &mut Vec<LangRuleFinding>) {
    check_frontmatter_node(node, source, out);
    let mut cursor = node.walk();
    for ch in node.children(&mut cursor) {
        walk_frontmatter_client_apis(ch, source, out);
    }
}

fn check_astro_element_directives(
    nodes: &[HtmlElementNode<'_>],
    template_source: &str,
    out: &mut Vec<LangRuleFinding>,
) {
    for node in nodes {
        if node
            .attrs
            .iter()
            .any(|a| a.name.eq_ignore_ascii_case("set:html"))
        {
            out.push(LangRuleFinding::new(
                AstroViolation::NoSetHtml,
                node.span,
                format!(
                    "`set:html` on `<{}>` injects unescaped HTML (XSS hazard)",
                    node.tag
                ),
            ));
        }
        if node
            .attrs
            .iter()
            .any(|a| CLIENT_HYDRATION_DIRECTIVES.contains(&a.name))
        {
            check_hydrated_island_node(node, template_source, out);
        }
        check_astro_element_directives(&node.children, template_source, out);
    }
}

fn is_non_serializable_island_attr(attr_name: &str, is_dynamic: bool, tag_line: &str) -> bool {
    if !is_dynamic {
        return false;
    }
    let is_cb = attr_name
        .strip_prefix("on")
        .and_then(|rest| rest.as_bytes().first())
        .is_some_and(u8::is_ascii_uppercase);
    is_cb || tag_line.contains("=>") || tag_line.contains("function(") || tag_line.contains("new ")
}

fn check_hydrated_island_node(
    node: &HtmlElementNode<'_>,
    template_source: &str,
    out: &mut Vec<LangRuleFinding>,
) {
    let eager = node
        .attrs
        .iter()
        .any(|a| matches!(a.name, "client:load" | "client:only"));
    if eager
        && NON_CRITICAL_EAGER_ISLANDS
            .iter()
            .any(|p| node.tag.starts_with(p))
    {
        out.push(LangRuleFinding::new(
            AstroViolation::ClientLoadBudget,
            node.span,
            format!(
                "eager `client:load`/`client:only` on `<{}>` bloats initial JS",
                node.tag
            ),
        ));
    }
    let line_idx = (node.span.start_line().saturating_sub(1)) as usize;
    let tag_line = template_source.lines().nth(line_idx).unwrap_or("");
    for attr in &node.attrs {
        if is_non_serializable_island_attr(attr.name, attr.is_dynamic, tag_line) {
            out.push(LangRuleFinding::new(
                AstroViolation::NonSerializableIslandProp,
                node.span,
                format!(
                    "non-serializable prop `{}` passed to hydrated island `<{}>`",
                    attr.name, node.tag
                ),
            ));
        }
    }
}

fn collect_template_refs(template: &str, doc: &mut UastDocument) {
    let ts_lang = tree_sitter_typescript::LANGUAGE_TSX.into();
    if let Ok(tree) = parse_with_language(&ts_lang, template) {
        doc.referenced_symbols
            .extend(collect_referenced_symbols(tree.root_node(), template));
    }
}

fn merge_astro_frontmatter(path: &RepoPath, fm: &str, doc: &mut UastDocument) {
    let Ok(fm_doc) = TypeScriptExtractor::new(TsDialect::TypeScript).extract_in_domain(
        path,
        fm,
        DomainKind::Code,
    ) else {
        return;
    };
    doc.syntax_errors.extend(fm_doc.syntax_errors);
    doc.imports.extend(fm_doc.imports);
    doc.exports.extend(fm_doc.exports);
    doc.callables.extend(fm_doc.callables);
    doc.types.extend(fm_doc.types);
    doc.referenced_symbols.extend(fm_doc.referenced_symbols);
    doc.translation_keys_used
        .extend(fm_doc.translation_keys_used);
    doc.bypass_findings.extend(fm_doc.bypass_findings);
    doc.kiss_yagni_findings.extend(fm_doc.kiss_yagni_findings);
    doc.lang_rule_findings.extend(fm_doc.lang_rule_findings);
    let ts_lang = tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into();
    if let Ok(tree) = parse_with_language(&ts_lang, fm) {
        walk_frontmatter_client_apis(tree.root_node(), fm, &mut doc.lang_rule_findings);
    }
}

fn merge_single_script(path: &RepoPath, script: &AstroEmbeddedBlock, doc: &mut UastDocument) {
    if script_has_is_inline(&script.attrs_raw) {
        doc.lang_rule_findings.push(LangRuleFinding::new(
            AstroViolation::ClientLoadBudget,
            script.open_tag_span,
            String::from("`<script is:inline>` bypasses Astro bundling and TypeScript checking"),
        ));
    }
    if let Ok(sc_doc) = TypeScriptExtractor::new(TsDialect::TypeScript).extract_in_domain(
        path,
        &script.line_aligned_body,
        DomainKind::Code,
    ) {
        doc.syntax_errors.extend(sc_doc.syntax_errors);
        doc.referenced_symbols.extend(sc_doc.referenced_symbols);
        doc.translation_keys_used
            .extend(sc_doc.translation_keys_used);
        doc.lang_rule_findings.extend(sc_doc.lang_rule_findings);
    }
}

fn merge_single_style(path: &RepoPath, style: &AstroEmbeddedBlock, doc: &mut UastDocument) {
    let Ok(css_doc) = CssExtractor.extract(path, &style.line_aligned_body) else {
        return;
    };
    doc.syntax_errors.extend(css_doc.syntax_errors);
    doc.lang_rule_findings.extend(css_doc.lang_rule_findings);
    doc.asset_refs.extend(css_doc.asset_refs);
    if let Some(f) = css_doc.forme {
        doc.forme
            .get_or_insert_with(|| FormeDocument {
                findings: Vec::new(),
            })
            .findings
            .extend(f.findings);
    }
}

/// Specification-grade 3-zone extractor for `.astro` components, layouts, and pages.
#[derive(Clone, Copy, Debug, Default)]
pub struct AstroExtractor;

impl LanguageExtractor for AstroExtractor {
    fn extract(&self, path: &RepoPath, source: &str) -> Result<UastDocument, ParseError> {
        let (total_lines, effective_lines) = line_metrics(source);
        let mut doc =
            UastDocument::new(path.clone(), Language::Astro, total_lines, effective_lines)?;
        let parts: AstroParts = split_astro_document(source);
        if let Some(fm) = &parts.frontmatter_line_aligned {
            merge_astro_frontmatter(path, fm, &mut doc);
        }
        let tree = parse_html_template_tree(&parts.template_line_aligned);
        audit_html_element_tree(&tree, &mut doc.lang_rule_findings);
        check_astro_element_directives(
            &tree,
            &parts.template_line_aligned,
            &mut doc.lang_rule_findings,
        );
        collect_template_refs(&parts.template_line_aligned, &mut doc);
        for script in &parts.scripts {
            merge_single_script(path, script, &mut doc);
        }
        for style in &parts.styles {
            merge_single_style(path, style, &mut doc);
        }
        doc.lang_rule_findings
            .sort_by(|a, b| a.span.cmp(&b.span).then_with(|| a.code().cmp(b.code())));
        Ok(doc)
    }
}
