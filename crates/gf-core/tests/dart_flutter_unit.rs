//! Unit & mutation-killing tests for Dart 3.6+ and Flutter 3.x core types.

use gf_core::{
    DartConfigIssue, DartFlutterViolation, DartToolsConfig, DartViolation, ExternalToolKind,
    FlutterViolation, FrameworkId, LangRuleViolation, LanguageId, ToolConfigViolation,
    is_dart_generated_file, is_dart_tool_config_file,
};

#[test]
fn dart_flutter_violation_codes_and_remediations_are_unique() {
    let all = [
        DartFlutterViolation::Dart(DartViolation::ExplicitDynamic),
        DartFlutterViolation::Dart(DartViolation::AvoidPrint),
        DartFlutterViolation::Dart(DartViolation::EmptyCatch),
        DartFlutterViolation::Dart(DartViolation::DeprecatedDartHtml),
        DartFlutterViolation::Dart(DartViolation::IgnoreForFileBypass),
        DartFlutterViolation::Flutter(FlutterViolation::BuildContextAcrossAsyncGap),
        DartFlutterViolation::Flutter(FlutterViolation::MissingDispose),
        DartFlutterViolation::Flutter(FlutterViolation::HardcodedColorOutsideTheme),
        DartFlutterViolation::Flutter(FlutterViolation::SetStateInBuild),
        DartFlutterViolation::Flutter(FlutterViolation::MissingIconButtonTooltip),
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
    assert_eq!(codes.len(), 10);
    assert_eq!(rems.len(), 10);
}

#[test]
fn dart_tools_config_and_context_slugs_resolve() {
    assert_eq!(LanguageId::from_slug("dart"), Some(LanguageId::Dart));
    assert!(LanguageId::Dart.is_programming_language());
    assert_eq!(
        FrameworkId::from_config_str("flutter"),
        Some(FrameworkId::Flutter)
    );
    assert_ne!(FrameworkId::Flutter.implicit_entrypoints(), &[] as &[&str]);
    assert_eq!(ExternalToolKind::DartAnalyze.as_str(), "dart-analyze");
    assert!(is_dart_tool_config_file("pubspec.yaml"));
    assert!(is_dart_tool_config_file("analysis_options.yaml"));
    assert!(!is_dart_tool_config_file("lib/main.dart"));
    assert!(is_dart_generated_file("lib/user.g.dart"));
    assert!(is_dart_generated_file("lib/user.freezed.dart"));
    assert!(!is_dart_generated_file("lib/user.dart"));
    let cfg = DartToolsConfig::default();
    assert_eq!(cfg.canonical_fragment(), "dart(auto)");
    for (issue, exp_key) in [
        (
            DartConfigIssue::AnalysisOptionsWeakened,
            "analyzer.language.strict-casts",
        ),
        (
            DartConfigIssue::PubspecUnpinnedOrMissingSdk,
            "environment.sdk",
        ),
    ] {
        let tv = ToolConfigViolation::Dart(issue);
        assert_eq!(tv.tool(), ExternalToolKind::DartAnalyze);
        assert_eq!(issue.key(), exp_key);
        assert_eq!(tv.key(), exp_key);
        assert!(issue.remediation().len() >= 20);
        assert_eq!(tv.remediation(), issue.remediation());
    }
}
