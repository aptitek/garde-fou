//! Raw HTML tag filter (`MD-RAW-HTML`) and `.mdx` JSX/TSX cross-AST bridge
//! (`MD-FORBIDDEN-MDX-LOGIC` -> `GF063`).

use gf_core::{
    ContenusIssue, DomainKind, MarkdownViolation, RepoPath, Span, UastDocument, UiCheckKind,
};

use super::slugger_links::push_md_issue;
use crate::langs::ts::{TsDialect, TypeScriptExtractor};

fn extract_leading_tag_name(html: &str) -> Option<&str> {
    let trimmed = html.trim_start();
    if trimmed.starts_with("<!--") {
        return None;
    }
    let after_lt = trimmed.strip_prefix('<')?;
    let after_slash = after_lt.strip_prefix('/').unwrap_or(after_lt);
    let name_len = after_slash
        .bytes()
        .take_while(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.'))
        .count();
    let name = &after_slash[..name_len];
    (!name.is_empty()).then_some(name)
}

fn collect_bridged_tsx_issues(
    ts_doc: UastDocument,
    base_line: u32,
    issues: &mut Vec<ContenusIssue>,
) {
    let remap = |inner_line: u32| -> Span {
        let l = base_line.saturating_add(inner_line.saturating_sub(2));
        Span::new(l, l).unwrap_or(Span::FIRST)
    };
    for leak in ts_doc.style_leaks {
        push_md_issue(
            issues,
            MarkdownViolation::ForbiddenMdxLogic,
            remap(leak.span.start_line()),
            format!("impure style/asset prop in MDX component: {}", leak.detail),
        );
    }
    for f in ts_doc.lang_rule_findings {
        push_md_issue(
            issues,
            MarkdownViolation::ForbiddenMdxLogic,
            remap(f.span.start_line()),
            format!(
                "impure JSX/React construct in MDX (`{}`): {}",
                f.code(),
                f.message
            ),
        );
    }
    for ui in ts_doc.ui_checks {
        if ui.kind == UiCheckKind::NonInteractiveClick {
            push_md_issue(
                issues,
                MarkdownViolation::ForbiddenMdxLogic,
                remap(ui.span.start_line()),
                format!(
                    "interactive `onClick` handler in editorial MDX: {}",
                    ui.detail
                ),
            );
        }
    }
}

fn parse_mdx_tsx_fragment(trimmed: &str) -> Option<UastDocument> {
    let dummy_path = RepoPath::new("src/__mdx_bridge.tsx").ok()?;
    let extractor = TypeScriptExtractor::new(TsDialect::Tsx);
    let wrap = |expr: &str| format!("export const __mdx_frag = (\n{expr}\n);\n");
    let doc = extractor
        .extract_in_domain(&dummy_path, &wrap(trimmed), DomainKind::Code)
        .ok()?;
    if doc.syntax_errors.is_empty() {
        return Some(doc);
    }
    let stem = trimmed.strip_suffix('>')?;
    extractor
        .extract_in_domain(&dummy_path, &wrap(&format!("{stem} />")), DomainKind::Code)
        .ok()
        .filter(|d| d.syntax_errors.is_empty())
}

fn bridge_mdx_jsx_fragment(jsx_raw: &str, base_line: u32, issues: &mut Vec<ContenusIssue>) {
    let trimmed = jsx_raw.trim();
    if trimmed.starts_with("</") {
        return;
    }
    let prev_len = issues.len();
    if let Some(ts_doc) = parse_mdx_tsx_fragment(trimmed) {
        collect_bridged_tsx_issues(ts_doc, base_line, issues);
    }
    if issues.len() == prev_len
        && (trimmed.contains("onClick=") || trimmed.contains("dangerouslySetInnerHTML"))
    {
        push_md_issue(
            issues,
            MarkdownViolation::ForbiddenMdxLogic,
            Span::new(base_line, base_line).unwrap_or(Span::FIRST),
            String::from("forbidden `onClick` or `dangerouslySetInnerHTML` prop in MDX"),
        );
    }
}

/// Inspects an `Event::Html` or `Event::InlineHtml` fragment in `.md` or `.mdx`.
pub(super) fn inspect_html_or_mdx_tag(
    html: &str,
    is_mdx: bool,
    line: u32,
    issues: &mut Vec<ContenusIssue>,
) {
    let span = Span::new(line, line).unwrap_or(Span::FIRST);
    let Some(tag_name) = extract_leading_tag_name(html) else {
        return;
    };
    let starts_upper = tag_name.chars().next().is_some_and(char::is_uppercase);
    if !is_mdx || !starts_upper {
        push_md_issue(
            issues,
            MarkdownViolation::RawHtml,
            span,
            format!("raw HTML tag `<{tag_name}>` outside fenced/inline code"),
        );
        return;
    }
    bridge_mdx_jsx_fragment(html, line, issues);
}

fn has_mdx_style_import(trimmed: &str) -> bool {
    trimmed.starts_with("import ")
        && [".css\"", ".css'", ".scss\"", ".scss'"]
            .iter()
            .any(|ext| trimmed.contains(ext))
}

fn inspect_mdx_line(trimmed: &str, line_no: u32, issues: &mut Vec<ContenusIssue>) {
    let span = Span::new(line_no, line_no).unwrap_or(Span::FIRST);
    if trimmed.starts_with("export ") {
        push_md_issue(
            issues,
            MarkdownViolation::ForbiddenMdxLogic,
            span,
            String::from("`export` logic declaration inside editorial `.mdx`"),
        );
    } else if has_mdx_style_import(trimmed) {
        push_md_issue(
            issues,
            MarkdownViolation::ForbiddenMdxLogic,
            span,
            String::from("stylesheet `import` inside `.mdx` (`Forme` leak)"),
        );
    } else if extract_leading_tag_name(trimmed)
        .and_then(|t| t.chars().next())
        .is_some_and(char::is_uppercase)
        && !issues.iter().any(|i| i.span.start_line() == line_no)
    {
        bridge_mdx_jsx_fragment(trimmed, line_no, issues);
    } else if ["useState(", "useEffect(", "useReducer(", "fetch(", "axios."]
        .iter()
        .any(|tok| trimmed.contains(tok))
    {
        push_md_issue(
            issues,
            MarkdownViolation::ForbiddenMdxLogic,
            span,
            String::from("stateful React hook or network call inside editorial `.mdx`"),
        );
    }
}

/// Scans `.mdx` lines outside fenced code blocks for `export`, hooks, `fetch`, and `.css` imports.
pub(super) fn scan_mdx_esm_and_expressions(
    body: &str,
    body_start_line: u32,
    fenced_ranges: &[(usize, usize)],
    issues: &mut Vec<ContenusIssue>,
) {
    let mut byte_off = 0usize;
    for (idx, raw_line) in body.split_inclusive('\n').enumerate() {
        let in_fence = fenced_ranges
            .iter()
            .any(|&(s, e)| (s..e).contains(&byte_off));
        byte_off = byte_off.saturating_add(raw_line.len());
        if !in_fence {
            let line_no = body_start_line.saturating_add(u32::try_from(idx).unwrap_or(0));
            inspect_mdx_line(raw_line.trim(), line_no, issues);
        }
    }
}
