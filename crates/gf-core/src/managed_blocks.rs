//! Pure `#![no_std]` reversible extraction & surgical stripping of Garde-fou managed blocks
//! (`AGENTS.md`, `.gitignore`, `.dockerignore`, and `lefthook.yml`).

use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;

use crate::agent_fragments::{AGENTS_MANAGED_END, AGENTS_MANAGED_START};
use crate::gitignore::{MANAGED_END, MANAGED_START};

const DEFAULT_GENERATED_AGENTS_STUB: &str = "# AGENTS.md\n\n\
## Project-Specific Instructions\n\n\
Add custom architecture notes and domain invariants here (preserved across `garde sync`).";

/// Strips a single delimited managed block (`start_marker` .. `end_marker`) from `content`,
/// preserving all surrounding human-authored lines cleanly with LF line endings.
#[must_use]
pub fn strip_delimited_managed_block(
    content: &str,
    start_marker: &str,
    end_marker: &str,
) -> String {
    let norm = content.replace("\r\n", "\n").replace('\r', "\n");
    let Some(start_idx) = norm.find(start_marker) else {
        return norm;
    };
    let Some(rel_end) = norm[start_idx..].find(end_marker) else {
        return norm;
    };
    let end_idx = start_idx + rel_end + end_marker.len();
    let before = norm[..start_idx].trim_end_matches('\n');
    let after = norm[end_idx..]
        .trim_start_matches('\n')
        .trim_end_matches('\n');
    match (before.is_empty(), after.is_empty()) {
        (true, true) => String::new(),
        (true, false) => format!("{after}\n"),
        (false, true) => format!("{before}\n"),
        (false, false) => format!("{before}\n\n{after}\n"),
    }
}

/// Surgically removes the HTML-comment managed block from `AGENTS.md`.
#[must_use]
pub fn strip_html_managed_block(content: &str) -> String {
    strip_delimited_managed_block(content, AGENTS_MANAGED_START, AGENTS_MANAGED_END)
}

/// Surgically removes the `# >>> garde-fou managed >>>` block from `.gitignore` / `.dockerignore`.
#[must_use]
pub fn strip_hash_managed_block(content: &str) -> String {
    strip_delimited_managed_block(content, MANAGED_START, MANAGED_END)
}

/// Returns `true` when `stripped_agents_md` contains no human notes beyond the empty
/// default scaffold header emitted by `merge_agents_md(None, ...)`.
#[must_use]
pub fn is_empty_generated_agents_header(stripped_agents_md: &str) -> bool {
    let trimmed = stripped_agents_md.trim();
    trimmed.is_empty() || trimmed == DEFAULT_GENERATED_AGENTS_STUB
}

fn is_garde_run_command(line: &str) -> bool {
    let t = line.trim();
    t.starts_with("run:")
        && (t.contains("garde ") || t.ends_with("garde") || t.contains("garde-fou"))
}

/// Removes `garde` hook command entries from `lefthook.yml`.
///
/// Returns `None` if no non-`garde` hook commands remain (safe to delete `lefthook.yml`),
/// or `Some(cleaned)` preserving all user-authored hooks.
#[must_use]
pub fn strip_garde_lefthook_commands(content: &str) -> Option<String> {
    let norm = content.replace("\r\n", "\n").replace('\r', "\n");
    let lines: Vec<&str> = norm.lines().collect();
    let has_other_run = lines.iter().any(|line| {
        let t = line.trim();
        t.starts_with("run:") && !is_garde_run_command(line)
    });
    if !has_other_run {
        return None;
    }
    let kept: Vec<&str> = lines
        .into_iter()
        .filter(|line| !is_garde_run_command(line))
        .collect();
    let mut out = kept.join("\n");
    if !out.ends_with('\n') {
        out.push('\n');
    }
    Some(out)
}
