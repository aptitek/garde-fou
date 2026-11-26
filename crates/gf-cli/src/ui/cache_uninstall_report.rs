//! Pure Lipgloss formatters for `garde cache` and `garde uninstall` / `garde eject`.

use gf_engine::{
    CacheCleanSummary, CacheReport, UninstallAction, UninstallExecReport, UninstallPlan,
};

use super::box_layout::{CardStyle, TreeSymbol, kv_row, rounded_card, status_tree};
use super::policy::ColorPolicy;
use super::theme::{PillRole, Role, paint, paint_bold, pill};

/// Formats a byte count into human-readable `B` / `KiB` / `MiB`.
#[must_use]
pub fn format_bytes_human(bytes: u64) -> String {
    const KIB: u64 = 1024;
    const MIB: u64 = 1024 * 1024;
    if bytes >= MIB {
        let tenths = (bytes.saturating_mul(10)) / MIB;
        format!("{}.{} MiB", tenths / 10, tenths % 10)
    } else if bytes >= KIB {
        let tenths = (bytes.saturating_mul(10)) / KIB;
        format!("{}.{} KiB", tenths / 10, tenths % 10)
    } else {
        format!("{bytes} B")
    }
}

fn cache_status_rows(report: &CacheReport, policy: ColorPolicy) -> Vec<String> {
    let stale_list = if report.stale_schemas.is_empty() {
        String::from("aucun")
    } else {
        report.stale_schemas.join(", ")
    };
    let active_val = format!(
        "{} ({})",
        report.active_entries,
        format_bytes_human(report.active_bytes)
    );
    let stale_val = format!(
        "{stale_list} — {} entrées ({})",
        report.stale_entries,
        format_bytes_human(report.stale_bytes)
    );
    let orphan_val = format!(
        "{} ({})",
        report.orphan_entries,
        format_bytes_human(report.orphan_bytes)
    );
    let total_val = format!(
        "{} entrées · {}",
        report.total_entries(),
        format_bytes_human(report.total_bytes())
    );
    vec![
        kv_row(
            "dossier",
            &report.cache_root.display().to_string(),
            14,
            policy,
        ),
        kv_row("schéma actif", &report.active_schema, 14, policy),
        kv_row("entrées actives", &active_val, 14, policy),
        kv_row("schémas anciens", &stale_val, 14, policy),
        kv_row("orphelines", &orphan_val, 14, policy),
        kv_row("total disque", &total_val, 14, policy),
    ]
}

/// Renders `garde cache status` as a Lipgloss metric card.
#[must_use]
pub fn render_cache_status_human(
    report: &CacheReport,
    policy: ColorPolicy,
    max_width: usize,
) -> String {
    let lines = cache_status_rows(report, policy);
    let card = rounded_card(
        Some("Cache BLAKE3"),
        &lines,
        CardStyle::new(Role::Accent, policy),
        max_width,
    );
    format!(
        "{} {}\n{card}",
        pill("GARDE CACHE", PillRole::BadgeAccent, policy),
        paint_bold("status", Role::Accent, policy),
    )
}

/// Renders `garde cache prune` or `garde cache clean` summary.
#[must_use]
pub fn render_cache_clean_human(
    op: &str,
    summary: &CacheCleanSummary,
    policy: ColorPolicy,
) -> String {
    let header = format!(
        "{} {}",
        pill("✓ GARDE", PillRole::BadgeOk, policy),
        paint_bold(&format!("cache {op} ok"), Role::Success, policy),
    );
    let entries = vec![
        (
            TreeSymbol::Created,
            format!("{} entrée(s) supprimée(s)", summary.removed_entries),
        ),
        (
            TreeSymbol::Updated,
            format!("{} dossier(s) supprimé(s)", summary.removed_dirs),
        ),
        (
            TreeSymbol::Sealed,
            format!("{} libérés", format_bytes_human(summary.freed_bytes)),
        ),
    ];
    status_tree(&header, &entries, policy)
}

fn describe_uninstall_action(action: &UninstallAction) -> (TreeSymbol, String) {
    match action {
        UninstallAction::DeleteFile(rel) => {
            (TreeSymbol::Warn, format!("supprimer fichier : {rel}"))
        }
        UninstallAction::DeleteDir(rel) => {
            (TreeSymbol::Warn, format!("supprimer dossier : {rel}/"))
        }
        UninstallAction::StripManagedBlock(rel) => (
            TreeSymbol::Updated,
            format!("retirer bloc géré : {rel} (contenu humain préservé)"),
        ),
        UninstallAction::PreserveCustom(rel) => {
            (TreeSymbol::Intact, format!("préserver custom : {rel}"))
        }
        UninstallAction::MiseUninstall(slug) => (
            TreeSymbol::Warn,
            format!("mise uninstall {slug} && mise reshim"),
        ),
        UninstallAction::RemoveSelfBinary(p) => (
            TreeSymbol::Warn,
            format!("supprimer binaire : {}", p.display()),
        ),
        UninstallAction::NoteSecondaryBinary(p) => (
            TreeSymbol::Intact,
            format!("binaire secondaire détecté : {}", p.display()),
        ),
    }
}

/// Renders an [`UninstallPlan`] card (used for `--dry-run` and interactive confirmation).
#[must_use]
pub fn render_uninstall_plan_human(
    plan: &UninstallPlan,
    dry_run: bool,
    policy: ColorPolicy,
    max_width: usize,
) -> String {
    let mode = if dry_run {
        "DRY-RUN (aucune modification)"
    } else {
        "PLAN D'ACTION"
    };
    let header = format!(
        "{} {}",
        pill("GARDE UNINSTALL", PillRole::BadgeWarn, policy),
        paint_bold(mode, Role::Warning, policy),
    );
    if plan.is_empty() {
        return format!(
            "{header}\n  {}",
            paint("Aucun artefact Garde-fou détecté.", Role::Muted, policy)
        );
    }
    let items: Vec<(TreeSymbol, String)> =
        plan.actions.iter().map(describe_uninstall_action).collect();
    let tree_lines: Vec<String> = status_tree("Actions prévues :", &items, policy)
        .lines()
        .map(String::from)
        .collect();
    let card = rounded_card(
        Some("Plan de désinstallation"),
        &tree_lines,
        CardStyle::new(Role::Warning, policy),
        max_width,
    );
    format!("{header}\n{card}")
}

/// Renders the completed [`UninstallExecReport`] tree.
#[must_use]
pub fn render_uninstall_exec_human(report: &UninstallExecReport, policy: ColorPolicy) -> String {
    let header = format!(
        "{} {}",
        pill("✓ GARDE", PillRole::BadgeOk, policy),
        paint_bold("désinstallation terminée", Role::Success, policy),
    );
    let mut items: Vec<(TreeSymbol, String)> = report
        .applied
        .iter()
        .map(describe_uninstall_action)
        .collect();
    for w in &report.warnings {
        items.push((TreeSymbol::Warn, w.clone()));
    }
    status_tree(&header, &items, policy)
}
