//! Pure Bubble Tea-inspired state machine (`ExplorerModel`) for `garde tui` / `garde check -i`.

use gf_core::{Diagnostic, DomainLayoutConfig, WorkspaceArchetype, classify_path_for_archetype};

use super::explorer_types::{ExplorerAction, ExplorerMode, ExplorerSnapshot};
use super::keys::Key;
use crate::ui::osc52_clipboard_sequence;

const PAGE_STEP: usize = 6;

/// Pure, deterministic state machine for `garde tui`.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExplorerModel {
    pub(crate) snapshot: ExplorerSnapshot,
    pub(crate) mode: ExplorerMode,
    pub(crate) filter_query: String,
    pub(crate) cursor: usize,
    pub(crate) status_notice: Option<String>,
}

impl ExplorerModel {
    /// Constructs a new [`ExplorerModel`] over `snapshot`.
    #[must_use]
    pub const fn new(snapshot: ExplorerSnapshot) -> Self {
        Self {
            snapshot,
            mode: ExplorerMode::Normal,
            filter_query: String::new(),
            cursor: 0,
            status_notice: None,
        }
    }

    /// Hot-replaces the check snapshot while preserving active filter and clamped cursor.
    pub fn replace_snapshot(&mut self, snapshot: ExplorerSnapshot) {
        let ms = snapshot.elapsed_ms;
        self.snapshot = snapshot;
        clamp_explorer_cursor(self);
        self.status_notice = Some(format!("Analyse actualisée à chaud ({ms} ms)"));
    }

    /// Returns `true` when the active report has `0` violations.
    #[must_use]
    pub const fn is_clean(&self) -> bool {
        self.snapshot.is_clean()
    }

    /// Returns the current 0-indexed cursor within [`Self::filtered_indices`].
    #[must_use]
    pub const fn cursor(&self) -> usize {
        self.cursor
    }

    /// Returns the active live filter query.
    #[must_use]
    pub fn filter_query(&self) -> &str {
        &self.filter_query
    }

    /// Returns the active [`ExplorerMode`].
    #[must_use]
    pub const fn mode(&self) -> ExplorerMode {
        self.mode
    }

    /// Returns indices into `snapshot.diagnostics()` matching `filter_query`.
    #[must_use]
    pub fn filtered_indices(&self) -> Vec<usize> {
        let needle = self.filter_query.trim().to_ascii_lowercase();
        let layout = DomainLayoutConfig::strict_defaults();
        let arch = self.snapshot.archetype();
        self.snapshot
            .diagnostics()
            .iter()
            .enumerate()
            .filter(|(_, d)| matches_filter(d, &needle, &layout, arch))
            .map(|(idx, _)| idx)
            .collect()
    }

    /// Returns the currently highlighted [`Diagnostic`] if the filtered list is non-empty.
    #[must_use]
    pub fn selected_diagnostic(&self) -> Option<&Diagnostic> {
        let indices = self.filtered_indices();
        let &diag_idx = indices.get(self.cursor)?;
        self.snapshot.diagnostics().get(diag_idx)
    }

    /// Processes a single [`Key`] event and returns the next [`ExplorerAction`].
    pub fn handle_key(&mut self, key: Key) -> ExplorerAction {
        if key == Key::CtrlC {
            return ExplorerAction::Quit;
        }
        match self.mode {
            ExplorerMode::Filtering => handle_filtering_key(self, key),
            ExplorerMode::AgentPreview => handle_agent_preview_key(self, key),
            ExplorerMode::Normal => handle_normal_key(self, key),
        }
    }
}

fn matches_filter(
    d: &Diagnostic,
    needle: &str,
    layout: &DomainLayoutConfig,
    arch: WorkspaceArchetype,
) -> bool {
    needle.is_empty()
        || d.rule_id.to_ascii_lowercase().contains(needle)
        || d.path.as_str().to_ascii_lowercase().contains(needle)
        || classify_path_for_archetype(&d.path, layout, arch)
            .as_str()
            .contains(needle)
        || d.message.to_ascii_lowercase().contains(needle)
}

fn clamp_explorer_cursor(m: &mut ExplorerModel) {
    let len = m.filtered_indices().len();
    m.cursor = if len == 0 { 0 } else { m.cursor.min(len - 1) };
}

fn handle_filtering_key(m: &mut ExplorerModel, key: Key) -> ExplorerAction {
    match key {
        Key::Esc => {
            m.filter_query.clear();
            m.mode = ExplorerMode::Normal;
        }
        Key::Enter => m.mode = ExplorerMode::Normal,
        Key::Backspace => {
            m.filter_query.pop();
        }
        Key::Char(ch) => m.filter_query.push(ch),
        _ => {}
    }
    clamp_explorer_cursor(m);
    ExplorerAction::Redraw
}

fn handle_agent_preview_key(m: &mut ExplorerModel, key: Key) -> ExplorerAction {
    match key {
        Key::Char('a') | Key::Esc => {
            m.mode = ExplorerMode::Normal;
            ExplorerAction::Redraw
        }
        Key::Char('r') => ExplorerAction::Reload,
        Key::Char('y') => {
            m.status_notice = Some(String::from("Rapport Agent copié (OSC 52)"));
            ExplorerAction::CopyOsc52(osc52_clipboard_sequence(&m.snapshot.agent_markdown))
        }
        Key::Char('q') => ExplorerAction::Quit,
        _ => ExplorerAction::Redraw,
    }
}

fn handle_normal_key(m: &mut ExplorerModel, key: Key) -> ExplorerAction {
    let len = m.filtered_indices().len();
    match key {
        Key::Char('q') | Key::Esc => return ExplorerAction::Quit,
        Key::Char('/') => m.mode = ExplorerMode::Filtering,
        Key::Char('a') => m.mode = ExplorerMode::AgentPreview,
        Key::Char('r') => return ExplorerAction::Reload,
        Key::Char('y') => return build_copy_action(m),
        _ => move_explorer_cursor(&mut m.cursor, len, key),
    }
    ExplorerAction::Redraw
}

fn move_explorer_cursor(cursor: &mut usize, len: usize, key: Key) {
    if len == 0 {
        *cursor = 0;
        return;
    }
    match key {
        Key::Up | Key::Char('k') => *cursor = cursor.saturating_sub(1),
        Key::Down | Key::Char('j') => *cursor = (*cursor + 1).min(len - 1),
        Key::PageUp => *cursor = cursor.saturating_sub(PAGE_STEP),
        Key::PageDown => *cursor = (*cursor + PAGE_STEP).min(len - 1),
        Key::Home | Key::Char('g') => *cursor = 0,
        Key::End | Key::Char('G') => *cursor = len - 1,
        _ => {}
    }
}

fn build_copy_action(m: &mut ExplorerModel) -> ExplorerAction {
    let payload = m.selected_diagnostic().map_or_else(
        || m.snapshot.agent_markdown.clone(),
        |d| {
            format!(
                "- **[{}]** `{}:{}-{}` (`measured: {}`, `max: {}`) — {} | **Remediation**: {}",
                d.rule_id,
                d.path,
                d.span.start_line(),
                d.span.end_line(),
                d.measured,
                d.threshold,
                d.message,
                d.remediation,
            )
        },
    );
    m.status_notice = Some(String::from(
        "Diagnostic copié dans le presse-papier (OSC 52)",
    ));
    ExplorerAction::CopyOsc52(osc52_clipboard_sequence(&payload))
}
