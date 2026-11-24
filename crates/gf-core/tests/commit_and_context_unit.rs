//! Direct unit tests in `gf-core` for `commit_msg.rs` (`GF077`), `context.rs` aliases/entrypoints,
//! `skills_catalog.rs` SHA/slug boundaries, `domains.rs`, and `mutation.rs` (`GF052`).

use gf_core::{
    AgentsConfig, CommitConfig, ExternalToolId, FeatureId, ForbiddenRuleKind, FrameworkId,
    Language, LanguageId, MutationSummary, ProjectContext, ScannerPolicy, Validation,
    parse_extra_skill_spec, select_contextual_skills, truncate_replacement,
    validate_commit_message,
};

#[test]
fn validate_commit_message_enforces_header_length_and_description_boundaries() {
    let mut cfg = CommitConfig::strict_defaults();
    for blank in ["", "   \n\t\r\n  ", "# git comment\n   # indented\r\n"] {
        match validate_commit_message(blank, &cfg) {
            Validation::Valid(()) => assert_eq!(blank, "unreachable"),
            Validation::Invalid(d) => assert_eq!((d[0].measured, d[0].threshold), (0, 1)),
        }
    }

    let commented = "# comment line\r\n   \r\nfeat(core): add strict parser\r\n# diff";
    assert_eq!(
        validate_commit_message(commented, &cfg),
        Validation::Valid(())
    );

    cfg.max_header_len = 20;
    assert_eq!(
        validate_commit_message("feat: 12345678901234", &cfg),
        Validation::Valid(())
    );
    match validate_commit_message("feat: 123456789012345", &cfg) {
        Validation::Valid(()) => assert_eq!(21, 20),
        Validation::Invalid(d) => assert_eq!((d[0].measured, d[0].threshold), (21, 20)),
    }

    cfg.max_header_len = 100;
    assert_eq!(
        validate_commit_message("feat: abcde", &cfg),
        Validation::Valid(())
    );
    match validate_commit_message("feat: abcd", &cfg) {
        Validation::Valid(()) => assert_eq!(4, 5),
        Validation::Invalid(d) => assert_eq!((d[0].measured, d[0].threshold), (4, 5)),
    }
    for vague in ["feat: wip", "fix: Fix.", "chore:   stuff...  "] {
        assert!(!validate_commit_message(vague, &cfg).is_valid(), "{vague}");
    }
}

#[test]
fn validate_commit_message_enforces_prefixes_kebab_scopes_and_allowed_scopes() {
    let mut cfg = CommitConfig::strict_defaults();
    for valid in [
        "feat!: breaking api change",
        "feat(core)!: breaking scope change",
        "fix(gf-core/v2): handle nested spans",
        "refactor(core,rules): unify validators",
    ] {
        assert_eq!(
            validate_commit_message(valid, &cfg),
            Validation::Valid(()),
            "{valid}"
        );
    }

    for invalid in [
        "feat(core):missing-space-after-colon",
        "no-colon-at-all",
        "unknown: valid summary",
        "feat(): valid summary",
        "(core): valid summary",
        "feat(unclosed: valid summary",
        "feat)bad: valid summary",
        ": valid summary",
        "feat(-lead): valid summary",
        "feat(trail-): valid summary",
        "feat(double--dash): valid summary",
        "feat(Upper): valid summary",
        "feat(a,,b): valid summary",
        "feat(bad_under): valid summary",
    ] {
        assert!(
            !validate_commit_message(invalid, &cfg).is_valid(),
            "{invalid}"
        );
    }

    for empty_pair in ["feat(): valid summary", "(core): valid summary"] {
        match validate_commit_message(empty_pair, &cfg) {
            Validation::Valid(()) => assert_eq!(empty_pair, "unreachable"),
            Validation::Invalid(d) => assert!(d[0].message.contains("empty commit type or scope")),
        }
    }
    cfg.allowed_scopes = vec![String::from("core"), String::from("rules")];
    assert_eq!(
        validate_commit_message("feat(core,rules): allowed pair", &cfg),
        Validation::Valid(())
    );
    assert!(!validate_commit_message("feat(core,cli): cli disallowed", &cfg).is_valid());
}

