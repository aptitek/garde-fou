//! Line-level Markdown/MDX prose scanners: inline-code stripping, image extraction,
//! raw HTML detection, and MDX logic checks (`GF063` / `GF064`).

use gf_core::{ContenusIssue, ContenusIssueKind, Span, UastDocument};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum MdBlockMode {
    Prose,
    Frontmatter,
    CodeFence { marker: char, len: usize },
}

/// Mutable line-by-line Markdown/MDX scan session replacing raw tuple accumulators.
pub(super) struct MdScanSession<'a> {
    pub is_mdx: bool,
    pub mode: MdBlockMode,
    pub prev_heading: u8,
    pub h1_count: u32,
    pub issues: Vec<ContenusIssue>,
    pub doc: &'a mut UastDocument,
}

/// Removes inline backtick code spans (`` `...` `` / ```` ``...`` ````) from a single line.
pub fn strip_inline_code_spans(line: &str) -> String {
    let bytes = line.as_bytes();
    let mut out = String::with_capacity(line.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'`' {
            let mut ticks = 0;
            while i + ticks < bytes.len() && bytes[i + ticks] == b'`' {
                ticks += 1;
            }
            let opener_end = i + ticks;
            let needle = &line[i..opener_end];
            if let Some(rel_close) = line[opener_end..].find(needle) {
                i = opener_end + rel_close + ticks;
                out.push(' ');
                continue;
            }
        }
        let ch = line[i..].chars().next().unwrap_or(' ');
        out.push(ch);
        i += ch.len_utf8();
    }
    out
}

fn is_local_markdown_asset_target(raw_target: &str) -> bool {
    !raw_target.is_empty()
        && !raw_target.contains("://")
        && !raw_target.starts_with('#')
        && !raw_target.starts_with("mailto:")
}

impl MdScanSession<'_> {
    fn record_md_image_target(&mut self, alt: &str, raw_target: &str, span: Span) {
        if alt.trim().is_empty() {
            self.issues.push(ContenusIssue {
                kind: ContenusIssueKind::MissingImageAlt,
                detail: format!("Markdown image `![]({raw_target})` has empty `alt` text"),
                span,
            });
        }
        if raw_target.to_ascii_lowercase().starts_with("data:") {
            self.issues.push(ContenusIssue {
                kind: ContenusIssueKind::DataUri,
                detail: String::from("inline `data:` URI in Markdown image"),
                span,
            });
        } else if is_local_markdown_asset_target(raw_target) {
            let clean = raw_target.split(['?', '#']).next().unwrap_or(raw_target);
            if !clean.is_empty() {
                self.doc.asset_refs.insert(String::from(clean));
            }
        }
    }

    /// Extracts Markdown `![alt](target)` image links, flagging missing `alt` and `data:` URIs.
    pub(super) fn extract_md_images(&mut self, prose: &str, span: Span) {
        let mut from = 0;
        while let Some(rel) = prose[from..].find("![") {
            let alt_start = from + rel + 2;
            let Some(alt_end_rel) = prose[alt_start..].find("](") else {
                break;
            };
            let alt = &prose[alt_start..alt_start + alt_end_rel];
            let url_start = alt_start + alt_end_rel + 2;
            let Some(url_end_rel) = prose[url_start..].find(')') else {
                break;
            };
            let raw_target = prose[url_start..url_start + url_end_rel]
                .split_whitespace()
                .next()
                .unwrap_or("");
            self.record_md_image_target(alt, raw_target, span);
            from = url_start + url_end_rel + 1;
        }
    }

    /// Detects forbidden JS/React exports, hooks, `fetch`, inline `style={{`, or `.css` imports in MDX.
    pub(super) fn check_mdx_logic(&mut self, trimmed: &str, span: Span) {
        let forbidden = [
            "export function ",
            "export const ",
            "export default ",
            "useState(",
            "useEffect(",
            "fetch(",
            "style={{",
        ];
        if let Some(&pat) = forbidden.iter().find(|&&p| trimmed.contains(p)) {
            self.issues.push(ContenusIssue {
                kind: ContenusIssueKind::ForbiddenMdxLogic,
                detail: format!("forbidden logic/style `{}` inside MDX content", pat.trim()),
                span,
            });
        } else if trimmed.starts_with("import ")
            && (trimmed.contains(".css") || trimmed.contains(".scss"))
        {
            self.issues.push(ContenusIssue {
                kind: ContenusIssueKind::ForbiddenMdxLogic,
                detail: String::from("direct stylesheet import inside MDX content"),
                span,
            });
        }
    }
}

fn parse_html_tag_after_lt(prose: &str, lt_idx: usize, is_mdx: bool) -> Option<String> {
    let bytes = prose.as_bytes();
    let after_lt = if bytes.get(lt_idx + 1) == Some(&b'/') {
        lt_idx + 2
    } else {
        lt_idx + 1
    };
    let rest = prose.get(after_lt..)?;
    let tag: String = rest
        .chars()
        .take_while(|c| c.is_ascii_alphanumeric() || *c == '-')
        .collect();
    let first_ch = tag.chars().next().filter(char::is_ascii_alphabetic)?;
    let next_ch = rest.get(tag.len()..)?.chars().next()?;
    if !matches!(next_ch, ' ' | '\t' | '>' | '/') {
        return None;
    }
    (!(is_mdx && first_ch.is_ascii_uppercase())).then_some(tag)
}

fn strip_html_comments(line: &str) -> String {
    let mut out = String::with_capacity(line.len());
    let mut rest = line;
    while let Some((before, after_close)) = rest
        .split_once("<!--")
        .and_then(|(b, after_open)| after_open.split_once("-->").map(|(_, ac)| (b, ac)))
    {
        out.push_str(before);
        out.push(' ');
        rest = after_close;
    }
    if let Some((before, _)) = rest.split_once("<!--") {
        out.push_str(before);
    } else {
        out.push_str(rest);
    }
    out
}

/// Detects any raw HTML tag (`<div>`, `<script>`, etc.) in code-stripped prose.
pub fn detect_raw_html_tag(prose: &str, is_mdx: bool) -> Option<String> {
    let uncommented = strip_html_comments(prose);
    uncommented
        .bytes()
        .enumerate()
        .filter(|&(_, b)| b == b'<')
        .find_map(|(idx, _)| parse_html_tag_after_lt(&uncommented, idx, is_mdx))
}
