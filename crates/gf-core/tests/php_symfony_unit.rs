//! Unit & mutation-killing tests for PHP 8.4, Symfony 7, Doctrine ORM 3, and Twig 3 core types.

use gf_core::{
    ExternalToolKind, FrameworkId, LangRuleViolation, LanguageId, PhpConfigIssue,
    PhpSymfonyViolation, PhpToolsConfig, PhpViolation, SymfonyViolation, ToolConfigViolation,
    TwigViolation, is_php_tool_config_file,
};

#[test]
fn php_symfony_twig_violation_codes_and_remediations_are_unique() {
    let all = [
        PhpSymfonyViolation::Php(PhpViolation::MissingStrictTypes),
        PhpSymfonyViolation::Php(PhpViolation::UnsafeExecOrUnserialize),
        PhpSymfonyViolation::Php(PhpViolation::SuperglobalDirectAccess),
        PhpSymfonyViolation::Php(PhpViolation::LooseComparison),
        PhpSymfonyViolation::Php(PhpViolation::ErrorSuppressionOperator),
        PhpSymfonyViolation::Php(PhpViolation::DebugDumpLeftover),
        PhpSymfonyViolation::Symfony(SymfonyViolation::ServiceLocatorAntiPattern),
        PhpSymfonyViolation::Symfony(SymfonyViolation::RawSqlOrDqlInjection),
        PhpSymfonyViolation::Symfony(SymfonyViolation::FlushInsideLoop),
        PhpSymfonyViolation::Symfony(SymfonyViolation::MissingAccessControl),
        PhpSymfonyViolation::Symfony(SymfonyViolation::FinalDoctrineEntity),
        PhpSymfonyViolation::Symfony(SymfonyViolation::UnboundedFindAll),
        PhpSymfonyViolation::Symfony(SymfonyViolation::LegacyAnnotation),
        PhpSymfonyViolation::Twig(TwigViolation::RawFilterXss),
        PhpSymfonyViolation::Twig(TwigViolation::InlineStyleOrScript),
        PhpSymfonyViolation::Twig(TwigViolation::DumpLeftover),
        PhpSymfonyViolation::Twig(TwigViolation::UnbalancedBlock),
    ];
    let mut codes = std::collections::BTreeSet::new();
    let mut rems = std::collections::BTreeSet::new();
    for v in all {
        assert_ne!(v.code(), "");
        assert!(v.remediation().len() >= 20);
        assert_eq!(format!("{v}"), v.code());
        assert!(codes.insert(v.code()), "duplicate code {}", v.code());
        assert!(rems.insert(v.remediation()), "duplicate remediation");
        let wrapped = LangRuleViolation::from(v);
        assert_eq!(wrapped.code(), v.code());
        assert_eq!(wrapped.remediation(), v.remediation());
        assert!(!wrapped.is_entrypoint_exempt());
    }
    assert_eq!(codes.len(), 17);
    assert_eq!(rems.len(), 17);
}

#[test]
fn php_tools_config_and_context_slugs_resolve() {
    assert_eq!(LanguageId::from_slug("php"), Some(LanguageId::Php));
    assert_eq!(LanguageId::from_slug("twig"), Some(LanguageId::Twig));
    assert!(LanguageId::Php.is_programming_language());
    assert_eq!(
        FrameworkId::from_config_str("symfony"),
        Some(FrameworkId::Symfony)
    );
    assert_eq!(
        FrameworkId::from_config_str("doctrine"),
        Some(FrameworkId::Doctrine)
    );
    assert_ne!(FrameworkId::Symfony.implicit_entrypoints(), &[] as &[&str]);
    assert_eq!(ExternalToolKind::Phpstan.as_str(), "phpstan");
    assert_eq!(ExternalToolKind::Psalm.as_str(), "psalm");
    assert!(is_php_tool_config_file("composer.json"));
    assert!(is_php_tool_config_file("phpstan.neon.dist"));
    assert!(is_php_tool_config_file("psalm.xml"));
    assert!(!is_php_tool_config_file("src/App.php"));
    let cfg = PhpToolsConfig::default();
    assert_eq!(cfg.canonical_fragment(), "php(auto,off)");
    for (issue, expected_key) in [
        (
            PhpConfigIssue::PhpstanLevelTooLowOrBlanketIgnore,
            "parameters.level",
        ),
        (PhpConfigIssue::PsalmErrorLevelTooWeak, "psalm.errorLevel"),
        (PhpConfigIssue::ComposerMissingPhpOrPsr4, "autoload.psr-4"),
    ] {
        let tv = ToolConfigViolation::Php(issue);
        assert_eq!(tv.tool(), issue.tool());
        assert_eq!(issue.key(), expected_key);
        assert_eq!(tv.key(), expected_key);
        assert!(issue.remediation().len() >= 20);
        assert_eq!(tv.remediation(), issue.remediation());
    }
}
