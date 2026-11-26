//! Specification-grade `.astro` frontmatter (`--- ... ---`), template, `<script>`,
//! and `<style>` splitter preserving exact 1-based line numbers across all zones.

use gf_core::Span;

use crate::langs::html_w3c::parse_raw_tag_attrs;
use crate::langs::toml_lang::span_from_byte_offset;

/// Embedded `<script>` or `<style>` block extracted from an `.astro` template.
#[derive(Clone, Debug)]
pub(super) struct AstroEmbeddedBlock {
    /// 1-based span of the opening `<script>` or `<style>` tag.
    pub open_tag_span: Span,
    /// Raw attribute slice inside `<script ...>` or `<style ...>`.
    pub attrs_raw: String,
    /// Inner source prefixed with `\n` padding so line 1..N matches the `.astro` file.
    pub line_aligned_body: String,
}

/// Line-aligned zones extracted from an `.astro` file.
#[derive(Clone, Debug)]
pub(super) struct AstroParts {
    /// Optional server TypeScript frontmatter (`--- ... ---`), line-aligned with `\n` padding.
    pub frontmatter_line_aligned: Option<String>,
    /// Template HTML/JSX body with frontmatter replaced by `\n` padding.
    pub template_line_aligned: String,
    /// Extracted `<script>` blocks.
    pub scripts: Vec<AstroEmbeddedBlock>,
    /// Extracted `<style>` blocks.
    pub styles: Vec<AstroEmbeddedBlock>,
}

fn blank_prefix_preserving_newlines(slice: &str) -> String {
    let newlines = slice.bytes().filter(|&b| b == b'\n').count();
    "\n".repeat(newlines)
}

#[derive(Clone, Copy, Eq, PartialEq)]
enum FmScanMode {
    Normal,
    BlockComment,
    Backtick,
}

fn step_normal_char(
    c: char,
    chars: &mut std::iter::Peekable<std::str::Chars<'_>>,
    mode: &mut FmScanMode,
) -> bool {
    if c == '/' && chars.peek() == Some(&'/') {
        return true;
    }
    if c == '/' && chars.peek() == Some(&'*') {
        chars.next();
        *mode = FmScanMode::BlockComment;
    } else if c == '`' {
        *mode = FmScanMode::Backtick;
    }
    false
}

fn step_mode_char(
    c: char,
    chars: &mut std::iter::Peekable<std::str::Chars<'_>>,
    mode: &mut FmScanMode,
) -> bool {
    match *mode {
        FmScanMode::BlockComment => {
            if c == '*' && chars.peek() == Some(&'/') {
                chars.next();
                *mode = FmScanMode::Normal;
            }
            false
        }
        FmScanMode::Backtick => {
            if c == '\\' {
                chars.next();
            } else if c == '`' {
                *mode = FmScanMode::Normal;
            }
            false
        }
        FmScanMode::Normal => step_normal_char(c, chars, mode),
    }
}

fn update_multiline_state(line: &str, mode: &mut FmScanMode) {
    let mut chars = line.chars().peekable();
    while let Some(c) = chars.next() {
        if step_mode_char(c, &mut chars, mode) {
            break;
        }
    }
}

fn split_frontmatter(source: &str) -> (Option<String>, String) {
    let lines: Vec<&str> = source.split_inclusive('\n').collect();
    let Some(open_idx) = lines.iter().position(|l| !l.trim().is_empty()) else {
        return (None, String::from(source));
    };
    if lines[open_idx].trim() != "---" {
        return (None, String::from(source));
    }
    let mut mode = FmScanMode::Normal;
    let mut close_idx = None;
    for (idx, &line) in lines.iter().enumerate().skip(open_idx + 1) {
        if mode == FmScanMode::Normal && line.trim() == "---" {
            close_idx = Some(idx);
            break;
        }
        update_multiline_state(line, &mut mode);
    }
    let Some(close_idx) = close_idx else {
        return (None, String::from(source));
    };
    let fm_pad = "\n".repeat(open_idx + 1);
    let fm_body: String = lines[open_idx + 1..close_idx].concat();
    let tpl_pad = "\n".repeat(close_idx + 1);
    let tpl_body: String = lines[close_idx + 1..].concat();
    (
        Some(format!("{fm_pad}{fm_body}")),
        format!("{tpl_pad}{tpl_body}"),
    )
}

fn extract_tag_blocks(template: &str, tag_name: &str) -> Vec<AstroEmbeddedBlock> {
    let mut out = Vec::new();
    let close_pat = format!("</{tag_name}>");
    let mut skip_until = 0usize;
    for (open_idx, _) in template.match_indices('<') {
        if open_idx < skip_until {
            continue;
        }
        let after_lt = &template[open_idx + 1..];
        let Some(after_tag) = after_lt.strip_prefix(tag_name) else {
            continue;
        };
        if !after_tag.starts_with(|c: char| c.is_ascii_whitespace() || c == '>') {
            continue;
        }
        let Some((raw_attrs, after_open_gt)) = after_tag.split_once('>') else {
            break;
        };
        let attrs_trimmed = raw_attrs.trim();
        if attrs_trimmed.ends_with('/') {
            continue;
        }
        let Some((body, after_close)) = after_open_gt.split_once(&close_pat) else {
            break;
        };
        let body_start = template.len() - after_open_gt.len();
        let pad = blank_prefix_preserving_newlines(&template[..body_start]);
        out.push(AstroEmbeddedBlock {
            open_tag_span: span_from_byte_offset(template, open_idx),
            attrs_raw: String::from(attrs_trimmed),
            line_aligned_body: format!("{pad}{body}"),
        });
        skip_until = template.len() - after_close.len();
    }
    out
}

/// Splits an `.astro` document into line-aligned frontmatter, template, `<script>`, and `<style>` blocks.
#[must_use]
pub(super) fn split_astro_document(source: &str) -> AstroParts {
    let (frontmatter_line_aligned, template_line_aligned) = split_frontmatter(source);
    let scripts = extract_tag_blocks(&template_line_aligned, "script");
    let styles = extract_tag_blocks(&template_line_aligned, "style");
    AstroParts {
        frontmatter_line_aligned,
        template_line_aligned,
        scripts,
        styles,
    }
}

/// Returns `true` if `attrs_raw` on `<script ...>` contains `is:inline`.
#[must_use]
pub(super) fn script_has_is_inline(attrs_raw: &str) -> bool {
    parse_raw_tag_attrs(attrs_raw)
        .iter()
        .any(|a| a.name.eq_ignore_ascii_case("is:inline"))
}
