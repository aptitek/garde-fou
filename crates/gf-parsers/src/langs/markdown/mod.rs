//! Specification-grade `CommonMark` / GFM (`pulldown-cmark`) and MDX extractor
//! (`GF063`, `GF064`, `GF000`, `GF080`).

mod frontmatter;
mod mdx_bridge;
pub mod obsidian;
pub mod slugger_links;

use std::collections::{BTreeMap, BTreeSet};

use gf_core::{ContenusDocument, Language, MarkdownViolation, RepoPath, Span, UastDocument};
use pulldown_cmark::{BrokenLink, Event, LinkType, Options, Parser, Tag, TagEnd};

use crate::cst::line_metrics;
use crate::types::{LanguageExtractor, ParseError};
use frontmatter::{
    ActiveHeading, ActiveImage, ActiveLink, FrontmatterSplit, LineIndex, heading_level_u8,
    split_and_validate_frontmatter,
};
use mdx_bridge::{inspect_html_or_mdx_tag, scan_mdx_esm_and_expressions};
use slugger_links::{MdLinkAcc, github_slugify, push_md_issue, scan_reversed_links_outside_code};

const MD_PARSE_OPTS: Options = Options::ENABLE_TABLES
    .union(Options::ENABLE_STRIKETHROUGH)
    .union(Options::ENABLE_TASKLISTS)
    .union(Options::ENABLE_GFM);

/// Specification-grade `CommonMark` / GFM (`.md`) and MDX (`.mdx`) extractor.
#[derive(Clone, Copy, Debug, Default)]
pub struct MarkdownExtractor;

#[derive(Default)]
struct MdWalkState {
    is_mdx: bool,
    prev_heading: u8,
    h1_count: u32,
    active_heading: Option<ActiveHeading>,
    active_image: Option<ActiveImage>,
    active_link: Option<ActiveLink>,
    slug_counts: BTreeMap<String, usize>,
    heading_slugs: BTreeSet<String>,
    fenced_ranges: Vec<(usize, usize)>,
    code_ranges: Vec<(usize, usize)>,
    acc: MdLinkAcc,
}

impl MdWalkState {
    fn new(is_mdx: bool) -> Self {
        Self {
            is_mdx,
            ..Self::default()
        }
    }

    fn finish_heading(&mut self) {
        let Some(heading) = self.active_heading.take() else {
            return;
        };
        if heading.level == 1 {
            self.h1_count = self.h1_count.saturating_add(1);
            if self.h1_count > 1 {
                push_md_issue(
                    &mut self.acc.issues,
                    MarkdownViolation::HeadingHierarchy,
                    heading.span,
                    String::from("multiple top-level `# H1` headings in document"),
                );
            }
        }
        if self.prev_heading > 0 && heading.level > self.prev_heading.saturating_add(1) {
            push_md_issue(
                &mut self.acc.issues,
                MarkdownViolation::HeadingHierarchy,
                heading.span,
                format!(
                    "heading level jump `H{}` -> `H{}`",
                    self.prev_heading, heading.level
                ),
            );
        }
        self.prev_heading = heading.level;
        let slug = github_slugify(&heading.text, &mut self.slug_counts);
        if !slug.is_empty() {
            self.heading_slugs.insert(slug);
        }
    }

    fn append_inline_text(&mut self, txt: &str) {
        if let Some(h) = &mut self.active_heading {
            h.text.push_str(txt);
        }
        if let Some(img) = &mut self.active_image {
            img.alt.push_str(txt);
        }
    }

    fn handle_start_tag(&mut self, tag: Tag<'_>, range: std::ops::Range<usize>, span: Span) {
        match tag {
            Tag::Heading { level, .. } => {
                self.active_heading = Some(ActiveHeading::new(heading_level_u8(level), span));
            }
            Tag::CodeBlock(_) => {
                self.fenced_ranges.push((range.start, range.end));
                self.code_ranges.push((range.start, range.end));
            }
            Tag::Image { dest_url, .. } => {
                self.active_image = Some(ActiveImage::new(&dest_url, span));
            }
            Tag::Link {
                link_type,
                dest_url,
                ..
            } => {
                self.active_link = Some(ActiveLink::new(link_type, &dest_url, span));
            }
            _ => {}
        }
    }