#[test]
fn context_enums_round_trip_aliases_display_and_entrypoints() {
    for l in [LanguageId::Rust, LanguageId::TypeScript, LanguageId::Python] {
        assert_eq!(LanguageId::from_config_str(l.as_str()), Some(l));
        assert_eq!(
            LanguageId::from_config_str(&l.as_str().to_ascii_uppercase()),
            Some(l)
        );
        assert_eq!(l.to_string(), l.as_str());
    }
    assert_eq!(LanguageId::from_config_str("cobol"), None);

    for f in [
        FeatureId::Docker,
        FeatureId::GitHooks,
        FeatureId::CiGithub,
        FeatureId::TaskRunner,
        FeatureId::Ui,
        FeatureId::I18n,
        FeatureId::Content,
        FeatureId::Tdd,
        FeatureId::Storybook,
    ] {
        assert_eq!(FeatureId::from_config_str(f.as_str()), Some(f));
        assert_eq!(f.to_string(), f.as_str());
    }
    assert_eq!(FeatureId::from_config_str("unknown"), None);

    let mut seen_ep = 0_usize;
    for fw in [
        FrameworkId::CargoWorkspace,
        FrameworkId::React,
        FrameworkId::Astro,
        FrameworkId::NextJs,
        FrameworkId::FastApi,
        FrameworkId::Django,
    ] {
        assert_eq!(FrameworkId::from_config_str(fw.as_str()), Some(fw));
        assert_eq!(fw.to_string(), fw.as_str());
        assert_ne!(fw.implicit_entrypoints(), [] as [&str; 0]);
        seen_ep += fw.implicit_entrypoints().len();
    }
    assert!(seen_ep >= 24);
    assert_eq!(FrameworkId::from_config_str("rails"), None);
    assert_eq!(
        ScannerPolicy::from_config_str("FALSE"),
        Some(ScannerPolicy::Off)
    );
    assert_eq!(ScannerPolicy::from_config_str("bogus"), None);
}

#[test]
fn skills_catalog_sha_and_selection_boundaries() -> Result<(), &'static str> {
    for bad in [
        "no-at-sign",
        "/repo@skill",
        "org/@skill",
        "org/a/b@skill",
        "org/..@skill",
        "org/repo@",
        "org/repo@bad.id",
        "org/repo@sk#123456",
        "org/repo@sk#123456g",
    ] {
        assert!(parse_extra_skill_spec(bad).is_err(), "{bad}");
    }
    assert!(parse_extra_skill_spec(&format!("org/repo@sk#{}", "a".repeat(65))).is_err());

    let seven = parse_extra_skill_spec("  org/repo@my_skill-1#abcdef0  ")?;
    assert_eq!(seven.pinned_rev.as_deref(), Some("abcdef0"));
    assert_eq!(seven.skill_path, "skills/my_skill-1");
    let sixty_four = parse_extra_skill_spec(&format!("org/repo@sk#{}", "f".repeat(64)))?;
    assert_eq!(sixty_four.pinned_rev.as_deref().map(str::len), Some(64));

    let mut ctx = ProjectContext::default();
    ctx.languages
        .extend([LanguageId::Rust, LanguageId::Python, LanguageId::TypeScript]);
    ctx.frameworks.insert(FrameworkId::NextJs);
    let mut ag = AgentsConfig::defaults();
    ag.disabled_skills = vec![String::from("ast-grep")];
    ag.extra_skills = vec![String::from("acme/tools@custom-lint"), String::from("bad")];
    let selected = select_contextual_skills(&ctx, &ag);
    assert!(!selected.iter().any(|s| s.id == "ast-grep"));
    for id in [
        "property-based-testing",
        "rust-best-practices",
        "rust-review",
        "modern-python",
        "web-design-guidelines",
        "vercel-react-best-practices",
        "custom-lint",
    ] {
        assert!(selected.iter().any(|s| s.id == id), "missing {id}");
    }
    Ok(())
}

#[test]
fn mutation_summary_and_truncation_exact_boundaries() {
    let empty_mut = MutationSummary {
        unviable: 9,
        ..MutationSummary::default()
    };
    assert_eq!(
        (empty_mut.total_viable(), empty_mut.score_percent_floor()),
        (0, None)
    );
    let scored = MutationSummary {
        caught: 7,
        timeout: 2,
        missed: 1,
        unviable: 50,
        survivors: Vec::new(),
    };
    assert_eq!(
        (
            scored.total_viable(),
            scored.total_neutralized(),
            scored.score_percent_floor()
        ),
        (10, 9, Some(90))
    );
    let s120 = "a".repeat(120);
    assert_eq!(truncate_replacement(&s120), s120);
    let s121 = truncate_replacement(&"a".repeat(121));
    assert_eq!(s121.len(), 120);
    assert!(s121.ends_with("..."));
    assert_eq!(ForbiddenRuleKind::Unsafe.as_str(), "unsafe");
    assert_eq!(ExternalToolId::Ruff.as_str(), "ruff");
    assert!(Language::Rust.is_code_analyzed() && Language::Rust.is_always_discovered());
    assert!(!Language::Css.is_code_analyzed() && Language::Css.is_always_discovered());
    assert!(!Language::Json.is_code_analyzed() && !Language::Json.is_always_discovered());
}
