//! Closed algebraic sum types for Jupyter Notebook (`.ipynb`), Quarto (`.qmd`/`_quarto.yml`),
//! and Obsidian (`OFM`/`.canvas`) violations (`GF080`) plus `[notebook]` configuration.

use alloc::collections::BTreeSet;
use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;
use core::fmt;
use serde::{Deserialize, Serialize};

use crate::diagnostic::Span;

/// `[notebook]` configuration governing Jupyter `.ipynb` hygiene gates.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NotebookConfig {
    /// When `true`, reject notebooks committed with non-empty cell `outputs[]` (`NB-DIRTY-OUTPUTS`).
    #[serde(default)]
    pub require_clean_outputs: bool,
}

impl NotebookConfig {
    /// Deterministic canonical fragment for `BLAKE3` cache keys.
    #[must_use]
    pub fn canonical_fragment(self) -> String {
        format!("nb(clean={})", self.require_clean_outputs)
    }
}

/// Native Jupyter Notebook (`.ipynb`, nbformat v4) hygiene and reproducibility violations.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
pub enum NotebookViolation {
    /// Redacted secret leaked inside a notebook cell `outputs[]` payload (`NB-SECRET-IN-OUTPUT`).
    SecretInOutput,
    /// Committed cell output with `output_type == "error"` traceback (`NB-COMMITTED-ERROR-TRACEBACK`).
    CommittedErrorTraceback,
    /// Non-null code cell `execution_count` values are not strictly increasing (`NB-OUT-OF-ORDER-EXECUTION`).
    OutOfOrderExecution,
    /// Code cell exceeds `max_function_lines` effective lines (`NB-OVERSIZED-CELL`).
    OversizedCell,
    /// Code cell contains persisted outputs when `[notebook] require_clean_outputs = true` (`NB-DIRTY-OUTPUTS`).
    DirtyOutputs,
}

const NB_SPECS: [(&str, &str); 5] = [
    (
        "NB-SECRET-IN-OUTPUT",
        "Strip or rotate the leaked credential, clear notebook cell outputs, and never print secrets to stdout/display.",
    ),
    (
        "NB-COMMITTED-ERROR-TRACEBACK",
        "Fix the failing cell and re-run the notebook top-to-bottom (or clear outputs) before committing.",
    ),
    (
        "NB-OUT-OF-ORDER-EXECUTION",
        "Run 'Restart Kernel & Run All Cells' so `execution_count` increases strictly top-to-bottom (or clear all execution counts).",
    ),
    (
        "NB-OVERSIZED-CELL",
        "Split this code cell into smaller focused cells or extract reusable logic into a dedicated `.py` module.",
    ),
    (
        "NB-DIRTY-OUTPUTS",
        "Clear cell outputs before committing (e.g. `jupyter nbconvert --clear-output --inplace`) or disable `notebook.require_clean_outputs`.",
    ),
];

impl NotebookViolation {
    /// Canonical rule identifier (`"NB-..."`).
    #[must_use]
    pub const fn code(self) -> &'static str {
        NB_SPECS[self as usize].0
    }

    /// Actionable root-cause remediation guidance.
    #[must_use]
    pub const fn remediation(self) -> &'static str {
        NB_SPECS[self as usize].1
    }
}

/// Native Quarto (`_quarto.yml` and `.qmd`) structural and cross-reference violations.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
pub enum QuartoViolation {
    /// File referenced in `_quarto.yml` (`chapters`, `bibliography`, `sidebar`, `theme`, etc.) does not exist.
    MissingReferencedFile,
    /// Unclosed or mismatched Pandoc fenced div `:::` in `.qmd`.
    UnclosedDiv,
    /// Target of `{{< include ... >}}` or `{{< embed ... >}}` shortcode does not exist.
    BrokenShortcodeTarget,
    /// Cross-reference (`@fig-*`, `@tbl-*`, `@sec-*`, `@eq-*`) has no matching `#| label:` or `{#...}` target.
    BrokenCrossref,
    /// Malformed YAML in `#| ...` executable cell options.
    InvalidCellOptionYaml,
}

const QUARTO_SPECS: [(&str, &str); 5] = [
    (
        "QUARTO-MISSING-REFERENCED-FILE",
        "Create the missing file or update the path in `_quarto.yml` (`chapters`, `sidebar`, `bibliography`, `theme`).",
    ),
    (
        "QUARTO-UNCLOSED-DIV",
        "Balance every opening Pandoc fenced div `::: {.class}` with a matching closing `:::` line.",
    ),
    (
        "QUARTO-BROKEN-SHORTCODE-TARGET",
        "Verify that the path referenced by `{{< include ... >}}` or `{{< embed ... >}}` exists relative to this `.qmd` file.",
    ),
    (
        "QUARTO-BROKEN-CROSSREF",
        "Define a matching `#| label: <id>` option or `{#<id>}` heading/div identifier for every `@fig-`/`@tbl-`/`@sec-`/`@eq-` cross-reference.",
    ),
    (
        "QUARTO-INVALID-CELL-OPTION",
        "Fix the YAML syntax in `#| key: value` Quarto cell options.",
    ),
];

