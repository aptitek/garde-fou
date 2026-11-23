//! Safe-Rust lexical/structural CSS & SCSS analyzer for `DomainKind::Forme` (`GF061`/`GF064`).

use gf_core::{
    FormeDocument, FormeFinding, FormeFindingKind, Language, RepoPath, Span, UastDocument,
};

use crate::cst::line_metrics;
use crate::{LanguageExtractor, ParseError};

/// Safe-Rust stylesheet extractor for `.css` and `.scss` files.
#[derive(Clone, Copy, Debug, Default)]
pub struct CssExtractor;

impl LanguageExtractor for CssExtractor {
    fn extract(&self, path: &RepoPath, source: &str) -> Result<UastDocument, ParseError> {
        let (total_lines, effective_lines) = line_metrics(source);
        let mut doc = UastDocument::new(path.clone(), Language::Css, total_lines, effective_lines)?;
        let stripped = strip_block_comments_preserving_lines(source);
        let mut findings = Vec::new();

        for (idx, raw_line) in stripped.lines().enumerate() {
            let line_no = u32::try_from(idx).unwrap_or(u32::MAX).saturating_add(1);
            let span = Span::new(line_no, line_no).unwrap_or(Span::FIRST);
            let line = strip_scss_line_comment(raw_line).trim();
            if !line.is_empty() {
                scan_css_line(line, span, &mut findings, &mut doc);
            }
        }

        doc.forme = Some(FormeDocument { findings });
        Ok(doc)
    }
}

fn strip_block_comments_preserving_lines(source: &str) -> String {
    let mut out = String::with_capacity(source.len());
    let mut chars = source.chars().peekable();
    let mut in_comment = false;

    while let Some(ch) = chars.next() {
        if in_comment {
            if ch == '*' && chars.peek() == Some(&'/') {
                let _ = chars.next();
                in_comment = false;
            } else if ch == '\n' {
                out.push('\n');
            }
        } else if ch == '/' && chars.peek() == Some(&'*') {
            let _ = chars.next();
            in_comment = true;
        } else {
            out.push(ch);
        }
    }
    out
}

fn is_unquoted_line_comment_at(bytes: &[u8], idx: usize) -> bool {
    bytes.get(idx) == Some(&b'/')
        && bytes.get(idx + 1) == Some(&b'/')
        && (idx == 0 || bytes[idx - 1] != b':')
}

fn strip_scss_line_comment(line: &str) -> &str {
    let bytes = line.as_bytes();
    let mut in_quote: Option<u8> = None;
    for (idx, &b) in bytes.iter().enumerate() {
        match in_quote {
            Some(q) if b == q => in_quote = None,
            None if b == b'"' || b == b'\'' => in_quote = Some(b),
            None if is_unquoted_line_comment_at(bytes, idx) => return &line[..idx],
            Some(_) | None => {}
        }
    }
    line
}

fn extract_urls(line: &str, span: Span, findings: &mut Vec<FormeFinding>, doc: &mut UastDocument) {
    let lower = line.to_ascii_lowercase();
    let mut search_from = 0;
    while let Some(rel_idx) = lower[search_from..].find("url(") {
        let start = search_from + rel_idx + 4;
        let Some(end_rel) = line[start..].find(')') else {
            break;
        };
        let raw_url = line[start..start + end_rel]
            .trim()
            .trim_matches(['"', '\'']);
        if raw_url.to_ascii_lowercase().starts_with("data:") {
            findings.push(FormeFinding {
                kind: FormeFindingKind::DataUri,
                detail: String::from("embedded `url(data:...)` in stylesheet"),
                span,
            });
        } else if !raw_url.is_empty()
            && !raw_url.contains("://")
            && !raw_url.starts_with('#')
            && !raw_url.starts_with("var(")
        {
            let clean = raw_url.split(['?', '#']).next().unwrap_or(raw_url);
            if !clean.is_empty() {
                doc.asset_refs.insert(String::from(clean));
            }
        }
        search_from = start + end_rel + 1;
    }
}

fn check_content_property(line: &str, span: Span, findings: &mut Vec<FormeFinding>) {
    let Some((prop, rhs)) = line.split_once(':') else {
        return;
    };
    if !prop.trim().eq_ignore_ascii_case("content") {
        return;
    }
    let val = rhs.trim().trim_end_matches(';').trim();
    if let Some(unquoted) = val
        .strip_prefix('"')
        .and_then(|s| s.strip_suffix('"'))
        .or_else(|| val.strip_prefix('\'').and_then(|s| s.strip_suffix('\'')))
        && unquoted.chars().filter(|c| c.is_alphabetic()).count() >= 2
    {
        findings.push(FormeFinding {
            kind: FormeFindingKind::TranslatedContentText,
            detail: format!("translatable text in CSS `content: \"{unquoted}\"`"),
            span,
        });
    }
}

fn detect_raw_color_in_value(rhs: &str) -> Option<String> {
    let lower = rhs.to_ascii_lowercase();
    for fn_name in ["rgb(", "rgba(", "hsl(", "hsla(", "oklch(", "oklab("] {
        if lower.contains(fn_name) {
            return Some(String::from(fn_name.trim_end_matches('(')));
        }
    }
    for tok in rhs.split(|c: char| c.is_whitespace() || matches!(c, ',' | ';' | '(' | ')')) {
        if let Some(hex) = tok.strip_prefix('#')
            && matches!(hex.len(), 3 | 4 | 6 | 8)
            && hex.bytes().all(|b| b.is_ascii_hexdigit())
        {
            return Some(String::from(tok));
        }
    }
    None
}

fn detect_raw_px_in_value(rhs: &str) -> Option<String> {
    for tok in rhs.split(|c: char| c.is_whitespace() || matches!(c, ',' | ';' | '(' | ')')) {
        if let Some(num) = tok.strip_suffix("px")
            && !num.is_empty()
            && num != "0"
            && num
                .chars()
                .all(|c| c.is_ascii_digit() || c == '.' || c == '-')
            && num.chars().any(|c| c.is_ascii_digit())
        {
            return Some(String::from(tok));
        }
    }
    None
}

fn scan_css_line(line: &str, span: Span, findings: &mut Vec<FormeFinding>, doc: &mut UastDocument) {
    extract_urls(line, span, findings, doc);
    check_content_property(line, span, findings);

    if line.to_ascii_lowercase().contains("!important") {
        findings.push(FormeFinding {
            kind: FormeFindingKind::ImportantOverride,
            detail: String::from("`!important` override"),
            span,
        });
    }

    let Some((_, rhs)) = line.split_once(':') else {
        return;
    };
    if let Some(color) = detect_raw_color_in_value(rhs) {
        findings.push(FormeFinding {
            kind: FormeFindingKind::RawColor,
            detail: format!("raw color `{color}` (use `var(--...)` design token)"),
            span,
        });
    }
    if let Some(px) = detect_raw_px_in_value(rhs) {
        findings.push(FormeFinding {
            kind: FormeFindingKind::RawPxUnit,
            detail: format!("magic length `{px}` (use `var(--...)` spacing/size token)"),
            span,
        });
    }
}
