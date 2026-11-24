//! Strict injectivity proofs for `Config::canonical_string()`, `canonical_fragment()`,
//! `default_atomic_design_boundaries()`, and `[clean_code]` / `[tdd]` / `[rules]` accessors.

use std::collections::BTreeSet;

use gf_core::{
    BoundaryRule, CleanCodeConfig, CommitConfig, Config, DomainLayoutConfig, ExternalToolId,
    FeatureId, ForbiddenRuleKind, ForbiddenRulesConfig, FrameworkId, LanguageId, LimitsConfig,
    PythonToolsConfig, ScannerPolicy, TddConfig, TddParityGate, ToolsConfig, YagniRule,
    default_atomic_design_boundaries,
};

fn record_distinct(base_str: &str, seen: &mut BTreeSet<String>, cfg: &Config, label: &str) {
    let s = cfg.canonical_string();
    assert!(!s.is_empty(), "{label} produced empty canonical_string");
    assert_ne!(s, base_str, "{label} did not change canonical_string");
    assert!(seen.insert(s), "{label} collided in canonical_string");
}

fn verify_limits_and_domains_injectivity(base: &Config, base_s: &str, seen: &mut BTreeSet<String>) {
    assert_eq!(LimitsConfig::default(), LimitsConfig::strict_defaults());
    assert_eq!(
        DomainLayoutConfig::default(),
        DomainLayoutConfig::strict_defaults()
    );
    let limit_fns: [fn(&mut LimitsConfig); 12] = [
        |l| l.max_file_lines += 1,
        |l| l.max_function_lines += 1,
        |l| l.max_parameters += 1,
        |l| l.max_cyclomatic_complexity += 1,
        |l| l.max_cognitive_complexity += 1,
        |l| l.max_nesting_depth += 1,
        |l| l.max_imports_per_file += 1,
        |l| l.max_methods_per_type += 1,
        |l| l.max_struct_bools += 1,
        |l| l.min_clone_lines += 1,
        |l| l.min_clone_nodes += 1,
        |l| l.min_clone_similarity_pct += 1,
    ];
    for (idx, mutate) in limit_fns.into_iter().enumerate() {
        let mut c = base.clone();
        mutate(&mut c.limits);
        record_distinct(base_s, seen, &c, &format!("limits[{idx}]"));
    }

    let dom_fns: [fn(&mut DomainLayoutConfig); 8] = [
        |d| d.code_globs.push(String::from("alt_code/**")),
        |d| d.scripts_globs.push(String::from("alt_scripts/**")),
        |d| d.forme_globs.push(String::from("alt_forme/**")),
        |d| d.forme_token_globs.push(String::from("alt_tok/**")),
        |d| d.texte_globs.push(String::from("alt_txt/**")),
        |d| d.contenus_globs.push(String::from("alt_cnt/**")),
        |d| d.asset_globs.push(String::from("alt_asset/**")),
        |d| d.max_asset_bytes += 1,
    ];
    for (idx, mutate) in dom_fns.into_iter().enumerate() {
        let mut c = base.clone();
        mutate(&mut c.domains);
        record_distinct(base_s, seen, &c, &format!("domains[{idx}]"));
    }
}

fn verify_boundaries_injectivity(base: &Config, base_s: &str, seen: &mut BTreeSet<String>) {
    let base_rule = BoundaryRule {
        from: String::from("src/a/**"),
        allow: vec![String::from("src/shared/**")],
        deny: vec![String::from("src/b/**")],
        reason: String::from("layer rule"),
    };
    for (idx, b_mut) in [
        BoundaryRule {
            from: String::from("src/z/**"),
            ..base_rule.clone()
        },
        BoundaryRule {
            allow: vec![String::from("src/core/**")],
            ..base_rule.clone()
        },
        BoundaryRule {
            deny: vec![String::from("src/db/**")],
            ..base_rule.clone()
        },
        BoundaryRule {
            reason: String::from("other reason"),
            ..base_rule
        },
    ]
    .into_iter()
    .enumerate()
    {
        let mut c = base.clone();
        c.boundaries = vec![b_mut];
        record_distinct(base_s, seen, &c, &format!("boundary[{idx}]"));
    }
}

fn verify_fortress_injectivity(base: &Config, base_s: &str, seen: &mut BTreeSet<String>) {
    let fort_fns: [fn(&mut Config); 13] = [
        |c| c.integrity.require_lock = true,
        |c| _ = c.architecture.no_std_crates.insert(String::from("core_a")),
        |c| _ = c.architecture.io_allowed_crates.insert(String::from("io")),
        |c| {
            _ = c
                .architecture
                .console_allowed_crates
                .insert(String::from("cli"));
        },
        |c| c.clean_code.max_nested_ternaries = 2,
        |c| c.clean_code.max_property_chain = 9,
        |c| c.clean_code.max_bool_parameters = 3,
        |c| c.clean_code.forbid_todos = false,
        |c| _ = c.clean_code.yagni_rules.remove(&YagniRule::WildcardExports),
        |c| {
            _ = c
                .clean_code
                .yagni_rules
                .remove(&YagniRule::SingleImplTraits);
        },
        |c| c.tdd.forbid_assertion_free_tests = false,
        |c| _ = c.tdd.parity_gates.insert(TddParityGate::CompanionTest),
        |c| _ = c.tdd.parity_gates.insert(TddParityGate::ComponentStories),
    ];
    for (idx, mutate) in fort_fns.into_iter().enumerate() {
        let mut c = base.clone();
        mutate(&mut c);
        record_distinct(base_s, seen, &c, &format!("fortress[{idx}]"));
    }
}

