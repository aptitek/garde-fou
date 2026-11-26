//! Pure Lipgloss & visual-tree renderers for `garde init`, `garde sync`,
//! `garde lock`, `garde commit-msg`, and ephemeral TTY spinner frames.

use gf_core::{Diagnostic, Validation};
use gf_engine::{FileSyncAction, InitReport, SyncReport};

use super::box_layout::{CardStyle, TreeSymbol, kv_row, rounded_card, status_tree};
use super::policy::ColorPolicy;
use super::theme::{PillRole, Role, paint, paint_bold, pill};

const BRAILLE_FRAMES: [&str; 10] = ["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];

/// Renders a single ephemeral Braille spinner frame (`⠋ Synchronisation...`).
#[must_use]
pub fn spinner_frame(tick: usize, label: &str, policy: ColorPolicy) -> String {
    let glyph = BRAILLE_FRAMES[tick % BRAILLE_FRAMES.len()];
    format!(
        "{} {}",
        paint_bold(glyph, Role::Accent, policy),
        paint(label, Role::Muted, policy)
    )
}

/// Renders `garde init` results as a clean Lipgloss visual tree.
#[must_use]
pub fn render_init_human(report: &InitReport, policy: ColorPolicy) -> String {
    let header = format!(
        "{} {} `{}` dans {} ({})",
        pill("✓ GARDE", PillRole::BadgeOk, policy),
        paint_bold("initialized", Role::Success, policy),
        report.project_name,
        report.root.display(),
        paint(&report.context.summary_inline(), Role::Muted, policy),
    );
    let mut items: Vec<(TreeSymbol, String)> = Vec::new();
    if report.git_initialized {
        items.push((
            TreeSymbol::Created,
            String::from(".git/ (git init -b main)"),
        ));
    }
    for rel in &report.created_files {
        if rel == "garde.lock" {
            items.push((TreeSymbol::Sealed, String::from("garde.lock scellé")));
        } else {
            items.push((TreeSymbol::Created, format!("{rel} (créé)")));
        }
    }
    for rel in &report.updated_files {
        items.push((TreeSymbol::Updated, format!("{rel} (mis à jour)")));
    }
    if !report.preserved_files.is_empty() {
        items.push((
            TreeSymbol::Intact,
            format!("{} fichier(s) intact(s)", report.preserved_files.len()),
        ));
    }
    if !report.installed_skills.is_empty() {
        items.push((
            TreeSymbol::Created,
            format!("skills : {}", report.installed_skills.join(", ")),
        ));
    }
    for warn in &report.warnings {
        items.push((TreeSymbol::Warn, format!("warn: {warn}")));
    }
    status_tree(&header, &items, policy)
}

/// Renders `garde sync` results as a clean Lipgloss visual tree.
#[must_use]
pub fn render_sync_human(report: &SyncReport, policy: ColorPolicy) -> String {
    let header = format!(
        "{} {} ({})",
        pill("✓ GARDE", PillRole::BadgeOk, policy),
        paint_bold("garde: sync ok", Role::Success, policy),
        paint(&report.context.summary_inline(), Role::Muted, policy),
    );
    let mut items: Vec<(TreeSymbol, String)> = Vec::new();
    let agents_entry = match report.agents_md {
        FileSyncAction::Created => (TreeSymbol::Created, String::from("AGENTS.md (créé)")),
        FileSyncAction::Updated => (TreeSymbol::Updated, String::from("AGENTS.md (mis à jour)")),
        FileSyncAction::Unchanged => (TreeSymbol::Intact, String::from("AGENTS.md (intact)")),
    };
    items.push(agents_entry);
    if !report.installed_skills.is_empty() {
        items.push((
            TreeSymbol::Created,
            format!(".agents/skills/ ({})", report.installed_skills.join(", ")),
        ));
    }
    if !report.preserved_skills.is_empty() {
        items.push((
            TreeSymbol::Intact,
            format!("skills intacts : {}", report.preserved_skills.join(", ")),
        ));
    }
    if report.lock_updated {
        items.push((TreeSymbol::Sealed, String::from("garde.lock scellé")));
    }
    for (id, reason) in &report.rejected_skills {
        items.push((TreeSymbol::Warn, format!("rejected skill `{id}`: {reason}")));
    }
    for warn in &report.warnings {
        items.push((TreeSymbol::Warn, format!("warn: {warn}")));
    }
    status_tree(&header, &items, policy)
}

/// Renders `garde lock` confirmation with aligned BLAKE3 digest.
#[must_use]
pub fn render_lock_human(digest: &str, policy: ColorPolicy) -> String {
    let short = digest.get(0..16).unwrap_or(digest);
    let header = format!(
        "{} {}",
        pill("✓ GARDE", PillRole::BadgeOk, policy),
        paint_bold("garde.lock scellé", Role::Success, policy),
    );
    let entries = vec![
        (
            TreeSymbol::Sealed,
            String::from("garde-fou.toml -> garde.lock"),
        ),
        (TreeSymbol::Intact, kv_row("blake3", short, 8, policy)),
    ];
    status_tree(&header, &entries, policy)
}

/// Renders `garde commit-msg` validation outcome (`GF077`).
#[must_use]
pub fn render_commit_msg_human(
    outcome: &Validation<(), Diagnostic>,
    policy: ColorPolicy,
    max_width: usize,
) -> String {
    match outcome {
        Validation::Valid(()) => format!(
            "{} {}",
            pill("✓ GARDE", PillRole::BadgeOk, policy),
            paint_bold("garde: commit-msg ok", Role::Success, policy),
        ),
        Validation::Invalid(diags) => {
            let mut lines = Vec::with_capacity(diags.len() * 2);
            for d in diags {
                let code = pill(&d.rule_id, PillRole::BadgeError, policy);
                lines.push(format!("{code} {}: {}", d.path, d.message));
                lines.push(format!(
                    "{} {}",
                    paint_bold("💡 Remédiation :", Role::Accent, policy),
                    paint(&d.remediation, Role::Muted, policy)
                ));
            }
            let card = rounded_card(
                Some("Conventional Commits"),
                &lines,
                CardStyle::new(Role::Error, policy),
                max_width,
            );
            format!(
                "{} {}\n{card}",
                pill("✗ GARDE", PillRole::BadgeError, policy),
                paint_bold("garde: commit-msg FAIL", Role::Error, policy),
            )
        }
    }
}
