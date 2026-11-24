//! Pure `#![no_std]` modular `AGENTS.md` fragment composer and non-destructive managed-block merger.

use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;

use crate::agent_templates::{
    CORE_FRAGMENT, DOMAINS_FRAGMENT, feature_fragment, framework_fragment, language_fragment,
};
use crate::config::Config;
use crate::context::ProjectContext;
use crate::skills_catalog::SkillEntry;

/// Opening HTML comment marker delimiting the auto-generated `AGENTS.md` section.
pub const AGENTS_MANAGED_START: &str = "<!-- >>> garde-fou managed >>> -->";

/// Closing HTML comment marker delimiting the auto-generated `AGENTS.md` section.
pub const AGENTS_MANAGED_END: &str = "<!-- <<< garde-fou managed <<< -->";

fn is_fence_toggle(trimmed: &str) -> Option<(char, usize)> {
    let first = trimmed.chars().next()?;
    if first != '`' && first != '~' {
        return None;
    }
    let count = trimmed.chars().take_while(|&c| c == first).count();
    (count >= 3).then_some((first, count))
}

fn update_fence_state(trimmed: &str, fence: &mut Option<(char, usize)>) -> bool {
    if let Some((open_ch, open_len)) = *fence {
        if let Some((ch, len)) = is_fence_toggle(trimmed)
            && ch == open_ch
            && len >= open_len
            && trimmed[len..].trim().is_empty()
        {
            *fence = None;
        }
        return true;
    }
    if let Some(marker) = is_fence_toggle(trimmed) {
        *fence = Some(marker);
        return true;
    }
    false
}

/// Shifts ATX Markdown headings (`# ` -> `## `) by `+1` level outside fenced code blocks,
/// preserving `GF063` single-`H1` and sequential-heading invariants in the assembled `AGENTS.md`.
#[must_use]
pub fn demote_headings_outside_fences(fragment: &str) -> String {
    let mut out = String::with_capacity(fragment.len() + 16);
    let mut fence: Option<(char, usize)> = None;

    for (idx, raw_line) in fragment.trim().lines().enumerate() {
        if idx > 0 {
            out.push('\n');
        }
        let line = raw_line.trim_end_matches('\r');
        if update_fence_state(line.trim_start(), &mut fence) {
            out.push_str(line);
            continue;
        }
        let hashes = line.bytes().take_while(|&b| b == b'#').count();
        if (1..=5).contains(&hashes) && line[hashes..].starts_with(' ') {
            out.push('#');
        }
        out.push_str(line);
    }
    out
}

fn render_thresholds_table(config: &Config) -> String {
    let l = &config.limits;
    let cc = &config.clean_code;
    let cov = config
        .coverage
        .min_line_coverage
        .map_or_else(|| String::from("n/a"), |pct| format!("{pct}%"));
    let mut_score = config
        .mutation
        .min_mutation_score
        .map_or_else(|| String::from("n/a"), |pct| format!("{pct}%"));
    format!(
        "\n\n## Active Thresholds (`garde-fou.toml`)\n\n\
         | Parameter | Limit |\n\
         | :--- | :--- |\n\
         | `max_file_lines` | `{}` |\n\
         | `max_function_lines` | `{}` |\n\
         | `max_parameters` | `{}` |\n\
         | `max_cognitive_complexity` | `{}` |\n\
         | `max_cyclomatic_complexity` | `{}` |\n\
         | `max_struct_bools` | `{}` |\n\
         | `min_clone_lines` / `min_clone_nodes` / `min_clone_similarity_pct` | `{}` / `{}` / `{}%` |\n\
         | `max_property_chain` / `max_bool_parameters` | `{}` / `{}` |\n\
         | `min_line_coverage` (`LCOV`) | `{cov}` |\n\
         | `min_mutation_score` (`GF052`) | `{mut_score}` |",
        l.max_file_lines,
        l.max_function_lines,
        l.max_parameters,
        l.max_cognitive_complexity,
        l.max_cyclomatic_complexity,
        l.max_struct_bools,
        l.min_clone_lines,
        l.min_clone_nodes,
        l.min_clone_similarity_pct,
        cc.max_property_chain,
        cc.max_bool_parameters,
    )
}

