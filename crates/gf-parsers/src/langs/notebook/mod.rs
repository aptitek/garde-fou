//! Native Jupyter Notebook (`.ipynb`, `nbformat` v4) extractor with `IPython` magic
//! neutralization and multi-AST (`Python`, `Shell`, `SQL`, `Markdown`) cell bridging.

pub mod ipynb_bridge;
pub mod ipynb_parser;

use gf_core::{AcademicSummary, Language, RepoPath, SqlDialectKind, UastDocument};

use crate::cst::line_metrics;
use crate::types::{LanguageExtractor, ParseError};
use ipynb_bridge::bridge_notebook_cells;
use ipynb_parser::{audit_notebook_cells_hygiene, parse_ipynb_cells};

/// Extractor for Jupyter Notebook (`.ipynb`) files.
#[derive(Clone, Copy, Debug, Default)]
pub struct NotebookExtractor;

impl NotebookExtractor {
    /// Extracts a Jupyter `.ipynb` [`UastDocument`] with an optional workspace SQL dialect hint.
    ///
    /// # Errors
    ///
    /// Returns [`ParseError`] if inner Tree-sitter grammar initialization or span construction fails.
    pub fn extract_with_sql_dialect(
        path: &RepoPath,
        source: &str,
        sql_dialect: Option<SqlDialectKind>,
    ) -> Result<UastDocument, ParseError> {
        let (total_lines, effective_lines) = line_metrics(source);
        let mut doc = UastDocument::new(
            path.clone(),
            Language::Notebook,
            total_lines,
            effective_lines,
        )?;
        let Some(cells) = parse_ipynb_cells(source, &mut doc.syntax_errors) else {
            return Ok(doc);
        };
        let mut output_spans = Vec::new();
        audit_notebook_cells_hygiene(path, &cells, &mut doc.lang_rule_findings, &mut output_spans);

        let cell_lines = bridge_notebook_cells(path, &cells, sql_dialect, &mut doc)?;

        doc.academic = Some(AcademicSummary {
            notebook_cell_lines: cell_lines,
            notebook_output_spans: output_spans,
            ..AcademicSummary::default()
        });
        Ok(doc)
    }
}

impl LanguageExtractor for NotebookExtractor {
    fn extract(&self, path: &RepoPath, source: &str) -> Result<UastDocument, ParseError> {
        Self::extract_with_sql_dialect(path, source, None)
    }
}
