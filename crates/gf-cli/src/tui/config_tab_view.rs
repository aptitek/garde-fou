//! Pure Lipgloss view renderer (`render_config_tab`) for Tab `[2] Projet & Garde-fous`.

use std::collections::BTreeSet;

use gf_core::{ArchetypeSetting, GovernanceFlag};

use super::config_tab::ConfigTabModel;
use super::config_tab_catalog::{
    ALL_ARCHETYPE_SETTINGS, ALL_FEATURES, ALL_FRAMEWORKS, ALL_LANGUAGES, ConfigCategory,
};
use crate::ui::{CardStyle, ColorPolicy, PillRole, Role, paint, paint_bold, pill, rounded_card};

#[derive(Clone, Copy)]
enum ControlKind {
    Radio,
    Checkbox,
}

#[derive(Clone, Copy)]
enum RowFocus {
    Normal,
    Focused,
}

#[derive(Clone, Copy)]
struct OptionVisual {
    focus: RowFocus,
    control: ControlKind,
    selected: bool,
    detected: bool,
}

fn format_option_row(label: &str, vis: OptionVisual, policy: ColorPolicy) -> String {
    let is_focused = matches!(vis.focus, RowFocus::Focused);
    let ptr = if is_focused {
        paint_bold("▸", Role::Accent, policy)
    } else {
        String::from(" ")
    };
    let mark = match (vis.control, vis.selected) {
        (ControlKind::Radio, true) => paint_bold("◉", Role::Success, policy),
        (ControlKind::Radio, false) => paint("○", Role::Muted, policy),
        (ControlKind::Checkbox, true) => paint_bold("[✓]", Role::Success, policy),
        (ControlKind::Checkbox, false) => paint("[ ]", Role::Muted, policy),
    };
    let text = if is_focused {
        paint_bold(label, Role::Accent, policy)
    } else {
        String::from(label)
    };
    let auto_tag = if vis.detected {
        format!(" {}", paint("(détecté)", Role::Muted, policy))
    } else {
        String::new()
    };
    format!("{ptr} {mark} {text}{auto_tag}")
}

fn render_category_bar(active: ConfigCategory, policy: ColorPolicy) -> String {
    let chips: Vec<String> = ConfigCategory::ALL
        .iter()
        .map(|&cat| {
            let role = if cat == active {
                PillRole::BadgeAccent
            } else {
                PillRole::BadgeMuted
            };
            pill(cat.label(), role, policy)
        })
        .collect();
    chips.join(" ")
}

struct SetRowsCtx<'a, T> {
    items: &'a [T],
    selected: &'a BTreeSet<T>,
    detected: &'a BTreeSet<T>,
    cursor: usize,
    policy: ColorPolicy,
}

fn format_set_rows<T: Ord + Copy>(
    ctx: &SetRowsCtx<'_, T>,
    label_of: fn(T) -> &'static str,
) -> Vec<String> {
    ctx.items
        .iter()
        .enumerate()
        .map(|(i, &item)| {
            let vis = OptionVisual {
                focus: if i == ctx.cursor {
                    RowFocus::Focused
                } else {
                    RowFocus::Normal
                },
                control: ControlKind::Checkbox,
                selected: ctx.selected.contains(&item),
                detected: ctx.detected.contains(&item),
            };
            format_option_row(label_of(item), vis, ctx.policy)
        })
        .collect()
}

fn format_archetype_rows(m: &ConfigTabModel, policy: ColorPolicy) -> Vec<String> {
    ALL_ARCHETYPE_SETTINGS
        .iter()
        .enumerate()
        .map(|(i, &setting)| {
            let label = match setting {
                ArchetypeSetting::Auto => {
                    format!("Auto (détecté: {})", m.draft.detected_archetype.as_str())
                }
                ArchetypeSetting::Explicit(a) => String::from(a.as_str()),
            };
            let vis = OptionVisual {
                focus: if i == m.cursor {
                    RowFocus::Focused
                } else {
                    RowFocus::Normal
                },
                control: ControlKind::Radio,
                selected: m.draft.archetype == setting,
                detected: false,
            };
            format_option_row(&label, vis, policy)
        })
        .collect()
}

fn format_governance_rows(m: &ConfigTabModel, policy: ColorPolicy) -> Vec<String> {
    GovernanceFlag::ALL
        .iter()
        .enumerate()
        .map(|(i, &flag)| {
            let vis = OptionVisual {
                focus: if i == m.cursor {
                    RowFocus::Focused
                } else {
                    RowFocus::Normal
                },
                control: ControlKind::Checkbox,
                selected: m.draft.governance.is_set(flag),
                detected: false,
            };
            format_option_row(flag.label(), vis, policy)
        })
        .collect()
}

fn build_category_rows(m: &ConfigTabModel, policy: ColorPolicy) -> Vec<String> {
    match m.category {
        ConfigCategory::Archetype => format_archetype_rows(m, policy),
        ConfigCategory::Languages => format_set_rows(
            &SetRowsCtx {
                items: &ALL_LANGUAGES,
                selected: &m.draft.languages,
                detected: &m.draft.detected_languages,
                cursor: m.cursor,
                policy,
            },
            gf_core::LanguageId::as_str,
        ),
        ConfigCategory::Frameworks => format_set_rows(
            &SetRowsCtx {
                items: &ALL_FRAMEWORKS,
                selected: &m.draft.frameworks,
                detected: &m.draft.detected_frameworks,
                cursor: m.cursor,
                policy,
            },
            gf_core::FrameworkId::as_str,
        ),
        ConfigCategory::Features => format_set_rows(
            &SetRowsCtx {
                items: &ALL_FEATURES,
                selected: &m.draft.features,
                detected: &m.draft.detected_features,
                cursor: m.cursor,
                policy,
            },
            gf_core::FeatureId::as_str,
        ),
        ConfigCategory::Governance => format_governance_rows(m, policy),
    }
}

/// Renders Tab `[2] Projet & Garde-fous` (`ConfigTabModel`) as a Lipgloss card.
#[must_use]
pub fn render_config_tab(model: &ConfigTabModel, policy: ColorPolicy, max_width: usize) -> String {
    let cat_bar = render_category_bar(model.category, policy);
    let mut lines = vec![cat_bar, String::new()];
    lines.extend(build_category_rows(model, policy));

    let pending = model.unsaved_changes();
    lines.push(String::new());
    let status = if pending > 0 {
        format!(
            "{}  {}",
            paint_bold(
                &format!("● {pending} changement(s) non enregistré(s)"),
                Role::Warning,
                policy,
            ),
            pill("Appliquer & Synchroniser (s)", PillRole::BadgeOk, policy),
        )
    } else {
        paint(
            "✓ Configuration synchronisée avec garde-fou.toml",
            Role::Success,
            policy,
        )
    };
    lines.push(status);
    if let Some(msg) = &model.notice {
        lines.push(paint_bold(&format!("✓ {msg}"), Role::Accent, policy));
    }

    let card = rounded_card(
        Some("Projet & Garde-fous (garde-fou.toml lossless)"),
        &lines,
        CardStyle::new(Role::Accent, policy),
        max_width,
    );
    let help = paint(
        "  [←/→ h/l] catégorie · [↑/↓ j/k] ligne · [Espace/x] cocher · [s/Entrée] appliquer & sync · [u/Échap] annuler",
        Role::Muted,
        policy,
    );
    format!("{card}\n{help}")
}
