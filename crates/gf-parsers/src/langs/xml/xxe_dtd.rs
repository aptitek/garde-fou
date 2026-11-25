//! Pre-parse XML DTD/XXE inspection (`XML-FORBID-DTD-XXE`) and `roxmltree` DOM wrapper
//! with 32,768-node Billion-Laughs cap and 1-based line/column `GF000` diagnostics.

use gf_core::{Span, SyntaxErrorNode};
use roxmltree::{Document, Error as XmlError, ParsingOptions};

/// Maximum DOM nodes permitted in a single XML/SVG document (`roxmltree` `DoS` guard).
const MAX_XML_NODES: u32 = 32_768;

pub struct ParsedXmlOutcome<'a> {
    pub dom: Option<Document<'a>>,
    pub dtd_xxe_hits: Vec<(u32, String)>,
    pub syntax_errors: Vec<SyntaxErrorNode>,
}

fn line_at_byte(source: &str, byte_idx: usize) -> u32 {
    let prefix = source.get(..byte_idx).unwrap_or(source);
    let newlines = prefix.bytes().filter(|&b| b == b'\n').count();
    u32::try_from(newlines)
        .unwrap_or(u32::MAX)
        .saturating_add(1)
}

fn strip_xml_comments_preserving_newlines(source: &str) -> String {
    let mut chunks = source.split("<!--");
    let Some(first) = chunks.next() else {
        return String::new();
    };
    let mut out = String::from(first);
    for chunk in chunks {
        out.push_str("    ");
        if let Some((comment_body, after_close)) = chunk.split_once("-->") {
            for b in comment_body.bytes() {
                out.push(if b == b'\n' { '\n' } else { ' ' });
            }
            out.push_str("   ");
            out.push_str(after_close);
        } else {
            break;
        }
    }
    out
}

fn contains_ascii_word(haystack_upper: &str, word: &str) -> bool {
    haystack_upper
        .split(|c: char| !c.is_ascii_alphanumeric())
        .any(|tok| tok == word)
}

fn scan_prologue_dtd_xxe(source: &str, out: &mut Vec<(u32, String)>) {
    let masked = strip_xml_comments_preserving_newlines(source);
    let upper = masked.to_ascii_uppercase();

    for (idx, _) in upper.match_indices("<!ENTITY") {
        out.push((
            line_at_byte(&masked, idx),
            String::from("`<!ENTITY>` declaration forbidden (XXE / Billion-Laughs risk)"),
        ));
    }

    for (idx, _) in upper.match_indices("<!DOCTYPE") {
        let line = line_at_byte(&masked, idx);
        let after = upper
            .get(idx..)
            .and_then(|s| s.strip_prefix("<!DOCTYPE"))
            .unwrap_or("");
        let decl = after.split('>').next().unwrap_or(after);
        if decl.contains('[') {
            out.push((
                line,
                String::from("`<!DOCTYPE ... [...]>` internal DTD subset is forbidden"),
            ));
        } else if contains_ascii_word(decl, "SYSTEM") || contains_ascii_word(decl, "PUBLIC") {
            out.push((
                line,
                String::from("external `SYSTEM`/`PUBLIC` DTD reference is forbidden (XXE)"),
            ));
        }
    }
}

pub fn parse_xml_checked(source: &str) -> ParsedXmlOutcome<'_> {
    let mut dtd_xxe_hits = Vec::new();
    let mut syntax_errors = Vec::new();
    scan_prologue_dtd_xxe(source, &mut dtd_xxe_hits);

    let opts = ParsingOptions {
        allow_dtd: true,
        nodes_limit: MAX_XML_NODES,
        ..ParsingOptions::default()
    };
    let dom = match Document::parse_with_options(source, opts) {
        Ok(doc) => Some(doc),
        Err(XmlError::NodesLimitReached) => {
            dtd_xxe_hits.push((
                1,
                format!("XML node count exceeds maximum limit ({MAX_XML_NODES})"),
            ));
            None
        }
        Err(err) => {
            let pos = err.pos();
            let line = pos.row.max(1);
            syntax_errors.push(SyntaxErrorNode {
                span: Span::new(line, line).unwrap_or(Span::FIRST),
                message: format!("col {}: {err}", pos.col),
            });
            None
        }
    };
    ParsedXmlOutcome {
        dom,
        dtd_xxe_hits,
        syntax_errors,
    }
}
