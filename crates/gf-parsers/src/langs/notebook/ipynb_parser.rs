//! `nbformat` v4 JSON parser with exact per-cell JSON line offsets, execution-order
//! verification, committed error-traceback detection, and secret scanning in `outputs[]`.

use gf_core::{LangRuleFinding, NotebookViolation, RepoPath, Span, SyntaxErrorNode};
use serde::Deserialize;
use serde_json::Value;

use crate::secrets::extract_secret_findings;

/// `nbformat` v4 string or array-of-lines field (`source`, `text`, `data["text/plain"]`, etc.).
#[derive(Clone, Debug, Deserialize)]
#[serde(untagged)]
enum SourceValue {
    /// Single concatenated string.
    Single(String),
    /// Canonical list of lines (usually with trailing `\n`).
    Lines(Vec<String>),
}

impl SourceValue {
    fn into_joined(self) -> String {
        match self {
            Self::Single(s) => s,
            Self::Lines(lines) => lines.concat(),
        }
    }
}

/// Deserialized `nbformat` v4 cell.
#[derive(Clone, Debug, Deserialize)]
struct RawCell {
    /// `"code"`, `"markdown"`, or `"raw"`.
    cell_type: String,
    /// Cell text payload (`String` or `Vec<String>`).
    #[serde(default = "default_source")]
    source: SourceValue,
    /// Optional execution counter for `"code"` cells.
    #[serde(default)]
    execution_count: Option<i64>,
    /// Output objects for `"code"` cells.
    #[serde(default)]
    outputs: Vec<Value>,
}

const fn default_source() -> SourceValue {
    SourceValue::Single(String::new())
}

/// Top-level `nbformat` v4 JSON structure.
#[derive(Clone, Debug, Deserialize)]
struct RawNotebook {
    /// Notebook cells array.
    cells: Vec<RawCell>,
    /// Major nbformat version (must be `>= 4`).
    nbformat: i64,
}

/// Parsed cell with 1-based index and approximate JSON file start line.
pub struct ParsedCell {
    /// 1-based cell index within `cells[]`.
    pub cell_index: u32,
    /// 1-based line number of this cell in the raw `.ipynb` file.
    pub json_line: u32,
    /// `"code"`, `"markdown"`, or `"raw"`.
    pub cell_type: String,
    /// Normalized cell source text.
    pub source: String,
    /// Execution counter if `"code"` cell was executed.
    pub execution_count: Option<i64>,
    /// Output JSON objects.
    pub outputs: Vec<Value>,
}

fn locate_cell_json_lines(raw_json: &str) -> Vec<u32> {
    let mut lines = Vec::new();
    for (idx, line) in raw_json.lines().enumerate() {
        if line.contains("\"cell_type\"") {
            lines.push(u32::try_from(idx + 1).unwrap_or(u32::MAX));
        }
    }
    lines
}

/// Parses `.ipynb` JSON into [`ParsedCell`] items or records a `GF000` [`SyntaxErrorNode`].
pub fn parse_ipynb_cells(
    source: &str,
    syntax_errors: &mut Vec<SyntaxErrorNode>,
) -> Option<Vec<ParsedCell>> {
    let raw: RawNotebook = match serde_json::from_str(source) {
        Ok(nb) => nb,
        Err(err) => {
            let line = u32::try_from(err.line()).unwrap_or(1).max(1);
            syntax_errors.push(SyntaxErrorNode {
                span: Span::new(line, line).unwrap_or(Span::FIRST),
                message: format!("invalid Jupyter Notebook JSON: {err}"),
            });
            return None;
        }
    };
    if raw.nbformat < 4 {
        syntax_errors.push(SyntaxErrorNode {
            span: Span::FIRST,
            message: format!(
                "unsupported Jupyter nbformat v{} (expected >= 4)",
                raw.nbformat
            ),
        });
        return None;
    }
    let cell_lines = locate_cell_json_lines(source);
    let cells = raw
        .cells
        .into_iter()
        .enumerate()
        .map(|(idx, cell)| {
            let cell_index = u32::try_from(idx + 1).unwrap_or(u32::MAX);
            let json_line = cell_lines.get(idx).copied().unwrap_or(1).max(1);
            ParsedCell {
                cell_index,
                json_line,
                cell_type: cell.cell_type,
                source: cell.source.into_joined(),
                execution_count: cell.execution_count,
                outputs: cell.outputs,
            }
        })
        .collect();
    Some(cells)
}

fn collect_output_text(val: &Value, buf: &mut String) {
    match val {
        Value::String(s) => {
            buf.push_str(s);
            buf.push('\n');
        }
        Value::Array(items) => {
            for item in items {
                collect_output_text(item, buf);
            }
        }
        _ => {}
    }
}

fn extract_cell_output_blob(output: &Value) -> String {
    let mut blob = String::new();
    for key in ["text", "evalue", "traceback"] {
        if let Some(v) = output.get(key) {
            collect_output_text(v, &mut blob);
        }
    }
    if let Some(data) = output.get("data").and_then(Value::as_object) {
        for mime in ["text/plain", "text/html", "application/json"] {
            if let Some(v) = data.get(mime) {
                collect_output_text(v, &mut blob);
            }
        }
    }
    blob
}

fn audit_cell_outputs(
    path: &RepoPath,
    cell: &ParsedCell,
    span: Span,
    findings: &mut Vec<LangRuleFinding>,
) {
    for out_val in &cell.outputs {
        if out_val.get("output_type").and_then(Value::as_str) == Some("error") {
            let ename = out_val
                .get("ename")
                .and_then(Value::as_str)
                .unwrap_or("Error");
            findings.push(LangRuleFinding::new(
                NotebookViolation::CommittedErrorTraceback,
                span,
                format!(
                    "[cell #{}] committed error traceback (`{ename}`)",
                    cell.cell_index
                ),
            ));
        }
        let blob = extract_cell_output_blob(out_val);
        for sec in extract_secret_findings(path, &blob) {
            findings.push(LangRuleFinding::new(
                NotebookViolation::SecretInOutput,
                span,
                format!(
                    "[cell #{}] secret leaked in cell output (`{}`: {})",
                    cell.cell_index, sec.kind, sec.redacted_preview
                ),
            ));
        }
    }
}

/// Checks execution order, committed error tracebacks, and secret leaks inside `outputs[]`.
pub fn audit_notebook_cells_hygiene(
    path: &RepoPath,
    cells: &[ParsedCell],
    findings: &mut Vec<LangRuleFinding>,
    output_spans: &mut Vec<(u32, Span)>,
) {
    let mut prev_exec: Option<i64> = None;
    for cell in cells {
        if cell.cell_type != "code" {
            continue;
        }
        let span = Span::new(cell.json_line, cell.json_line).unwrap_or(Span::FIRST);
        if let Some(count) = cell.execution_count {
            if prev_exec.is_some_and(|prev| count <= prev) {
                findings.push(LangRuleFinding::new(
                    NotebookViolation::OutOfOrderExecution,
                    span,
                    format!(
                        "[cell #{}] out-of-order `execution_count`: [{count}] follows [{}]",
                        cell.cell_index,
                        prev_exec.unwrap_or(0)
                    ),
                ));
            }
            prev_exec = Some(count);
        }
        if !cell.outputs.is_empty() {
            output_spans.push((cell.cell_index, span));
        }
        audit_cell_outputs(path, cell, span, findings);
    }
}
