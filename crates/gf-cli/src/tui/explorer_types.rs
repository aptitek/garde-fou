//! Leaf types (`ExplorerSnapshot`, `ExplorerMode`, `ExplorerAction`) for `garde tui`.

use std::collections::BTreeMap;

use gf_core::{Diagnostic, WorkspaceArchetype};
use gf_engine::CheckReport;

use crate::ui::SourceFileView;

/// Immutable check payload displayed and hot-reloaded inside `ExplorerModel`.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExplorerSnapshot {
    /// Latest workspace [`CheckReport`].
    pub report: CheckReport,
    /// Lazy in-memory source snippets for diagnosed files only.
    pub sources: BTreeMap<String, SourceFileView>,
    /// Execution duration of the check in milliseconds.
    pub elapsed_ms: u128,
    /// Pre-rendered `--format agent` Markdown report ready for preview & `OSC 52` copy.
    pub agent_markdown: String,
}

impl ExplorerSnapshot {
    /// Returns `true` when zero violations were found.
    #[must_use]
    pub const fn is_clean(&self) -> bool {
        self.report.is_clean()
    }

    /// Returns the resolved [`WorkspaceArchetype`] of the analyzed workspace.
    #[must_use]
    pub const fn archetype(&self) -> WorkspaceArchetype {
        self.report.context.archetype
    }

    /// Returns the slice of all diagnostics in the report.
    #[must_use]
    pub fn diagnostics(&self) -> &[Diagnostic] {
        &self.report.diagnostics
    }
}

/// Active interaction mode of `ExplorerModel`.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ExplorerMode {
    /// Navigating diagnostic list and inspecting code frames.
    Normal,
    /// Typing a live filter query (`/GF061`, `/forme`, `/path`).
    Filtering,
    /// Viewing the `--format agent` LLM-ready Markdown block.
    AgentPreview,
}

/// Side-effect command emitted by `ExplorerModel::handle_key` to the imperative terminal loop.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ExplorerAction {
    /// Re-render the current model viewport.
    Redraw,
    /// Re-run `garde check` hot via `BLAKE3` cache and replace the snapshot.
    Reload,
    /// Emit the wrapped `OSC 52` Base64 sequence to the terminal clipboard and redraw.
    CopyOsc52(String),
    /// Exit the interactive TUI cleanly, restoring cursor and screen.
    Quit,
}