    fn handle_end_tag(&mut self, end: TagEnd) {
        match end {
            TagEnd::Heading(_) => self.finish_heading(),
            TagEnd::Image => {
                if let Some(img) = self.active_image.take() {
                    self.acc.inspect_image(&img.alt, &img.url, img.span);
                }
            }
            TagEnd::Link => {
                if let Some(link) = self.active_link.take() {
                    self.acc.inspect_link(link.kind, &link.url, link.span);
                }
            }
            _ => {}
        }
    }

    fn handle_event(
        &mut self,
        ev: Event<'_>,
        range: std::ops::Range<usize>,
        lines: &LineIndex<'_>,
    ) {
        let span = lines.span_at(range.start);
        match ev {
            Event::Start(tag) => self.handle_start_tag(tag, range, span),
            Event::End(end) => self.handle_end_tag(end),
            Event::Text(txt) => self.append_inline_text(&txt),
            Event::Code(code) => {
                self.code_ranges.push((range.start, range.end));
                self.append_inline_text(&code);
            }
            Event::Html(h) | Event::InlineHtml(h) => {
                inspect_html_or_mdx_tag(&h, self.is_mdx, span.start_line(), &mut self.acc.issues);
            }
            _ => {}
        }
    }

    fn finalize_post_walk(
        &mut self,
        fm: &FrontmatterSplit<'_>,
        lines: &LineIndex<'_>,
        broken_refs: Vec<(String, Span)>,
    ) {
        for (ref_id, sp) in broken_refs {
            push_md_issue(
                &mut self.acc.issues,
                MarkdownViolation::UnresolvedRefOrAnchor,
                sp,
                format!("unresolved Markdown reference link `[{ref_id}]`"),
            );
        }
        for (anchor, sp) in &self.acc.local_anchors_used {
            if !self.heading_slugs.contains(anchor) {
                push_md_issue(
                    &mut self.acc.issues,
                    MarkdownViolation::UnresolvedRefOrAnchor,
                    *sp,
                    format!("unresolved local heading anchor `#{anchor}`"),
                );
            }
        }
        scan_reversed_links_outside_code(
            fm.body,
            &self.code_ranges,
            |off| lines.span_at(off),
            &mut self.acc.issues,
        );
        if self.is_mdx {
            scan_mdx_esm_and_expressions(
                fm.body,
                fm.body_start_line,
                &self.fenced_ranges,
                &mut self.acc.issues,
            );
        }
    }
}

impl LanguageExtractor for MarkdownExtractor {
    fn extract(&self, path: &RepoPath, source: &str) -> Result<UastDocument, ParseError> {
        let (total_lines, effective_lines) = line_metrics(source);
        let mut doc = UastDocument::new(
            path.clone(),
            Language::Markdown,
            total_lines,
            effective_lines,
        )?;
        let is_mdx = path
            .extension()
            .is_some_and(|e| e.eq_ignore_ascii_case("mdx"));
        let fm = split_and_validate_frontmatter(source, &mut doc);
        let lines = LineIndex::new(fm.body, fm.body_start_line);
        let mut state = MdWalkState::new(is_mdx);
        let mut broken_refs: Vec<(String, Span)> = Vec::new();
        let mut cb = |broken: BrokenLink<'_>| {
            if matches!(broken.link_type, LinkType::Reference | LinkType::Collapsed) {
                broken_refs.push((
                    String::from(broken.reference.as_ref()),
                    lines.span_at(broken.span.start),
                ));
            }
            None
        };
        for (ev, range) in
            Parser::new_with_broken_link_callback(fm.body, MD_PARSE_OPTS, Some(&mut cb))
                .into_offset_iter()
        {
            state.handle_event(ev, range, &lines);
        }
        state.finalize_post_walk(&fm, &lines, broken_refs);
        let obs_meta = obsidian::extract_obsidian_metadata(
            fm.body,
            &state.code_ranges,
            fm.body_start_line,
            &mut state.acc.asset_refs,
        );
        doc.asset_refs = state.acc.asset_refs;
        doc.academic = obs_meta;
        doc.contenus = Some(ContenusDocument {
            issues: state.acc.issues,
            heading_slugs: state.heading_slugs,
            doc_links: state.acc.doc_links,
        });
        Ok(doc)
    }
}
