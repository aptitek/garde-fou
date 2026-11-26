//! Pure Safe Rust Mermaid v11 & `PlantUML` diagram analyzers + Markdown/Quarto fenced bridge (`GF063`/`GF080`).

pub mod fenced_bridge;
pub mod mermaid;
pub mod plantuml;

use gf_core::{Language, RepoPath, UastDocument};

use crate::cst::line_metrics;
use crate::types::ParseError;

/// Extracts a standalone `.mmd` / `.mermaid` [`UastDocument`].
///
/// # Errors
///
/// Returns [`ParseError`] if constructing the root span fails.
pub fn extract_mermaid(path: &RepoPath, source: &str) -> Result<UastDocument, ParseError> {
    let (total_lines, effective_lines) = line_metrics(source);
    let mut doc = UastDocument::new(
        path.clone(),
        Language::Mermaid,
        total_lines,
        effective_lines,
    )?;
    mermaid::analyze_mermaid_block(source, 1, &mut doc.lang_rule_findings);
    Ok(doc)
}

/// Extracts a standalone `.puml` / `.plantuml` / `.iuml` / `.pu` [`UastDocument`].
///
/// # Errors
///
/// Returns [`ParseError`] if constructing the root span fails.
pub fn extract_plantuml(path: &RepoPath, source: &str) -> Result<UastDocument, ParseError> {
    let (total_lines, effective_lines) = line_metrics(source);
    let mut doc = UastDocument::new(
        path.clone(),
        Language::PlantUml,
        total_lines,
        effective_lines,
    )?;
    plantuml::analyze_plantuml_block(source, 1, &mut doc.lang_rule_findings);
    Ok(doc)
}
