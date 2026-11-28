//! Property-based and boundary tests for `demote_headings_outside_fences`,
//! `compose_agents_managed_block` tables, and `merge_agents_md` idempotence (`GF063` / `GF070`).

use gf_core::{
    AGENTS_MANAGED_END, AGENTS_MANAGED_START, Config, FeatureId, FrameworkId, LanguageId,
    ProjectContext, SkillEntry, compose_agents_managed_block, demote_headings_outside_fences,
    extract_agents_managed_block, merge_agents_md,
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
fn demote_headings_respects_levels_spaces_newlines_and_fence_lengths() {
    let levels =
        "# H1\r\n## H2\n### H3\n#### H4\n##### H5\n###### H6\n#sans_espace\n``not_fence\nplain";
    let got = demote_headings_outside_fences(levels);
    assert!(!got.starts_with('\n') && !got.contains('\r'));
    assert_eq!(
        got,
        "## H1\n### H2\n#### H3\n##### H4\n###### H5\n###### H6\n#sans_espace\n``not_fence\nplain"
    );

    let fenced = "````md\n# Inside 4-backtick\n```\n# Still inside (3 < 4)\n~~~sh\n# Still inside (~ != `)\n```` info\n# Still inside (trailing info)\n````   \n# Outside 1\n~~~sh\n# Inside tilde\n````\n# Still inside tilde\n~~~~\n# Outside 2";
    let expected = "````md\n# Inside 4-backtick\n```\n# Still inside (3 < 4)\n~~~sh\n# Still inside (~ != `)\n```` info\n# Still inside (trailing info)\n````   \n## Outside 1\n~~~sh\n# Inside tilde\n````\n# Still inside tilde\n~~~~\n## Outside 2";
    assert_eq!(demote_headings_outside_fences(fenced), expected);
}

#[test]
fn compose_agents_renders_exact_thresholds_and_domains() {
    let mut cfg = Config::strict_defaults();
    cfg.limits.max_file_lines = 251;
    cfg.limits.max_function_lines = 53;
    cfg.limits.max_parameters = 7;
    cfg.limits.max_cognitive_complexity = 11;
    cfg.limits.max_cyclomatic_complexity = 13;
    cfg.limits.max_struct_bools = 3;
    cfg.limits.min_clone_lines = 17;
    cfg.limits.min_clone_nodes = 43;
    cfg.limits.min_clone_similarity_pct = 89;
    cfg.clean_code.max_property_chain = 6;
    cfg.clean_code.max_bool_parameters = 2;
    cfg.coverage.min_line_coverage = Some(88);
    cfg.mutation.min_mutation_score = Some(97);
    cfg.domains.code_globs = vec![String::from("custom_src/**")];
    cfg.domains.scripts_globs = vec![String::from("custom_scripts/**")];
    cfg.domains.forme_globs = vec![String::from("custom_forme/**")];
    cfg.domains.forme_token_globs = vec![String::from("custom_tokens/**")];
    cfg.domains.texte_globs = vec![String::from("custom_texte/**")];
    cfg.domains.contenus_globs = vec![String::from("custom_docs/**")];
    cfg.domains.asset_globs = vec![String::from("custom_assets/**")];
    cfg.domains.max_asset_bytes = 777_777;

    let empty_skills_block = compose_agents_managed_block(&ProjectContext::default(), &cfg, &[]);
    assert!(empty_skills_block.contains("Run `garde sync` to populate"));
    for needle in [
        "`251`",
        "`53`",
        "`7`",
        "`11`",
        "`13`",
        "`3`",
        "`17` / `43` / `89%`",
        "`6` / `2`",
        "`88%`",
        "`97%`",
        "`custom_src/**`",
        "`custom_scripts/**`",
        "`custom_forme/**` (`custom_tokens/**`)",
        "`custom_texte/**`",
        "`custom_docs/**`",
        "`custom_assets/**` (`777777` B)",
    ] {
        assert!(empty_skills_block.contains(needle), "missing {needle}");
    }
}

#[test]
fn compose_agents_skills_sorting_and_edge_merges() {
    let skills = vec![
        SkillEntry {
            id: String::from("zeta"),
            summary: String::from("Pipe | escaped"),
        },
        SkillEntry {
            id: String::from("alpha"),
            summary: String::from("First skill"),
        },
    ];
    let with_skills = compose_agents_managed_block(
        &ProjectContext::default(),
        &Config::strict_defaults(),
        &skills,
    );
    assert!(with_skills.contains("| `n/a` |") && with_skills.contains("Pipe / escaped"));
    assert!(with_skills.find("`alpha`") < with_skills.find("`zeta`"));

    assert_eq!(extract_agents_managed_block("no markers"), None);
    assert_eq!(extract_agents_managed_block(AGENTS_MANAGED_START), None);
    let blank_merge = merge_agents_md(Some("   \r\n  "), "inner");
    assert!(blank_merge.starts_with("# AGENTS.md\n") && blank_merge.ends_with('\n'));
    let unmarked = merge_agents_md(Some("# Custom\r\nKeep me"), "inner");
    assert!(unmarked.starts_with("# Custom\nKeep me\n\n") && unmarked.ends_with('\n'));
    let no_tail_nl = format!("# H\n{AGENTS_MANAGED_START}\nold\n{AGENTS_MANAGED_END}");
    assert!(merge_agents_md(Some(&no_tail_nl), "inner").ends_with('\n'));
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

    let mut ctx_v2 = ctx_v1;
    ctx_v2.features.extend([FeatureId::Docker, FeatureId::Ui]);
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
    tr.run(
        &((0u8..8u8, 0u16..512u16), 0u8..64u8),
        |((l_m, f_m), fw_m)| {
            let mut ctx = ProjectContext::default();
            for (i, &l) in all_langs.iter().enumerate() {
                if (l_m >> i) & 1 == 1 {
                    ctx.languages.insert(l);
                }
            }
            for (i, &f) in all_feats.iter().enumerate() {
                if (f_m >> i) & 1 == 1 {
                    ctx.features.insert(f);
                }
            }
            for (i, &fw) in all_fws.iter().enumerate() {
                if (fw_m >> i) & 1 == 1 {
                    ctx.frameworks.insert(fw);
                }
            }
            let full = merge_agents_md(None, &compose_agents_managed_block(&ctx, &cfg, &[]));
            prop_assert_eq!(count_h1_outside_fences(&full), 1);
            Ok(())
        },
    )
    .map_err(|e| e.to_string())
}
