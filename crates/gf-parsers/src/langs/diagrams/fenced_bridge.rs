//! Fenced diagram bridge extracting ```` ```mermaid ```` / ```` ```{mermaid} ```` / ```` ```plantuml ````
//! blocks from Markdown, MDX, Quarto `.qmd`, and Jupyter `.ipynb` cells with exact 1-based line offsets.

use gf_core::{LangRuleFinding, RepoPath, SqlDialectKind, UastDocument};

use super::{mermaid::analyze_mermaid_block, plantuml::analyze_plantuml_block};
use crate::langs::{markdown::MarkdownExtractor, quarto::QuartoExtractor};
use crate::types::{LanguageExtractor, ParseError};

#[derive(Clone, Copy)]
enum FencedKind {
    Mermaid,
    PlantUml,
}

fn classify_fence_info(info: &str) -> Option<FencedKind> {
    let tok = info.trim().to_ascii_lowercase();
    if tok.starts_with("{{") {
        return None;
    }
    let clean = tok.trim_start_matches('{').trim_end_matches('}');
    match clean.split_whitespace().next().unwrap_or("") {
        "mermaid" => Some(FencedKind::Mermaid),
        "plantuml" | "puml" => Some(FencedKind::PlantUml),
        _ => None,
    }
}

fn dispatch_fenced(kind: FencedKind, buf: &str, body_start: u32, out: &mut Vec<LangRuleFinding>) {
    match kind {
        FencedKind::Mermaid => analyze_mermaid_block(buf, body_start, out),
        FencedKind::PlantUml => analyze_plantuml_block(buf, body_start, out),
    }
}

struct ActiveFence {
    kind: FencedKind,
    fence_len: usize,
    body_start: u32,
    buf: String,
}

fn scan_fenced_diagrams(source: &str, base_line: u32, out: &mut Vec<LangRuleFinding>) {
    let mut active: Option<ActiveFence> = None;
    for (idx, line) in source.lines().enumerate() {
        let ln = base_line.saturating_add(u32::try_from(idx).unwrap_or(0));
        let trimmed = line.trim_start();
        let ticks = trimmed.bytes().take_while(|&b| b == b'`').count();
        if let Some(ref mut fence) = active {
            if ticks >= fence.fence_len && trimmed[ticks..].trim().is_empty() {
                dispatch_fenced(fence.kind, &fence.buf, fence.body_start, out);
                active = None;
                continue;
            }
            fence.buf.push_str(line);
            fence.buf.push('\n');
            continue;
        }
        if ticks >= 3
            && let Some(kind) = classify_fence_info(&trimmed[ticks..])
        {
            active = Some(ActiveFence {
                kind,
                fence_len: ticks,
                body_start: ln.saturating_add(1),
                buf: String::new(),
            });
        }
    }
}

/// Extracts Markdown/MDX and validates embedded fenced Mermaid/`PlantUML` blocks.
///
/// # Errors
///
/// Returns [`ParseError`] if Markdown extraction fails.
pub fn extract_markdown_with_diagrams(
    path: &RepoPath,
    source: &str,
) -> Result<UastDocument, ParseError> {
    let mut doc = MarkdownExtractor.extract(path, source)?;
    scan_fenced_diagrams(source, 1, &mut doc.lang_rule_findings);
    Ok(doc)
}

/// Extracts Quarto `.qmd` and validates embedded fenced Mermaid/`PlantUML` blocks.
///
/// # Errors
///
/// Returns [`ParseError`] if Quarto extraction fails.
pub fn extract_quarto_with_diagrams(
    path: &RepoPath,
    source: &str,
    sql_hint: Option<SqlDialectKind>,
) -> Result<UastDocument, ParseError> {
    let mut doc = QuartoExtractor::extract_with_sql_dialect(path, source, sql_hint)?;
    scan_fenced_diagrams(source, 1, &mut doc.lang_rule_findings);
    Ok(doc)
}
