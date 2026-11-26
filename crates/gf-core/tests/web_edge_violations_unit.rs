//! Unit tests for Web, Design System & Edge sum types (`ASTRO-*`, `RR-*`, `PANDA-*`,
//! `M3E-*`, `W3C-*`, `A11Y-*`, `CF-*`, `DRIZZLE-*`, `WebEdgeConfigIssue`, `WebToolsConfig`).

use std::collections::BTreeSet;

use gf_core::{
    A11yViolation, AgentsConfig, AstroViolation, CloudflareViolation, Config, DrizzleViolation,
    ExternalToolKind, FeatureId, FrameworkId, LangRuleFinding, LangRuleViolation, Language,
    LanguageId, M3eViolation, PandaViolation, ProjectContext, ReactRouterViolation, RepoPath,
    ScannerPolicy, Span, ToolConfigFinding, ToolConfigViolation, W3cViolation, WebEdgeConfigIssue,
    WebEdgeViolation, WebToolsConfig, compose_dockerignore, compose_gitignore, glob_matches,
    is_audited_tool_config_file, is_web_edge_tool_config_file, select_contextual_skills,
};

fn all_web_edge_violations() -> Vec<WebEdgeViolation> {
    vec![
        WebEdgeViolation::Astro(AstroViolation::NoSetHtml),
        WebEdgeViolation::Astro(AstroViolation::FrontmatterClientApi),
        WebEdgeViolation::Astro(AstroViolation::NonSerializableIslandProp),
        WebEdgeViolation::Astro(AstroViolation::ClientLoadBudget),
        WebEdgeViolation::ReactRouter(ReactRouterViolation::ServerLeakToClient),
        WebEdgeViolation::ReactRouter(ReactRouterViolation::MutationInLoader),
        WebEdgeViolation::ReactRouter(ReactRouterViolation::RawInternalAnchor),
        WebEdgeViolation::ReactRouter(ReactRouterViolation::RootMissingErrorBoundary),
        WebEdgeViolation::Panda(PandaViolation::ArbitraryOrPrimitive),
        WebEdgeViolation::Panda(PandaViolation::CommittedStyledSystem),
        WebEdgeViolation::M3e(M3eViolation::TonalPairing),
        WebEdgeViolation::M3e(M3eViolation::SpringPhysicsMismatch),
        WebEdgeViolation::M3e(M3eViolation::RawElevationShapeTypo),
        WebEdgeViolation::M3e(M3eViolation::TouchTargetAndStateLayer),
        WebEdgeViolation::W3c(W3cViolation::InteractiveNesting),
        WebEdgeViolation::W3c(W3cViolation::VoidElementChildren),
        WebEdgeViolation::W3c(W3cViolation::ContentModel),
        WebEdgeViolation::W3c(W3cViolation::LandmarkAndIdUniqueness),
        WebEdgeViolation::A11y(A11yViolation::AriaInvalid),
        WebEdgeViolation::A11y(A11yViolation::HiddenFocusable),
        WebEdgeViolation::A11y(A11yViolation::TabindexAndFocusRing),
        WebEdgeViolation::A11y(A11yViolation::MediaFormMotion),
        WebEdgeViolation::Cloudflare(CloudflareViolation::BindingCoherence),
        WebEdgeViolation::Cloudflare(CloudflareViolation::GlobalScopeImpure),
        WebEdgeViolation::Cloudflare(CloudflareViolation::ForbiddenNodeApi),
        WebEdgeViolation::Cloudflare(CloudflareViolation::D1NPlusOneOrLimit),
        WebEdgeViolation::Cloudflare(CloudflareViolation::R2StreamAndKeySafety),
        WebEdgeViolation::Drizzle(DrizzleViolation::DialectMismatch),
        WebEdgeViolation::Drizzle(DrizzleViolation::UnboundedMutation),
        WebEdgeViolation::Drizzle(DrizzleViolation::RawUnsafe),
        WebEdgeViolation::Drizzle(DrizzleViolation::FkMissingIndex),
        WebEdgeViolation::Drizzle(DrizzleViolation::FloatMoney),
    ]
}

#[test]
fn web_edge_violation_codes_remediations_and_display_are_distinct() {
    let mut codes = BTreeSet::new();
    let mut remediations = BTreeSet::new();
    for v in all_web_edge_violations() {
        assert_ne!(v.code(), "");
        assert!(v.remediation().len() >= 30, "{}", v.remediation());
        assert!(codes.insert(v.code()), "duplicate code {}", v.code());
        assert!(
            remediations.insert(v.remediation()),
            "duplicate remediation for {}",
            v.code()
        );

        let lr: LangRuleViolation = v.into();
        assert_eq!(lr.code(), v.code());
        assert_eq!(lr.remediation(), v.remediation());
        assert_eq!(lr.to_string(), v.code());

        let finding = LangRuleFinding::new(v, Span::FIRST, "test");
        assert_eq!(finding.code(), v.code());
        assert_eq!(finding.remediation(), v.remediation());
    }
}

#[rustfmt::skip]
const EXPECTED_CONFIG_ISSUES: [(WebEdgeConfigIssue, ExternalToolKind, &str); 5] = [
    (WebEdgeConfigIssue::PandaStrictTokensMissingOrFalse, ExternalToolKind::Tsc, "strictTokens"),
    (WebEdgeConfigIssue::WranglerInvalidCompatibilityDate, ExternalToolKind::WranglerTypes, "compatibility_date"),
    (WebEdgeConfigIssue::WranglerPlaintextSecretInVars, ExternalToolKind::WranglerTypes, "vars"),
    (WebEdgeConfigIssue::WranglerUnguardedRemoteBinding, ExternalToolKind::WranglerTypes, "remote"),
    (WebEdgeConfigIssue::WranglerMissingNodejsCompat, ExternalToolKind::WranglerTypes, "compatibility_flags"),
];

