//! Property-based tests for `merge_agents_md` idempotence, custom prose preservation,
//! and single-`H1` heading invariants across all fragment combinations (`GF063` / `GF070`).

use gf_core::{
    AGENTS_MANAGED_END, AGENTS_MANAGED_START, Config, FeatureId, FrameworkId, LanguageId,
    ProjectContext, SkillEntry, compose_agents_managed_block, extract_agents_managed_block,
    merge_agents_md,
};
use proptest::prelude::*;
use proptest::test_runner::{Config as ProptestConfig, TestRunner};

fn runner() -> TestRunner {
    TestRunner::new(ProptestConfig {
        cases: 128,
        failure_persistence: None,
        ..ProptestConfig::default()
    })
}

fn count_h1_outside_fences(md: &str) -> usize {
    let mut in_fence = false;
    let mut h1s = 0_usize;
    for raw in md.lines() {
        let trimmed = raw.trim_start();
        if trimmed.starts_with("```") || trimmed.starts_with("~~~") {
            in_fence = !in_fence;
            continue;
        }
        if !in_fence && raw.starts_with("# ") {
            h1s = h1s.saturating_add(1);
        }
    }
    h1s
}

#[test]
fn merge_agents_md_is_idempotent_and_preserves_custom_sections() -> Result<(), String> {
    let cfg = Config::strict_defaults();
    let mut ctx_v1 = ProjectContext::default();
    ctx_v1.languages.insert(LanguageId::Rust);
    let skills = vec![SkillEntry {
        id: String::from("garde-fou"),
        summary: String::from("Core static analysis skill."),
    }];
    let block_v1 = compose_agents_managed_block(&ctx_v1, &cfg, &skills);

    let mut ctx_v2 = ctx_v1.clone();
    ctx_v2.features.insert(FeatureId::Docker);
    ctx_v2.features.insert(FeatureId::Ui);
    let block_v2 = compose_agents_managed_block(&ctx_v2, &cfg, &skills);

    let mut tr = runner();
    let strat = (
        prop::collection::vec("[a-zA-Z0-9 _.-]{3,24}", 0..4),
        prop::collection::vec("[a-zA-Z0-9 _.-]{3,24}", 0..4),
    );
    tr.run(&strat, |(header_lines, footer_lines)| {
        let custom_doc = format!(
            "# AGENTS.md\n\n## Custom Team Rules\n\n{}\n\n{AGENTS_MANAGED_START}\nold\n{AGENTS_MANAGED_END}\n\n{}\n",
            header_lines.join("\n"),
            footer_lines.join("\n"),
        );

        let once = merge_agents_md(Some(&custom_doc), &block_v1);
        let twice = merge_agents_md(Some(&once), &block_v1);
        prop_assert_eq!(&once, &twice);
        let extracted = extract_agents_managed_block(&once);
        prop_assert_eq!(extracted.as_deref(), Some(block_v1.as_str()));
        prop_assert_eq!(count_h1_outside_fences(&once), 1);

        let upgraded = merge_agents_md(Some(&once), &block_v2);
        prop_assert!(upgraded.contains("Container & Docker Hardening"));
        for line in header_lines.iter().chain(footer_lines.iter()) {
            prop_assert!(upgraded.contains(line.as_str()));
        }
        prop_assert_eq!(count_h1_outside_fences(&upgraded), 1);
        Ok(())
    })
    .map_err(|e| e.to_string())
}

#[test]
fn every_context_combination_produces_single_h1_agents_md() -> Result<(), String> {
    let all_langs = [LanguageId::Rust, LanguageId::TypeScript, LanguageId::Python];
    let all_feats = [
        FeatureId::Docker,
        FeatureId::GitHooks,
        FeatureId::CiGithub,
        FeatureId::TaskRunner,
        FeatureId::Ui,
        FeatureId::I18n,
        FeatureId::Content,
        FeatureId::Tdd,
        FeatureId::Storybook,
    ];
    let all_fws = [
        FrameworkId::CargoWorkspace,
        FrameworkId::React,
        FrameworkId::Astro,
        FrameworkId::NextJs,
        FrameworkId::FastApi,
        FrameworkId::Django,
    ];
    let cfg = Config::strict_defaults();
    let mut tr = runner();
    let mask_strat = (0u8..8u8, 0u16..512u16, 0u8..64u8);

    tr.run(&mask_strat, |(l_mask, f_mask, fw_mask)| {
        let mut ctx = ProjectContext::default();
        for (i, &l) in all_langs.iter().enumerate() {
            if (l_mask >> i) & 1 == 1 {
                ctx.languages.insert(l);
            }
        }
        for (i, &f) in all_feats.iter().enumerate() {
            if (f_mask >> i) & 1 == 1 {
                ctx.features.insert(f);
            }
        }
        for (i, &fw) in all_fws.iter().enumerate() {
            if (fw_mask >> i) & 1 == 1 {
                ctx.frameworks.insert(fw);
            }
        }
        let block = compose_agents_managed_block(&ctx, &cfg, &[]);
        let full = merge_agents_md(None, &block);
        prop_assert_eq!(count_h1_outside_fences(&full), 1);
        Ok(())
    })
    .map_err(|e| e.to_string())
}
