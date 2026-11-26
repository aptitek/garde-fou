//! Context capsule bar and domain/rule breakdown footer for the `human` check reporter.

use std::collections::BTreeMap;
use std::fmt::Write as _;

use gf_core::{
    Diagnostic, DomainLayoutConfig, ProjectContext, WorkspaceArchetype, classify_path_for_archetype,
};

use super::box_layout::gauge_bar;
use super::policy::ColorPolicy;
use super::theme::{PillRole, Role, paint, pill};

/// Formats `ProjectContext` into compact tonal capsules for headers.
#[must_use]
pub fn render_context_capsules(
    ctx: &ProjectContext,
    lcov_tenths: Option<u32>,
    policy: ColorPolicy,
) -> String {
    let langs: Vec<&str> = ctx.languages.iter().map(|l| l.as_str()).collect();
    let feats: Vec<&str> = ctx.features.iter().map(|f| f.as_str()).collect();
    let fws: Vec<&str> = ctx.frameworks.iter().map(|fw| fw.as_str()).collect();
    let sep = paint(" · ", Role::Muted, policy);
    let arch = pill(
        &format!("arch={}", ctx.archetype.as_str()),
        PillRole::BadgeMuted,
        policy,
    );
    let mut line = format!(
        "  {arch}{sep}{}{sep}{}{sep}{}",
        paint(&format!("langs=[{}]", langs.join(",")), Role::Muted, policy),
        paint(
            &format!("features=[{}]", feats.join(",")),
            Role::Muted,
            policy
        ),
        paint(
            &format!("frameworks=[{}]", fws.join(",")),
            Role::Muted,
            policy
        ),
    );
    if let Some(pct) = lcov_tenths {
        let _ = write!(line, "{sep}LCOV {}", gauge_bar(pct, 10, policy));
    }
    line
}

/// Renders the synthetic domain & rule tally footer below diagnostic cards.
#[must_use]
pub fn render_violation_footer(
    diags: &[Diagnostic],
    archetype: WorkspaceArchetype,
    policy: ColorPolicy,
) -> String {
    let layout = DomainLayoutConfig::strict_defaults();
    let mut by_dom: BTreeMap<&str, usize> = BTreeMap::new();
    let mut by_rule: BTreeMap<&str, usize> = BTreeMap::new();
    for d in diags {
        *by_dom
            .entry(classify_path_for_archetype(&d.path, &layout, archetype).as_str())
            .or_insert(0) += 1;
        *by_rule.entry(d.rule_id.as_str()).or_insert(0) += 1;
    }
    let dom_str: Vec<String> = by_dom
        .iter()
        .map(|(k, v)| format!("{} ×{v}", k.to_ascii_uppercase()))
        .collect();
    let rule_str: Vec<String> = by_rule.iter().map(|(k, v)| format!("{k} ×{v}")).collect();
    let summary = format!(
        "  {} {}  {}  {}",
        pill("BILAN", PillRole::BadgeMuted, policy),
        dom_str.join(" · "),
        paint("│", Role::SurfaceBorder, policy),
        paint(&rule_str.join(" · "), Role::Warning, policy),
    );
    let tip = paint(
        "  Astuce : garde tui pour filtrer au clavier · garde check --format agent pour copier vers votre IA",
        Role::Muted,
        policy,
    );
    format!("{summary}\n{tip}")
}
