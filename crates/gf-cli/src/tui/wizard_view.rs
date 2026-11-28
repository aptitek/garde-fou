//! Pure Lipgloss view renderer (`render_wizard`) for the 4-step `garde init` wizard.

use super::wizard_data::{ARCHETYPE_CHOICES, FEATURE_CHOICES};
use super::wizard_preview::preview_init_files;
use super::wizard_state::WizardModel;
use super::wizard_types::{
    ConfirmChoice, MultiInputMode, MultiPaneSpec, MultiRow, RowState, WizardStep,
};
use crate::ui::{
    CardStyle, ColorPolicy, PillRole, Role, TreeSymbol, paint, paint_bold, pill, rounded_card,
    status_tree,
};

/// Renders the current viewport of [`WizardModel`] as a pure Lipgloss string.
#[must_use]
pub fn render_wizard(model: &WizardModel, policy: ColorPolicy, max_width: usize) -> String {
    let step_badge = match model.step {
        WizardStep::Archetype => "ÉTAPE 1/4 · ARCHÉTYPE",
        WizardStep::Stack => "ÉTAPE 2/4 · LANGAGES & FRAMEWORKS",
        WizardStep::Features => "ÉTAPE 3/4 · FEATURES & GARDE-FOUS",
        WizardStep::Confirm => "ÉTAPE 4/4 · APERÇU & CONFIRMATION",
    };
    let body = match model.step {
        WizardStep::Archetype => render_archetype_step(model, policy, max_width),
        WizardStep::Stack => render_stack_step(model, policy, max_width),
        WizardStep::Features => render_features_step(model, policy, max_width),
        WizardStep::Confirm => render_confirm_step(model, policy, max_width),
    };
    format!(
        "{} {}\n{body}",
        pill("GARDE INIT", PillRole::BadgeAccent, policy),
        pill(step_badge, PillRole::BadgeMuted, policy),
    )
}

fn accent_if(active: bool, on: &str, off: &str, policy: ColorPolicy) -> String {
    if active {
        paint_bold(on, Role::Accent, policy)
    } else {
        String::from(off)
    }
}

fn render_archetype_step(m: &WizardModel, policy: ColorPolicy, max_width: usize) -> String {
    let mut lines = Vec::with_capacity(ARCHETYPE_CHOICES.len() * 2);
    for (idx, choice) in ARCHETYPE_CHOICES.iter().enumerate() {
        let active = idx == m.cursor;
        let ptr = accent_if(active, "▸", " ", policy);
        let label = accent_if(active, choice.title, choice.title, policy);
        lines.push(format!("{ptr} {label}"));
        lines.push(format!(
            "    {}",
            paint(choice.description, Role::Muted, policy)
        ));
    }
    let card = rounded_card(
        Some("Choisissez l'archétype du projet"),
        &lines,
        CardStyle::new(Role::SurfaceBorder, policy),
        max_width,
    );
    let help = paint(
        "  [↑/↓ j/k] naviguer · [Entrée] valider · [Échap] annuler",
        Role::Muted,
        policy,
    );
    format!("{card}\n{help}")
}

fn filter_banner(m: &WizardModel, policy: ColorPolicy) -> Option<String> {
    if m.input_mode == MultiInputMode::Filtering {
        Some(format!(
            "{} {}",
            pill("FILTRE", PillRole::BadgeAccent, policy),
            paint_bold(&format!("/{}_", m.filter_query), Role::Accent, policy),
        ))
    } else if !m.filter_query.is_empty() {
        Some(format!(
            "{} {}",
            pill("FILTRE", PillRole::BadgeMuted, policy),
            paint(&format!("/{}", m.filter_query), Role::Muted, policy),
        ))
    } else {
        None
    }
}