#[test]
fn web_edge_config_issues_and_tools_config_metadata_are_exact()
-> Result<(), Box<dyn std::error::Error>> {
    for (issue, tool, key) in EXPECTED_CONFIG_ISSUES {
        assert_eq!((issue.tool(), issue.key()), (tool, key));
        assert!(issue.remediation().len() >= 30);
        let tv = ToolConfigViolation::WebEdge(issue);
        assert_eq!(tv.to_string(), format!("{}:{key}", tool.as_str()));
        let f = ToolConfigFinding {
            violation: tv,
            span: Span::FIRST,
            message: String::from("cfg"),
        };
        assert_eq!(
            (f.tool(), f.key(), f.remediation()),
            (tool, key, issue.remediation())
        );
    }
    for name in [
        "panda.config.ts",
        "panda.config.mjs",
        "wrangler.toml",
        "wrangler.jsonc",
        "wrangler.json",
    ] {
        let rp = RepoPath::new(name)?;
        assert!(is_web_edge_tool_config_file(&rp) && is_audited_tool_config_file(&rp));
    }
    assert!(!is_web_edge_tool_config_file(&RepoPath::new(
        "src/index.ts"
    )?));

    let mut cfg = Config::strict_defaults();
    assert_eq!(cfg.tools.web, WebToolsConfig::default());
    let s0 = cfg.canonical_string();
    cfg.tools.web.astro_check = ScannerPolicy::Required;
    let s1 = cfg.canonical_string();
    cfg.tools.web.html_validate = ScannerPolicy::Required;
    let s2 = cfg.canonical_string();
    cfg.tools.web.wrangler_types = ScannerPolicy::Auto;
    assert!(s0 != s1 && s1 != s2 && s2 != cfg.canonical_string());
    Ok(())
}

#[test]
fn context_aliases_and_entrypoints_cover_web_edge() {
    assert_eq!(
        LanguageId::from_config_str("astro"),
        Some(LanguageId::Astro)
    );
    assert_eq!(LanguageId::from_config_str("html"), Some(LanguageId::Html));
    assert_eq!(LanguageId::from_config_str("htm"), Some(LanguageId::Html));
    assert!(
        LanguageId::Astro.is_programming_language() && !LanguageId::Html.is_programming_language()
    );
    assert!(Language::Astro.is_always_discovered() && Language::Html.is_always_discovered());
    assert!(Language::Astro.is_code_analyzed() && !Language::Html.is_code_analyzed());

    assert_eq!(
        FrameworkId::from_config_str("react-router"),
        Some(FrameworkId::ReactRouter)
    );
    assert_eq!(
        FrameworkId::from_config_str("cloudflare-workers"),
        Some(FrameworkId::CloudflareWorkers)
    );
    assert_eq!(
        FrameworkId::from_config_str("drizzle"),
        Some(FrameworkId::Drizzle)
    );
    assert_eq!(
        FeatureId::from_config_str("panda-css"),
        Some(FeatureId::PandaCss)
    );
    assert_eq!(
        FeatureId::from_config_str("m3e"),
        Some(FeatureId::Material3Expressive)
    );
    assert_eq!(FeatureId::from_config_str("w3c"), Some(FeatureId::W3c));

    for (fw, p) in [
        (FrameworkId::Astro, "src/pages/blog/post.astro"),
        (FrameworkId::Astro, "src/content.config.ts"),
        (FrameworkId::ReactRouter, "app/root.tsx"),
        (FrameworkId::ReactRouter, "app/routes/home.tsx"),
        (FrameworkId::CloudflareWorkers, "src/worker.ts"),
        (FrameworkId::Drizzle, "drizzle.config.ts"),
    ] {
        assert!(
            fw.implicit_entrypoints().iter().any(|g| glob_matches(g, p)),
            "{fw} -> {p}"
        );
    }
}

#[test]
fn gitignore_dockerignore_and_skills_cover_web_edge() {
    let ctx = ProjectContext {
        languages: BTreeSet::from([LanguageId::Astro, LanguageId::Html]),
        features: BTreeSet::from([
            FeatureId::PandaCss,
            FeatureId::Material3Expressive,
            FeatureId::W3c,
        ]),
        frameworks: BTreeSet::from([
            FrameworkId::ReactRouter,
            FrameworkId::CloudflareWorkers,
            FrameworkId::Drizzle,
        ]),
        dockerignore_entries: None,
    };
    let (gi, di) = (compose_gitignore(&ctx), compose_dockerignore(&ctx));
    for entry in [
        "styled-system/",
        ".wrangler/",
        ".dev.vars",
        ".react-router/",
        ".astro/",
    ] {
        assert!(gi.contains(entry), "missing {entry}");
    }
    assert!(di.contains("styled-system") && di.contains(".wrangler"));

    let skills = select_contextual_skills(&ctx, &AgentsConfig::defaults());
    let ids: BTreeSet<&str> = skills.iter().map(|s| s.id.as_str()).collect();
    assert!(
        ids.contains("web-design-guidelines")
            && ids.contains("vercel-react-best-practices")
            && ids.contains("workers-best-practices")
    );
}
