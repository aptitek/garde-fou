//! GitHub-compatible heading anchor slugger (`rumdl` / `github-slugger`) and Markdown
//! link, image, reference, and reversed-link validators (`GF063` / `GF064`).

use std::collections::{BTreeMap, BTreeSet};

use gf_core::{ContenusIssue, ContenusIssueKind, MarkdownLinkTarget, MarkdownViolation, Span};
use pulldown_cmark::LinkType;

pub(super) fn push_md_issue(
    issues: &mut Vec<ContenusIssue>,
    v: MarkdownViolation,
    span: Span,
    detail: String,
) {
    issues.push(ContenusIssue {
        kind: ContenusIssueKind::MarkdownSpec(v),
        detail,
        span,
    });
}

/// Computes the GitHub-compatible anchor slug for `heading_text`, deduplicating repeats with `-1`, `-2`.
#[must_use]
pub fn github_slugify(heading_text: &str, seen_counts: &mut BTreeMap<String, usize>) -> String {
    let mut base = String::with_capacity(heading_text.len());
    for ch in heading_text.chars() {
        if ch == ' ' {
            base.push('-');
        } else if ch.is_alphanumeric() || ch == '_' || ch == '-' {
            for lc in ch.to_lowercase() {
                base.push(lc);
            }
        }
    }
    let count = seen_counts.entry(base.clone()).or_insert(0);
    let slug = if *count == 0 {
        base
    } else {
        format!("{base}-{count}")
    };
    *count += 1;
    slug
}

const EXTERNAL_URL_PREFIXES: &[&str] = &["http://", "https://", "mailto:", "tel:", "file://", "//"];

fn is_external_or_scheme_url(url: &str) -> bool {
    let low = url.to_ascii_lowercase();
    EXTERNAL_URL_PREFIXES.iter().any(|p| low.starts_with(p))
}

fn has_markdown_extension(path_part: &str) -> bool {
    path_part
        .rsplit_once('.')
        .is_some_and(|(_, ext)| matches!(ext.to_ascii_lowercase().as_str(), "md" | "mdx"))
}

/// Shared link/image/anchor accumulator for a single Markdown document.
#[derive(Default)]
pub(super) struct MdLinkAcc {
    pub local_anchors_used: Vec<(String, Span)>,
    pub doc_links: Vec<MarkdownLinkTarget>,
    pub asset_refs: BTreeSet<String>,
    pub issues: Vec<ContenusIssue>,
}

impl MdLinkAcc {
    /// Validates a finished `[label](dest_url)` link (`Autolink`/`Email` are always exempt).
    pub fn inspect_link(&mut self, link_type: LinkType, dest_url: &str, span: Span) {
        if matches!(link_type, LinkType::Autolink | LinkType::Email) {
            return;
        }
        let trimmed = dest_url.trim();
        if trimmed.is_empty() || trimmed == "#" {
            push_md_issue(
                &mut self.issues,
                MarkdownViolation::EmptyOrReversedLink,
                span,
                format!("empty Markdown link destination `({trimmed})`"),
            );
            return;
        }
        if trimmed.to_ascii_lowercase().starts_with("data:") {
            self.issues.push(ContenusIssue {
                kind: ContenusIssueKind::DataUri,
                detail: String::from("embedded `data:` URI in Markdown link"),
                span,
            });
            return;
        }
        if let Some(frag) = trimmed.strip_prefix('#') {
            self.local_anchors_used
                .push((frag.to_ascii_lowercase(), span));
            return;
        }
        if is_external_or_scheme_url(trimmed) {
            return;
        }
        let without_query = trimmed.split('?').next().unwrap_or(trimmed);
        let (file_part, frag_opt) = match without_query.split_once('#') {
            Some((p, f)) => (p, (!f.is_empty()).then(|| f.to_ascii_lowercase())),
            None => (without_query, None),
        };
        if has_markdown_extension(file_part) {
            self.doc_links.push(MarkdownLinkTarget {
                target_path: String::from(file_part),
                fragment: frag_opt,
                span,
            });
        }
    }

    /// Validates a finished `![alt](dest_url)` Markdown image and records local `asset_refs`.
    pub fn inspect_image(&mut self, alt_text: &str, dest_url: &str, span: Span) {
        let trimmed = dest_url.trim();
        if trimmed.is_empty() {
            push_md_issue(
                &mut self.issues,
                MarkdownViolation::EmptyOrReversedLink,
                span,
                String::from("Markdown image has empty source `![]( )`"),
            );
        }
        if alt_text.trim().is_empty() {
            push_md_issue(
                &mut self.issues,
                MarkdownViolation::EmptyOrReversedLink,
                span,
                format!("Markdown image `![]({trimmed})` is missing non-empty `alt` text"),
            );
        }
        if trimmed.to_ascii_lowercase().starts_with("data:") {
            self.issues.push(ContenusIssue {
                kind: ContenusIssueKind::DataUri,
                detail: String::from("embedded `data:` URI in Markdown image"),
                span,
            });
        } else if !trimmed.is_empty()
            && !is_external_or_scheme_url(trimmed)
            && !trimmed.starts_with('#')
        {
            let clean = trimmed.split(['?', '#']).next().unwrap_or(trimmed);
            if !clean.is_empty() && !has_markdown_extension(clean) {
                self.asset_refs.insert(String::from(clean));
            }
        }
    }
}

const REVERSED_TARGET_PREFIXES: &[&str] = &["http://", "https://", "./", "../", "/", "#"];

fn extract_reversed_target(
    body: &str,
    mid: usize,
    in_code: impl Fn(usize) -> bool,
) -> Option<(usize, &str)> {
    if in_code(mid) {
        return None;
    }
    let line_start = body[..mid].rfind('\n').map_or(0, |nl| nl.saturating_add(1));
    let before = &body[line_start..mid];
    let open_paren = before.rfind('(')?;
    let paren_off = line_start + open_paren;
    let label = before.get(open_paren..)?.strip_prefix('(')?.trim();
    if in_code(paren_off) || label.is_empty() {
        return None;
    }
    let after = body.get(mid..)?.strip_prefix(")[")?;
    let after_line = after.lines().next().unwrap_or("");
    let (raw_target, _) = after_line.split_once(']')?;
    let target = raw_target.trim();
    REVERSED_TARGET_PREFIXES
        .iter()
        .any(|p| target.starts_with(p))
        .then_some((paren_off, target))
}

/// Detects reversed `(label)[https://...]` link syntax in `body` outside fenced/inline code ranges.
pub(super) fn scan_reversed_links_outside_code(
    body: &str,
    code_ranges: &[(usize, usize)],
    span_at: impl Fn(usize) -> Span,
    issues: &mut Vec<ContenusIssue>,
) {
    let in_code = |off: usize| code_ranges.iter().any(|&(s, e)| (s..e).contains(&off));
    for (mid, _) in body.match_indices(")[") {
        if let Some((paren_off, target)) = extract_reversed_target(body, mid, in_code) {
            push_md_issue(
                issues,
                MarkdownViolation::EmptyOrReversedLink,
                span_at(paren_off),
                format!(
                    "reversed Markdown link syntax `(...)[{target}]`; write `[text]({target})`"
                ),
            );
        }
    }
}