impl QuartoViolation {
    /// Canonical rule identifier (`"QUARTO-..."`).
    #[must_use]
    pub const fn code(self) -> &'static str {
        QUARTO_SPECS[self as usize].0
    }

    /// Actionable root-cause remediation guidance.
    #[must_use]
    pub const fn remediation(self) -> &'static str {
        QUARTO_SPECS[self as usize].1
    }
}

/// Native Obsidian Flavored Markdown (`OFM`) and JSON Canvas (`.canvas`) violations.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
pub enum ObsidianViolation {
    /// `[[WikiLink]]` or `![[embed]]` target cannot be resolved to any note or asset in the vault.
    BrokenWikilinkOrEmbed,
    /// JSON Canvas `.canvas` references a missing file node or an edge with an unknown `fromNode`/`toNode` ID.
    CanvasBrokenRef,
}

const OBSIDIAN_SPECS: [(&str, &str); 2] = [
    (
        "OBS-BROKEN-WIKILINK-OR-EMBED",
        "Create the referenced note/asset or fix the `[[WikiLink]]` / `![[embed]]` target name in the vault.",
    ),
    (
        "OBS-CANVAS-BROKEN-REF",
        "Ensure every `.canvas` `file` node points to an existing vault file and every edge `fromNode`/`toNode` matches a declared node `id`.",
    ),
];

impl ObsidianViolation {
    /// Canonical rule identifier (`"OBS-..."`).
    #[must_use]
    pub const fn code(self) -> &'static str {
        OBSIDIAN_SPECS[self as usize].0
    }

    /// Actionable root-cause remediation guidance.
    #[must_use]
    pub const fn remediation(self) -> &'static str {
        OBSIDIAN_SPECS[self as usize].1
    }
}

/// Unified closed sum type across Jupyter, Quarto, and Obsidian native violations (`GF080`).
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
pub enum AcademicViolation {
    /// Jupyter Notebook (`.ipynb`) violation (`NB-*`).
    Notebook(NotebookViolation),
    /// Quarto (`_quarto.yml`, `.qmd`) violation (`QUARTO-*`).
    Quarto(QuartoViolation),
    /// Obsidian (`OFM`, `.canvas`) violation (`OBS-*`).
    Obsidian(ObsidianViolation),
}

impl AcademicViolation {
    /// Canonical ecosystem rule identifier.
    #[must_use]
    pub const fn code(self) -> &'static str {
        match self {
            Self::Notebook(v) => v.code(),
            Self::Quarto(v) => v.code(),
            Self::Obsidian(v) => v.code(),
        }
    }

    /// Actionable root-cause remediation guidance.
    #[must_use]
    pub const fn remediation(self) -> &'static str {
        match self {
            Self::Notebook(v) => v.remediation(),
            Self::Quarto(v) => v.remediation(),
            Self::Obsidian(v) => v.remediation(),
        }
    }
}

impl fmt::Display for AcademicViolation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.code())
    }
}

/// Reference extracted from `_quarto.yml`, `.qmd` shortcodes, Obsidian wikilinks, or `.canvas` files
/// for pure workspace-level resolution in `gf-rules`.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct WorkspaceRefItem {
    /// Target relative path or vault note stem.
    pub target: String,
    /// Optional heading/block fragment (`#Heading` or `#^block-id`).
    pub fragment: Option<String>,
    /// 1-based line span where the reference appeared.
    pub span: Span,
}

/// Workspace-resolved metadata extracted from Jupyter, Quarto, or Obsidian documents.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct AcademicSummary {
    /// Relative paths referenced in `_quarto.yml` (`chapters`, `bibliography`, `sidebar`, `theme`).
    #[serde(default)]
    pub quarto_config_refs: Vec<WorkspaceRefItem>,
    /// Relative paths referenced by `{{< include ... >}}` / `{{< embed ... >}}` in `.qmd`.
    #[serde(default)]
    pub quarto_shortcode_refs: Vec<WorkspaceRefItem>,
    /// Obsidian `[[WikiLink]]` and `![[embed]]` references extracted outside code and `%% ... %%`.
    #[serde(default)]
    pub obsidian_wikilinks: Vec<WorkspaceRefItem>,
    /// Obsidian `^block-id` identifiers declared in this note.
    #[serde(default)]
    pub obsidian_block_ids: BTreeSet<String>,
    /// File paths referenced by `"type": "file"` nodes in a `.canvas` file.
    #[serde(default)]
    pub canvas_file_refs: Vec<WorkspaceRefItem>,
    /// Code cell effective line counts `(cell_index_1based, effective_lines, span)` in `.ipynb`.
    #[serde(default)]
    pub notebook_cell_lines: Vec<(u32, u32, Span)>,
    /// Non-empty output spans `(cell_index_1based, span)` in `.ipynb` for `require_clean_outputs`.
    #[serde(default)]
    pub notebook_output_spans: Vec<(u32, Span)>,
}