fn render_domains_table(config: &Config) -> String {
    let d = &config.domains;
    format!(
        "\n\n## Active Six-Domain Globs\n\n\
         | Domain | Configured Globs |\n\
         | :--- | :--- |\n\
         | `Code` | `{}` |\n\
         | `Scripts` | `{}` |\n\
         | `Forme` (Tokens) | `{}` (`{}`) |\n\
         | `Texte` | `{}` |\n\
         | `Contenus` | `{}` |\n\
         | `Asset` (`max_asset_bytes`) | `{}` (`{}` B) |",
        d.code_globs.join(", "),
        d.scripts_globs.join(", "),
        d.forme_globs.join(", "),
        d.forme_token_globs.join(", "),
        d.texte_globs.join(", "),
        d.contenus_globs.join(", "),
        d.asset_globs.join(", "),
        d.max_asset_bytes,
    )
}

fn render_skills_index(active_skills: &[SkillEntry]) -> String {
    if active_skills.is_empty() {
        return String::from(
            "\n\n## Installed Agent Skills (`.agents/skills/`)\n\n- Run `garde sync` to populate `.agents/skills/`.\n",
        );
    }
    let mut sorted = active_skills.to_vec();
    sorted.sort();
    let rows: Vec<String> = sorted
        .into_iter()
        .map(|item| {
            format!(
                "| `{}` | `.agents/skills/{}/SKILL.md` | {} |",
                item.id,
                item.id,
                item.summary.replace('|', "/")
            )
        })
        .collect();
    format!(
        "\n\n## Installed Agent Skills (`.agents/skills/`)\n\n\
         | Skill | Path | Purpose |\n\
         | :--- | :--- | :--- |\n\
         {}\n",
        rows.join("\n")
    )
}

/// Composes the deterministic managed Markdown payload (excluding outer HTML comment markers).
#[must_use]
pub fn compose_agents_managed_block(
    ctx: &ProjectContext,
    config: &Config,
    active_skills: &[SkillEntry],
) -> String {
    let mut buf = format!(
        "## Garde-fou Managed Guardrails\n\n\
         > Auto-generated by `garde sync` for `{}`. Do not edit inside managed markers; run `garde sync` after changing project features or `garde-fou.toml`.",
        ctx.summary_inline()
    );
    buf.push_str(&render_thresholds_table(config));
    buf.push_str(&render_domains_table(config));

    let mut fragments: Vec<&str> = alloc::vec![CORE_FRAGMENT, DOMAINS_FRAGMENT];
    for &lang in &ctx.languages {
        fragments.push(language_fragment(lang));
    }
    for &feat in &ctx.features {
        fragments.push(feature_fragment(feat));
    }
    for &fw in &ctx.frameworks {
        fragments.push(framework_fragment(fw));
    }
    for frag in fragments {
        buf.push_str("\n\n");
        buf.push_str(&demote_headings_outside_fences(frag));
    }
    buf.push_str(&render_skills_index(active_skills));
    buf.trim_end().into()
}

/// Extracts the inner managed block between [`AGENTS_MANAGED_START`] and [`AGENTS_MANAGED_END`].
#[must_use]
pub fn extract_agents_managed_block(raw_agents_md: &str) -> Option<String> {
    let normalized = raw_agents_md.replace("\r\n", "\n").replace('\r', "\n");
    let (_, after_start) = normalized.split_once(AGENTS_MANAGED_START)?;
    let (inner, _) = after_start.split_once(AGENTS_MANAGED_END)?;
    Some(String::from(inner.trim()))
}

/// Non-destructively merges `managed_block` into `existing` `AGENTS.md`, preserving 100%
/// of user-authored sections outside [`AGENTS_MANAGED_START`] .. [`AGENTS_MANAGED_END`].
#[must_use]
pub fn merge_agents_md(existing: Option<&str>, managed_block: &str) -> String {
    let wrapped = format!(
        "{AGENTS_MANAGED_START}\n{}\n{AGENTS_MANAGED_END}",
        managed_block.trim()
    );
    let Some(raw) = existing.filter(|s| !s.trim().is_empty()) else {
        return format!(
            "# AGENTS.md\n\n\
             ## Project-Specific Instructions\n\n\
             Add custom architecture notes and domain invariants here (preserved across `garde sync`).\n\n\
             {wrapped}\n"
        );
    };
    let norm = raw.replace("\r\n", "\n").replace('\r', "\n");
    if let Some((before, rest)) = norm.split_once(AGENTS_MANAGED_START)
        && let Some((_, after)) = rest.split_once(AGENTS_MANAGED_END)
    {
        let mut out = String::from(before);
        out.push_str(&wrapped);
        out.push_str(after);
        if !out.ends_with('\n') {
            out.push('\n');
        }
        return out;
    }
    format!("{}\n\n{wrapped}\n", norm.trim_end())
}