fn verify_policies_and_gates(base: &Config, base_s: &str, seen: &mut BTreeSet<String>) {
    let pol_fns: [fn(&mut Config); 25] = [
        |c| c.commit.allowed_types = vec![String::from("feat")],
        |c| c.commit.allowed_scopes = vec![String::from("core")],
        |c| c.commit.max_header_len = 72,
        |c| c.agents.sync_policy = ScannerPolicy::Required,
        |c| c.agents.remote_skills = false,
        |c| c.agents.extra_skills = vec![String::from("o/r@s")],
        |c| c.agents.disabled_skills = vec![String::from("ast-grep")],
        |c| c.tools.python.ruff = ScannerPolicy::Off,
        |c| c.tools.python.mypy = ScannerPolicy::Required,
        |c| c.tools.python.pyright = ScannerPolicy::Auto,
        |c| c.scanners.secrets = ScannerPolicy::Required,
        |c| c.scanners.vulns = ScannerPolicy::Off,
        |c| c.coverage.min_line_coverage = Some(85),
        |c| c.coverage.min_branch_coverage = Some(70),
        |c| c.coverage.max_crap_score = Some(30),
        |c| c.coverage.lcov_path = Some(String::from("cov.info")),
        |c| c.mutation.min_mutation_score = Some(95),
        |c| c.mutation.zero_survivor_globs = vec![String::from("src/core/**")],
        |c| c.mutation.report_path = Some(String::from("out.json")),
        |c| c.rules.set_enabled(ForbiddenRuleKind::Dbg, false),
        |c| c.ignore_paths = vec![String::from("gen/**")],
        |c| c.entrypoints = vec![String::from("cmd/main.rs")],
        |c| _ = c.project.languages.insert(LanguageId::Rust),
        |c| _ = c.project.features.insert(FeatureId::Docker),
        |c| _ = c.project.frameworks.insert(FrameworkId::Astro),
    ];
    for (idx, mutate) in pol_fns.into_iter().enumerate() {
        let mut c = base.clone();
        mutate(&mut c);
        record_distinct(base_s, seen, &c, &format!("policy[{idx}]"));
    }
}

#[test]
fn config_canonical_string_is_strictly_injective_across_every_field() {
    let base = Config::default();
    assert_eq!(base, Config::strict_defaults());
    let base_s = base.canonical_string();
    let mut seen = BTreeSet::new();
    verify_limits_and_domains_injectivity(&base, &base_s, &mut seen);
    verify_boundaries_injectivity(&base, &base_s, &mut seen);
    verify_fortress_injectivity(&base, &base_s, &mut seen);
    verify_policies_and_gates(&base, &base_s, &mut seen);

    let mut ord_a = base.clone();
    ord_a.ignore_paths = vec![String::from("b"), String::from("a")];
    ord_a.entrypoints = vec![String::from("y"), String::from("x")];
    let mut ord_b = base;
    ord_b.ignore_paths = vec![String::from("a"), String::from("b")];
    ord_b.entrypoints = vec![String::from("x"), String::from("y")];
    assert_eq!(ord_a.canonical_string(), ord_b.canonical_string());
}

#[test]
fn atomic_boundaries_and_clean_code_tdd_rules_accessors_are_exact() {
    let atomic = default_atomic_design_boundaries();
    assert_eq!(atomic.len(), 3);
    assert_eq!(
        (atomic[0].from.as_str(), atomic[0].deny.len()),
        ("**/atoms/**", 8)
    );
    assert_eq!(
        (atomic[1].from.as_str(), atomic[1].deny.len()),
        ("**/molecules/**", 6)
    );
    assert_eq!(atomic[2].from, "**/organisms/**");
    assert_eq!(
        atomic[2].deny,
        vec!["**/templates/**", "**/pages/**", "**/routes/**"]
    );
    assert!(
        atomic
            .iter()
            .all(|b| b.allow.is_empty() && !b.reason.is_empty())
    );

    let mut cc = CleanCodeConfig::default();
    assert!(cc.has_wildcard_exports() && cc.has_single_impl_traits());
    cc.yagni_rules.clear();
    assert!(!cc.has_wildcard_exports() && !cc.has_single_impl_traits());
    assert_eq!(YagniRule::WildcardExports.as_str(), "wildcard_exports");
    assert_eq!(YagniRule::SingleImplTraits.as_str(), "single_impl_traits");

    let mut tdd = TddConfig::default();
    assert!(!tdd.require_companion() && !tdd.require_stories());
    tdd.parity_gates.extend([
        TddParityGate::CompanionTest,
        TddParityGate::ComponentStories,
    ]);
    assert!(tdd.require_companion() && tdd.require_stories());
    assert_eq!(TddParityGate::CompanionTest.as_str(), "companion");
    assert_eq!(TddParityGate::ComponentStories.as_str(), "stories");
    assert_eq!(CommitConfig::default(), CommitConfig::strict_defaults());
    assert_eq!(ToolsConfig::default().python, PythonToolsConfig::default());

    let mut rules = ForbiddenRulesConfig::default();
    for kind in ForbiddenRuleKind::ALL {
        assert!(rules.contains(kind) && kind.to_string() == kind.as_str());
        rules.set_enabled(kind, false);
        assert!(!rules.contains(kind));
        rules.set_enabled(kind, true);
        assert!(rules.contains(kind));
    }
    for tool in ExternalToolId::ALL {
        assert_eq!(tool.to_string(), tool.as_str());
    }
}
