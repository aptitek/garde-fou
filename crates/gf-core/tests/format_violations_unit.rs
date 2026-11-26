//! Exhaustive unit tests for `gf-core` CSS, XML/SVG, TOML, and Markdown sum types,
//! `[tools.{css,markdown,toml,xml}]` policies, and config filename predicates.

use std::collections::BTreeSet;

use gf_core::{
    Config, CssToolsConfig, CssViolation, ExternalToolKind, FormatConfigIssue, LangRuleFinding,
    LangRuleViolation, Language, LanguageId, MarkdownConfigIssue, MarkdownToolsConfig,
    MarkdownViolation, RepoPath, ScannerPolicy, StylelintConfigIssue, SvgoConfigIssue,
    TaploConfigIssue, TomlToolsConfig, TomlViolation, ToolConfigViolation, XmlSvgViolation,
    XmlToolsConfig, is_audited_tool_config_file, is_format_tool_config_file,
};

type Res = Result<(), Box<dyn std::error::Error>>;

#[test]
fn css_xml_toml_markdown_violation_codes_remediations_and_token_exemptions_are_distinct() {
    let css_all = [
        CssViolation::RawColor,
        CssViolation::FallbackTokenLeak,
        CssViolation::RawDimensionOrZIndex,
        CssViolation::NoIdSelector,
        CssViolation::NoImportant,
        CssViolation::NoAtImport,
        CssViolation::NoTransitionAll,
    ];
    let mut codes = BTreeSet::new();
    for v in css_all {
        assert!(v.code().starts_with("CSS-"));
        assert_eq!(v.to_string(), v.code());
        assert!(v.remediation().len() >= 20);
        assert!(codes.insert(v.code()));
    }
    assert!(CssViolation::RawColor.allowed_in_token_file());
    assert!(CssViolation::FallbackTokenLeak.allowed_in_token_file());
    assert!(CssViolation::RawDimensionOrZIndex.allowed_in_token_file());
    assert!(CssViolation::NoImportant.allowed_in_token_file());
    assert!(CssViolation::NoAtImport.allowed_in_token_file());
    assert!(!CssViolation::NoIdSelector.allowed_in_token_file());
    assert!(!CssViolation::NoTransitionAll.allowed_in_token_file());

    for xv in [
        XmlSvgViolation::ForbidDtdXxe,
        XmlSvgViolation::SvgDangerousElement,
        XmlSvgViolation::SvgEventOrSmilXss,
        XmlSvgViolation::SvgUnsafeHrefOrUse,
        XmlSvgViolation::UnpinnedDependency,
    ] {
        assert!(xv.code().starts_with("XML-") || xv.code().starts_with("SVG-"));
        assert_eq!(xv.to_string(), xv.code());
        assert!(xv.remediation().len() >= 20);
        assert!(codes.insert(xv.code()));
        let lf = LangRuleFinding::new(xv, gf_core::Span::FIRST, "m");
        assert_eq!(lf.code(), xv.code());
        assert!(!lf.violation.is_entrypoint_exempt());
    }
}

#[test]
fn toml_and_markdown_violation_codes_and_remediations_are_distinct() {
    let mut codes = BTreeSet::new();
    for tv in [
        TomlViolation::ConfusingTableReopen,
        TomlViolation::UnpinnedGitOrWildcard,
        TomlViolation::WorkspaceBypass,
    ] {
        assert!(tv.code().starts_with("TOML-"));
        assert_eq!(tv.to_string(), tv.code());
        assert!(tv.remediation().len() >= 20);
        assert!(codes.insert(tv.code()));
        let lr: LangRuleViolation = tv.into();
        assert_eq!(lr.code(), tv.code());
        assert!(!lr.is_entrypoint_exempt());
    }

    for mv in [
        MarkdownViolation::RawHtml,
        MarkdownViolation::ForbiddenMdxLogic,
        MarkdownViolation::HeadingHierarchy,
        MarkdownViolation::EmptyOrReversedLink,
        MarkdownViolation::UnresolvedRefOrAnchor,
    ] {
        assert!(mv.code().starts_with("MD-"));
        assert_eq!(mv.to_string(), mv.code());
        assert!(mv.remediation().len() >= 20);
        assert!(codes.insert(mv.code()));
    }
}

