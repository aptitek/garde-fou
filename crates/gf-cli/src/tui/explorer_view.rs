//! Pure Lipgloss view renderer (`render_explorer`) for `garde tui` / `garde check -i`.

use gf_core::{DomainLayoutConfig, classify_path_for_archetype};

use super::explorer_model::ExplorerModel;
use super::explorer_types::ExplorerMode;
use crate::ui::{
    CardStyle, ColorPolicy, HumanCheckMeta, PillRole, Role, SourceFileView, paint, paint_bold,
    pill, render_context_capsules, render_single_diagnostic_card, rounded_card,
};

const LIST_WINDOW: usize = 8;

/// Renders the current viewport of [`ExplorerModel`] as a pure Lipgloss string.
#[must_use]
pub fn render_explorer(model: &ExplorerModel, policy: ColorPolicy, max_width: usize) -> String {
    let top_bar = render_top_bar(model, policy);
    let body = match model.mode {
        ExplorerMode::AgentPreview => render_agent_preview(model, policy, max_width),
        ExplorerMode::Normal | ExplorerMode::Filtering => {
            if model.is_clean() {
                render_zero_violation_card(model, policy, max_width)
            } else {
                render_diagnostics_pane(model, policy, max_width)
            }
        }
    };
    format!("{top_bar}\n\n{body}\n{}", render_footer(model, policy))
}

fn render_top_bar(m: &ExplorerModel, policy: ColorPolicy) -> String {
    let rep = &m.snapshot.report;
    let brand = pill("GARDE TUI", PillRole::BadgeAccent, policy);
    let arch = pill(
        &format!("arch={}", m.snapshot.archetype().as_str()),
        PillRole::BadgeMuted,
        policy,
    );
    let files = pill(
        &format!("{} fichiers", rep.files_checked),
        PillRole::BadgeMuted,
        policy,
    );
    let count = if rep.is_clean() {
        pill("✓ 0 violation", PillRole::BadgeOk, policy)
    } else {
        pill(
            &format!("✗ {} violation(s)", rep.diagnostics.len()),
            PillRole::BadgeError,
            policy,
        )
    };
    let filter_cap = if m.mode == ExplorerMode::Filtering {
        pill(
            &format!("🔍 /{}_", m.filter_query),
            PillRole::BadgeAccent,
            policy,
        )
    } else if !m.filter_query.is_empty() {
        pill(
            &format!("🔍 /{}", m.filter_query),
            PillRole::BadgeWarn,
            policy,
        )
    } else {
        paint("[/] filtrer", Role::Muted, policy)
    };
    format!("{brand} {arch} {files} {count} {filter_cap}")
}

fn render_agent_preview(m: &ExplorerModel, policy: ColorPolicy, max_width: usize) -> String {
    let lines: Vec<String> = m
        .snapshot
        .agent_markdown
        .lines()
        .take(24)
        .map(String::from)
        .collect();
    rounded_card(
        Some("Aperçu Markdown Agent (--format agent)"),
        &lines,
        CardStyle::new(Role::Accent, policy),
        max_width,
    )
}

fn render_zero_violation_card(m: &ExplorerModel, policy: ColorPolicy, max_width: usize) -> String {
    let rep = &m.snapshot.report;
    let total_cache = rep.cache_hits + rep.cache_misses;
    let lines = vec![
        paint_bold(
            "✓ Espace de travail 100 % conforme aux garde-fous architecturaux",
            Role::Success,
            policy,
        ),
        format!(
            "Cache BLAKE3 : ⚡ {}/{total_cache} hits · Durée : {} ms",
            rep.cache_hits, m.snapshot.elapsed_ms
        ),
        render_context_capsules(&rep.context, None, policy),
    ];
    rounded_card(
        Some("Intégrité Vérifiée — 0 Violation"),
        &lines,
        CardStyle::new(Role::Success, policy),
        max_width,
    )
}

fn render_diagnostics_pane(m: &ExplorerModel, policy: ColorPolicy, max_width: usize) -> String {
    let indices = m.filtered_indices();
    if indices.is_empty() {
        let empty = vec![paint(
            &format!(
                "Aucune violation ne correspond au filtre `/{}`.",
                m.filter_query
            ),
            Role::Muted,
            policy,
        )];
        return rounded_card(
            Some("Liste filtrée (0 résultat)"),
            &empty,
            CardStyle::new(Role::SurfaceBorder, policy),
            max_width,
        );
    }

    let list_card = render_windowed_list(m, &indices, policy, max_width);
    let Some(selected) = m.selected_diagnostic() else {
        return list_card;
    };
    let src = m
        .snapshot
        .sources
        .get(selected.path.as_str())
        .unwrap_or(&SourceFileView::Unavailable);
    let meta = HumanCheckMeta {
        elapsed_ms: m.snapshot.elapsed_ms,
        lcov_tenths: None,
        policy,
        max_width,
    };
    let detail_card = render_single_diagnostic_card(selected, m.snapshot.archetype(), src, &meta);
    format!("{list_card}\n{detail_card}")
}

fn render_windowed_list(
    m: &ExplorerModel,
    indices: &[usize],
    policy: ColorPolicy,
    max_width: usize,
) -> String {
    let half = LIST_WINDOW / 2;
    let start = m
        .cursor
        .saturating_sub(half)
        .min(indices.len().saturating_sub(LIST_WINDOW));
    let end = (start + LIST_WINDOW).min(indices.len());
    let layout = DomainLayoutConfig::strict_defaults();
    let arch = m.snapshot.archetype();
    let all_diags = m.snapshot.diagnostics();

    let mut rows = Vec::with_capacity(end - start);
    for (offset, &diag_idx) in indices[start..end].iter().enumerate() {
        let pos = start + offset;
        if let Some(d) = all_diags.get(diag_idx) {
            let active = pos == m.cursor;
            let ptr = if active {
                paint_bold("▸", Role::Accent, policy)
            } else {
                String::from(" ")
            };
            let dom = classify_path_for_archetype(&d.path, &layout, arch)
                .as_str()
                .to_ascii_uppercase();
            let loc = format!("{}:{}", d.path, d.span.start_line());
            let loc_styled = if active {
                paint_bold(&loc, Role::Accent, policy)
            } else {
                loc
            };
            rows.push(format!(
                "{ptr} {} {} {loc_styled} — {}",
                pill(&d.rule_id, PillRole::BadgeError, policy),
                pill(&dom, PillRole::BadgeDomain, policy),
                paint(&d.message, Role::Muted, policy),
            ));
        }
    }
    let title = format!("Diagnostics ({}/{})", m.cursor + 1, indices.len());
    rounded_card(
        Some(&title),
        &rows,
        CardStyle::new(Role::SurfaceBorder, policy),
        max_width,
    )
}

fn render_footer(m: &ExplorerModel, policy: ColorPolicy) -> String {
    let notice = m.status_notice.as_deref().map_or_else(String::new, |msg| {
        format!(
            "  {}\n",
            paint_bold(&format!("✓ {msg}"), Role::Success, policy)
        )
    });
    let keys = paint(
        "  [↑/↓ j/k] naviguer · [/] filtrer · [r] relancer · [a] mode agent · [y] copier OSC 52 · [q/Échap] quitter",
        Role::Muted,
        policy,
    );
    format!("{notice}{keys}")
}
