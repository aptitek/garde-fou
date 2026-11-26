//! Pure Lipgloss + Miette `human` reporter for `garde check` (0-violation compact banner
//! and file-grouped diagnostic cards with source snippets).

use std::collections::BTreeMap;
use std::fmt::Write as _;

use gf_core::{Diagnostic, DomainLayoutConfig, WorkspaceArchetype, classify_path_for_archetype};
use gf_engine::{CheckReport, ScanScope};

use super::box_layout::{CardStyle, rounded_card};
use super::code_frame::{CodeFrameSpec, SourceFileView, render_code_frame};
use super::human_summary::{render_context_capsules, render_violation_footer};
use super::policy::ColorPolicy;
use super::theme::{PillRole, Role, paint, paint_bold, pill};

/// Display metadata passed alongside [`CheckReport`] to [`render_human_check`].
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct HumanCheckMeta {
    /// Wall-clock check execution time in milliseconds.
    pub elapsed_ms: u128,
    /// Optional `LCOV` line coverage expressed in tenths of a percent (`945` = `94.5%`).
    pub lcov_tenths: Option<u32>,
    /// Resolved terminal color policy.
    pub policy: ColorPolicy,
    /// Maximum terminal width in columns.
    pub max_width: usize,
}

impl Default for HumanCheckMeta {
    fn default() -> Self {
        Self {
            elapsed_ms: 0,
            lcov_tenths: None,
            policy: ColorPolicy::Plain,
            max_width: 100,
        }
    }
}

struct FileGroupSpec<'a> {
    path: &'a str,
    diags: &'a [&'a Diagnostic],
    archetype: WorkspaceArchetype,
    source: &'a SourceFileView,
}

const fn scope_word(scope: ScanScope) -> &'static str {
    match scope {
        ScanScope::All => "workspace",
        ScanScope::StagedOnly => "staged",
    }
}

fn render_clean_banner(report: &CheckReport, meta: &HumanCheckMeta) -> String {
    let badge = pill("✓ GARDE", PillRole::BadgeOk, meta.policy);
    let ok_lbl = paint_bold("ok", Role::Success, meta.policy);
    let total = report.cache_hits + report.cache_misses;
    let sep = paint(" · ", Role::Muted, meta.policy);
    let cache = paint(
        &format!("⚡ {}/{total} cache", report.cache_hits),
        Role::Accent,
        meta.policy,
    );
    let ms = paint(&format!("{} ms", meta.elapsed_ms), Role::Muted, meta.policy);
    let top = format!(
        "{badge} {ok_lbl}  scope={}{sep}{} fichiers{sep}{cache}{sep}{ms}",
        scope_word(report.scope),
        report.files_checked,
    );
    format!(
        "{top}\n{}",
        render_context_capsules(&report.context, meta.lcov_tenths, meta.policy)
    )
}

/// Builds the inner lines for a single [`Diagnostic`] inside a card.
#[must_use]
pub fn build_diagnostic_block(
    diag: &Diagnostic,
    archetype: WorkspaceArchetype,
    source: &SourceFileView,
    meta: &HumanCheckMeta,
) -> Vec<String> {
    let dom = classify_path_for_archetype(
        &diag.path,
        &DomainLayoutConfig::strict_defaults(),
        archetype,
    );
    let rule_pill = pill(&diag.rule_id, PillRole::BadgeError, meta.policy);
    let dom_pill = pill(
        &dom.as_str().to_ascii_uppercase(),
        PillRole::BadgeDomain,
        meta.policy,
    );
    let metric = if diag.threshold > 0 || diag.measured > 0 {
        paint(
            &format!(" (mesuré: {} / max: {})", diag.measured, diag.threshold),
            Role::Warning,
            meta.policy,
        )
    } else {
        String::new()
    };
    let mut lines = vec![format!("{rule_pill} {dom_pill} {}{metric}", diag.message)];
    let spec = CodeFrameSpec {
        path: diag.path.as_str(),
        start_line: diag.span.start_line(),
        end_line: diag.span.end_line(),
        source,
    };
    lines.extend(render_code_frame(
        &spec,
        meta.policy,
        meta.max_width.saturating_sub(6).max(20),
    ));
    lines.push(format!(
        "{} {}",
        paint_bold("💡 Remédiation :", Role::Accent, meta.policy),
        paint(&diag.remediation, Role::Muted, meta.policy)
    ));
    lines
}

/// Renders a standalone Lipgloss card for a single [`Diagnostic`] (used by `garde tui`).
#[must_use]
pub fn render_single_diagnostic_card(
    diag: &Diagnostic,
    archetype: WorkspaceArchetype,
    source: &SourceFileView,
    meta: &HumanCheckMeta,
) -> String {
    let lines = build_diagnostic_block(diag, archetype, source, meta);
    rounded_card(
        Some(diag.path.as_str()),
        &lines,
        CardStyle::new(Role::Error, meta.policy),
        meta.max_width,
    )
}

/// Renders the complete `human` check report (`0`-violation banner or grouped file cards).
#[must_use]
pub fn render_human_check(
    report: &CheckReport,
    sources: &BTreeMap<String, SourceFileView>,
    meta: &HumanCheckMeta,
) -> String {
    if report.is_clean() {
        return render_clean_banner(report, meta);
    }
    let mut grouped: BTreeMap<&str, Vec<&Diagnostic>> = BTreeMap::new();
    for diag in &report.diagnostics {
        grouped.entry(diag.path.as_str()).or_default().push(diag);
    }
    let mut out = render_fail_header(report, grouped.len(), meta);
    for (path, diags) in &grouped {
        let source = sources.get(*path).unwrap_or(&SourceFileView::Unavailable);
        let spec = FileGroupSpec {
            path,
            diags,
            archetype: report.context.archetype,
            source,
        };
        let _ = write!(out, "\n\n{}", render_file_group_card(&spec, meta));
    }
    let footer =
        render_violation_footer(&report.diagnostics, report.context.archetype, meta.policy);
    let _ = write!(out, "\n\n{footer}");
    out
}

fn render_fail_header(report: &CheckReport, file_count: usize, meta: &HumanCheckMeta) -> String {
    let badge = pill("✗ GARDE", PillRole::BadgeError, meta.policy);
    let fail = paint_bold("FAIL", Role::Error, meta.policy);
    let total = report.cache_hits + report.cache_misses;
    let sep = paint(" · ", Role::Muted, meta.policy);
    format!(
        "{badge} {fail}  {} violation(s) dans {file_count} fichier(s){sep}⚡ {}/{total} cache{sep}{} ms",
        report.diagnostics.len(),
        report.cache_hits,
        meta.elapsed_ms,
    )
}

fn render_file_group_card(group: &FileGroupSpec<'_>, meta: &HumanCheckMeta) -> String {
    let mut lines = Vec::new();
    let div_w = meta.max_width.saturating_sub(8).clamp(8, 48);
    let divider = paint(&"─".repeat(div_w), Role::SurfaceBorder, meta.policy);
    for (idx, diag) in group.diags.iter().enumerate() {
        if idx > 0 {
            lines.push(divider.clone());
        }
        lines.extend(build_diagnostic_block(
            diag,
            group.archetype,
            group.source,
            meta,
        ));
    }
    rounded_card(
        Some(group.path),
        &lines,
        CardStyle::new(Role::Error, meta.policy),
        meta.max_width,
    )
}