fn render_multiselect_pane(
    m: &WizardModel,
    spec: &MultiPaneSpec<'_>,
    policy: ColorPolicy,
    max_width: usize,
) -> String {
    let mut lines = Vec::new();
    if let Some(fb) = filter_banner(m, policy) {
        lines.push(fb);
    }
    if spec.rows.is_empty() {
        lines.push(paint(spec.empty_msg, Role::Muted, policy));
    }
    for (pos, row) in spec.rows.iter().enumerate() {
        let st = RowState {
            focused: pos == m.cursor,
            checked: spec.selected.contains(&row.orig_idx),
        };
        lines.push(format_check_row(st, (row.slug, row.summary), policy));
    }
    let card = rounded_card(
        Some(spec.title),
        &lines,
        CardStyle::new(Role::SurfaceBorder, policy),
        max_width,
    );
    let help = paint(
        "  [↑/↓] naviguer · [Espace/x] cocher · [a] tout · [/] filtrer · [Entrée] suivant · [Échap] annuler",
        Role::Muted,
        policy,
    );
    format!("{card}\n{help}")
}

fn render_stack_step(m: &WizardModel, policy: ColorPolicy, max_width: usize) -> String {
    let rows: Vec<MultiRow<'_>> = m
        .visible_stack_indices()
        .into_iter()
        .filter_map(|i| {
            m.stack_options.get(i).map(|t| MultiRow {
                orig_idx: i,
                slug: t.slug(),
                summary: t.summary(),
            })
        })
        .collect();
    let title = format!("Stack pour `{}`", m.selected_archetype.as_str());
    let spec = MultiPaneSpec {
        title: &title,
        empty_msg: "(aucune option ne correspond au filtre)",
        rows: &rows,
        selected: &m.selected_stack,
    };
    render_multiselect_pane(m, &spec, policy, max_width)
}

fn render_features_step(m: &WizardModel, policy: ColorPolicy, max_width: usize) -> String {
    let rows: Vec<MultiRow<'_>> = m
        .visible_feature_indices()
        .into_iter()
        .filter_map(|i| {
            FEATURE_CHOICES.get(i).map(|(f, d)| MultiRow {
                orig_idx: i,
                slug: f.as_str(),
                summary: d,
            })
        })
        .collect();
    let spec = MultiPaneSpec {
        title: "Features & Garde-fous",
        empty_msg: "(aucune feature ne correspond au filtre)",
        rows: &rows,
        selected: &m.selected_features,
    };
    render_multiselect_pane(m, &spec, policy, max_width)
}

fn format_check_row(state: RowState, labels: (&str, &str), policy: ColorPolicy) -> String {
    let ptr = accent_if(state.focused, "▸", " ", policy);
    let mark = if state.checked {
        paint_bold("[✓]", Role::Success, policy)
    } else {
        paint("[ ]", Role::Muted, policy)
    };
    let name = accent_if(state.focused, labels.0, labels.0, policy);
    format!(
        "{ptr} {mark} {name}  {}",
        paint(labels.1, Role::Muted, policy)
    )
}

fn format_confirm_prompt(choice: ConfirmChoice, policy: ColorPolicy) -> String {
    let is_yes = choice == ConfirmChoice::Yes;
    let yes_role = if is_yes {
        PillRole::BadgeOk
    } else {
        PillRole::BadgeMuted
    };
    let no_role = if is_yes {
        PillRole::BadgeMuted
    } else {
        PillRole::BadgeError
    };
    format!(
        "Initialiser le projet ?  {}  {}",
        pill("Oui", yes_role, policy),
        pill("Non", no_role, policy)
    )
}

fn render_confirm_step(m: &WizardModel, policy: ColorPolicy, max_width: usize) -> String {
    let req = m.build_request();
    let entries: Vec<(TreeSymbol, String)> = preview_init_files(&req)
        .into_iter()
        .map(|f| {
            let sym = if f == "garde.lock" {
                TreeSymbol::Sealed
            } else {
                TreeSymbol::Created
            };
            (sym, f)
        })
        .collect();
    let header = format!(
        "Cible : {} (archétype={})",
        req.root.display(),
        m.selected_archetype.as_str()
    );
    let mut lines: Vec<String> = status_tree(&header, &entries, policy)
        .lines()
        .map(String::from)
        .collect();
    lines.push(String::new());
    lines.push(format_confirm_prompt(m.confirm_choice, policy));
    let card = rounded_card(
        Some("Confirmation"),
        &lines,
        CardStyle::new(Role::Accent, policy),
        max_width,
    );
    let help = paint(
        "  [←/→ h/l] basculer · [y/n] direct · [Entrée] confirmer · [Échap] annuler",
        Role::Muted,
        policy,
    );
    format!("{card}\n{help}")
}
