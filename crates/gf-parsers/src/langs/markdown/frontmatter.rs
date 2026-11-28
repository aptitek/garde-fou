//! YAML frontmatter (`--- ... ---`) extractor and `saphyr-parser` bridge for Markdown/MDX
//! (`GF000` & `GF080` with exact 1-based line remapping).

use gf_core::{Span, SyntaxErrorNode, UastDocument, YamlFlavor};
use pulldown_cmark::{HeadingLevel, LinkType};

pub(super) struct ActiveHeading {
    pub level: u8,
    pub span: Span,
    pub text: String,
}

impl ActiveHeading {
    pub(super) const fn new(level: u8, span: Span) -> Self {
        Self {
            level,
            span,
            text: String::new(),
        }
    }
}

pub(super) struct ActiveImage {
    pub url: String,
    pub span: Span,
    pub alt: String,
}

impl ActiveImage {
    pub(super) fn new(url: &str, span: Span) -> Self {
        Self {
            url: String::from(url),
            span,
            alt: String::new(),
        }
    }
}

pub(super) struct ActiveLink {
    pub kind: LinkType,
    pub url: String,
    pub span: Span,
}

impl ActiveLink {
    pub(super) fn new(kind: LinkType, url: &str, span: Span) -> Self {
        Self {
            kind,
            url: String::from(url),
            span,
        }
    }
}

use crate::langs::yaml::{
    anchors_merge::resolve_yaml_document, events_tree::parse_yaml_stream,
    yaml_quirks::check_yaml_scalar_quirks,
};

/// Maps a `pulldown-cmark` [`HeadingLevel`] to `1..=6`.
#[must_use]
pub const fn heading_level_u8(lvl: HeadingLevel) -> u8 {
    match lvl {
        HeadingLevel::H1 => 1,
        HeadingLevel::H2 => 2,
        HeadingLevel::H3 => 3,
        HeadingLevel::H4 => 4,
        HeadingLevel::H5 => 5,
        HeadingLevel::H6 => 6,
    }
}

/// Result of splitting an optional leading YAML frontmatter block from `source`.
pub struct FrontmatterSplit<'a> {
    /// Markdown body slice after the closing `---` / `...` delimiter.
    pub body: &'a str,
    /// 1-based line number in `source` where `body` starts (`1` if no frontmatter).
    pub body_start_line: u32,
}

fn remap_span(inner: Span, line_delta: u32) -> Span {
    let s = inner.start_line().saturating_add(line_delta);
    let e = inner.end_line().saturating_add(line_delta).max(s);
    Span::new(s, e).unwrap_or(Span::FIRST)
}

fn validate_frontmatter_yaml(yaml_text: &str, line_delta: u32, doc: &mut UastDocument) {
    let stream = parse_yaml_stream(yaml_text);
    for mut syn in stream.syntax_errors {
        syn.span = remap_span(syn.span, line_delta);
        doc.syntax_errors.push(syn);
    }
    let mut findings = Vec::new();
    for raw_doc in &stream.docs {
        check_yaml_scalar_quirks(raw_doc, YamlFlavor::GenericYaml, &mut findings);
        resolve_yaml_document(raw_doc, &mut findings);
    }
    for mut f in findings {
        f.span = remap_span(f.span, line_delta);
        doc.lang_rule_findings.push(f);
    }
}

/// Splits and validates optional leading YAML frontmatter (`---` on line 1) from `source`.
pub fn split_and_validate_frontmatter<'a>(
    source: &'a str,
    doc: &mut UastDocument,
) -> FrontmatterSplit<'a> {
    let trimmed_bom = source.strip_prefix('\u{feff}').unwrap_or(source);
    let Some(after_open) = trimmed_bom
        .strip_prefix("---\r\n")
        .or_else(|| trimmed_bom.strip_prefix("---\n"))
    else {
        return FrontmatterSplit {
            body: source,
            body_start_line: 1,
        };
    };

    let mut byte_cursor = 0usize;
    let mut line_count = 1u32;
    for raw_line in after_open.split_inclusive('\n') {
        line_count = line_count.saturating_add(1);
        let clean = raw_line.trim_end_matches(['\r', '\n']);
        if clean == "---" || clean == "..." {
            let yaml_slice = &after_open[..byte_cursor];
            validate_frontmatter_yaml(yaml_slice, 1, doc);
            let body = &after_open[byte_cursor + raw_line.len()..];
            return FrontmatterSplit {
                body,
                body_start_line: line_count.saturating_add(1),
            };
        }
        byte_cursor += raw_line.len();
    }

    doc.syntax_errors.push(SyntaxErrorNode {
        span: Span::FIRST,
        message: String::from("unclosed YAML frontmatter `---` block in Markdown document"),
    });
    FrontmatterSplit {
        body: "",
        body_start_line: line_count.saturating_add(1),
    }
}

/// Maps byte offsets inside `body` to 1-based [`Span`]s relative to `base_line`.
pub struct LineIndex<'a> {
    body: &'a str,
    base_line: u32,
}

impl<'a> LineIndex<'a> {
    /// Builds a [`LineIndex`] for `body` starting at 1-based `base_line`.
    #[must_use]
    pub const fn new(body: &'a str, base_line: u32) -> Self {
        Self { body, base_line }
    }

    /// Maps a byte offset inside `body` to its 1-based [`Span`].
    #[must_use]
    pub fn span_at(&self, byte_offset: usize) -> Span {
        let prefix = self.body.get(..byte_offset).unwrap_or(self.body);
        let nl = u32::try_from(prefix.bytes().filter(|&b| b == b'\n').count()).unwrap_or(u32::MAX);
        let l = self.base_line.saturating_add(nl);
        Span::new(l, l).unwrap_or(Span::FIRST)
    }
}
