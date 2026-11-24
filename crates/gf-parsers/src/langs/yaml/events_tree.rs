//! Event-driven `saphyr-parser` 0.4 CST/AST stream builder.

use gf_core::{Span, SyntaxErrorNode};
use saphyr_parser::{Event, Marker, Parser, Span as SaphyrSpan};

pub use super::yaml_ast::{ParsedYamlStream, RawYamlStreamDoc, YamlNode, YamlScalar, YamlStyle};

fn to_u32_pos(val: usize) -> u32 {
    u32::try_from(val).unwrap_or(u32::MAX).max(1)
}

fn marker_to_span(marker: &Marker) -> Span {
    let line = to_u32_pos(marker.line());
    Span::new(line, line).unwrap_or(Span::FIRST)
}

fn saphyr_span_to_gf(sp: SaphyrSpan) -> Span {
    let sl = to_u32_pos(sp.start.line());
    let end_line = sp.end.line().saturating_sub(usize::from(sp.end.col() == 0));
    let el = to_u32_pos(end_line).max(sl);
    Span::new(sl, el).unwrap_or(Span::FIRST)
}

fn merge_container_span(start: Span, end: Span) -> Span {
    let sl = start.start_line();
    let el = end.end_line().max(sl);
    Span::new(sl, el).unwrap_or(start)
}

enum Frame {
    Seq {
        anchor: usize,
        start_span: Span,
        items: Vec<YamlNode>,
    },
    Map {
        anchor: usize,
        start_span: Span,
        entries: Vec<(YamlNode, YamlNode)>,
        pending_key: Option<YamlNode>,
    },
}

#[derive(Default)]
struct StreamBuilder {
    stack: Vec<Frame>,
    cur_doc: RawYamlStreamDoc,
    docs: Vec<RawYamlStreamDoc>,
}

impl StreamBuilder {
    fn flush_doc(&mut self) {
        if self.cur_doc.root.is_some() {
            self.docs.push(std::mem::take(&mut self.cur_doc));
        }
    }

    fn push_node(&mut self, node: YamlNode, anchor: usize) {
        if anchor > 0 {
            self.cur_doc.anchors.insert(anchor, node.clone());
        }
        match self.stack.last_mut() {
            Some(Frame::Seq { items, .. }) => items.push(node),
            Some(Frame::Map {
                entries,
                pending_key,
                ..
            }) => {
                if let Some(k) = pending_key.take() {
                    entries.push((k, node));
                } else {
                    *pending_key = Some(node);
                }
            }
            None => self.cur_doc.root = Some(node),
        }
    }

    fn finish_top_container(&mut self, end_sp: Span) {
        match self.stack.pop() {
            Some(Frame::Seq {
                anchor,
                start_span,
                items,
            }) => {
                let span = merge_container_span(start_span, end_sp);
                self.push_node(YamlNode::Sequence { items, span }, anchor);
            }
            Some(Frame::Map {
                anchor,
                start_span,
                entries,
                ..
            }) => {
                let span = merge_container_span(start_span, end_sp);
                self.push_node(YamlNode::Mapping { entries, span }, anchor);
            }
            None => {}
        }
    }

    fn handle_event(&mut self, ev: Event<'_>, sp: Span) {
        match ev {
            Event::DocumentStart(_) | Event::DocumentEnd => self.flush_doc(),
            Event::SequenceStart(anchor, _) => self.stack.push(Frame::Seq {
                anchor,
                start_span: sp,
                items: Vec::new(),
            }),
            Event::MappingStart(anchor, _) => self.stack.push(Frame::Map {
                anchor,
                start_span: sp,
                entries: Vec::new(),
                pending_key: None,
            }),
            Event::SequenceEnd | Event::MappingEnd => self.finish_top_container(sp),
            Event::Scalar(val, raw_style, anchor, tag) => {
                let style = YamlStyle::from_saphyr(raw_style);
                if style.is_block() && sp.end_line() > sp.start_line() {
                    self.cur_doc
                        .block_body_ranges
                        .push((sp.start_line() + 1, sp.end_line()));
                }
                let node = YamlNode::Scalar(YamlScalar {
                    value: val.into_owned(),
                    style,
                    has_tag: tag.is_some(),
                    span: sp,
                });
                self.push_node(node, anchor);
            }
            Event::Alias(target_id) => {
                self.push_node(
                    YamlNode::Alias {
                        target_id,
                        span: sp,
                    },
                    0,
                );
            }
            Event::Nothing | Event::StreamStart | Event::StreamEnd => {}
        }
    }
}

/// Parses a YAML 1.2 source string via `saphyr-parser` into [`ParsedYamlStream`].
#[must_use]
pub fn parse_yaml_stream(source: &str) -> ParsedYamlStream {
    let mut builder = StreamBuilder::default();
    let mut syntax_errors = Vec::new();
    for item in Parser::new_from_str(source) {
        match item {
            Ok((ev, raw_sp)) => builder.handle_event(ev, saphyr_span_to_gf(raw_sp)),
            Err(err) => {
                let m = err.marker();
                syntax_errors.push(SyntaxErrorNode {
                    span: marker_to_span(m),
                    message: format!("col {}: {}", m.col().saturating_add(1), err.info()),
                });
                break;
            }
        }
    }
    builder.flush_doc();
    ParsedYamlStream {
        docs: builder.docs,
        syntax_errors,
    }
}