#[test]
fn language_ids_and_discovery_flags_are_exact() {
    assert_eq!(LanguageId::from_config_str("css"), Some(LanguageId::Css));
    assert_eq!(LanguageId::from_config_str("scss"), Some(LanguageId::Css));
    assert_eq!(LanguageId::from_config_str("xml"), Some(LanguageId::Xml));
    assert_eq!(LanguageId::from_config_str("svg"), Some(LanguageId::Xml));
    assert_eq!(LanguageId::from_config_str("toml"), Some(LanguageId::Toml));
    assert_eq!(
        LanguageId::from_config_str("markdown"),
        Some(LanguageId::Markdown)
    );
    assert_eq!(
        LanguageId::from_config_str("mdx"),
        Some(LanguageId::Markdown)
    );
    assert!(!LanguageId::Css.is_programming_language());
    assert!(!LanguageId::Xml.is_programming_language());
    assert!(!LanguageId::Toml.is_programming_language());
    assert!(!LanguageId::Markdown.is_programming_language());

    assert!(Language::Xml.is_always_discovered());
    assert!(Language::Toml.is_always_discovered());
    assert!(!Language::Xml.is_code_analyzed());
}

#[test]
fn format_tool_config_issues_and_canonical_config_keys_are_exact() {
    let issues = [
        (
            FormatConfigIssue::Stylelint(StylelintConfigIssue::CriticalRuleDisabled),
            ExternalToolKind::Stylelint,
            "rules",
        ),
        (
            FormatConfigIssue::Markdown(MarkdownConfigIssue::DefaultDisabled),
            ExternalToolKind::Rumdl,
            "default",
        ),
        (
            FormatConfigIssue::Markdown(MarkdownConfigIssue::CriticalRuleDisabled),
            ExternalToolKind::Rumdl,
            "disable",
        ),
        (
            FormatConfigIssue::Taplo(TaploConfigIssue::WildcardExcluded),
            ExternalToolKind::Taplo,
            "exclude",
        ),
        (
            FormatConfigIssue::Svgo(SvgoConfigIssue::RemoveScriptsDisabled),
            ExternalToolKind::Xmllint,
            "plugins.removeScripts",
        ),
    ];
    for (iss, expected_tool, expected_key) in issues {
        let tcv = ToolConfigViolation::Formats(iss);
        assert_eq!(tcv.tool(), expected_tool);
        assert_eq!(tcv.key(), expected_key);
        assert!(tcv.remediation().len() >= 15);
    }
}

#[test]
fn format_tool_filenames_and_canonical_config_keys_are_exact() -> Res {
    for cfg_name in [
        ".stylelintrc.json",
        ".markdownlint.yaml",
        "rumdl.toml",
        "taplo.toml",
        "svgo.config.mjs",
    ] {
        let rp = RepoPath::new(cfg_name)?;
        assert!(is_format_tool_config_file(&rp));
        assert!(is_audited_tool_config_file(&rp));
    }

    let mut c1 = Config::strict_defaults();
    assert_eq!(c1.tools.css, CssToolsConfig::default());
    assert_eq!(c1.tools.markdown, MarkdownToolsConfig::default());
    assert_eq!(c1.tools.toml, TomlToolsConfig::default());
    assert_eq!(c1.tools.xml, XmlToolsConfig::default());
    let s1 = c1.canonical_string();
    assert!(s1.starts_with("v17|"));
    c1.tools.css.stylelint = ScannerPolicy::Off;
    let s2 = c1.canonical_string();
    assert_ne!(s1, s2);
    c1.tools.markdown.rumdl = ScannerPolicy::Required;
    let s3 = c1.canonical_string();
    assert_ne!(s2, s3);
    c1.tools.toml.taplo = ScannerPolicy::Off;
    let s4 = c1.canonical_string();
    assert_ne!(s3, s4);
    c1.tools.xml.xmllint = ScannerPolicy::Required;
    assert_ne!(s4, c1.canonical_string());
    Ok(())
}
