//! Exhaustive algebraic unit tests for [`TypescriptViolation`], [`ReactViolation`],
//! [`LangRuleViolation`], [`TypeScriptToolsConfig`], and `tsconfig` / `.d.ts` helpers.

use std::collections::BTreeSet;

use gf_core::{
    AsyncHazardKind, ExternalToolKind, LangRuleFinding, LangRuleViolation, ReactViolation,
    RepoPath, Span, ToolConfigFinding, ToolConfigViolation, TsDebugKind, TypeScriptToolsConfig,
    TypescriptViolation, is_audited_tool_config_file, is_known_strict_preset,
    is_ts_declaration_path, is_tsconfig_filename,
};

const TS_CASES: &[(TypescriptViolation, &str, bool)] = &[
    (
        TypescriptViolation::ExplicitAny,
        "TS-NO-EXPLICIT-ANY",
        false,
    ),
    (
        TypescriptViolation::DoubleOrAngleAssertion,
        "TS-NO-DOUBLE-ASSERTION",
        false,
    ),
    (
        TypescriptViolation::NonNullOptionalChain,
        "TS-NO-NON-NULL-OPTIONAL-CHAIN",
        false,
    ),
    (
        TypescriptViolation::AsyncHazard(AsyncHazardKind::AsyncPromiseExecutor),
        "TS-NO-ASYNC-PROMISE-EXECUTOR",
        false,
    ),
    (
        TypescriptViolation::AsyncHazard(AsyncHazardKind::AsyncEffectCallback),
        "TS-NO-ASYNC-EFFECT",
        false,
    ),
    (
        TypescriptViolation::ConsoleOrDebugger(TsDebugKind::ConsoleCall),
        "TS-NO-CONSOLE-DEBUG",
        true,
    ),
    (
        TypescriptViolation::ConsoleOrDebugger(TsDebugKind::DebuggerStmt),
        "TS-NO-CONSOLE-DEBUG",
        false,
    ),
];

const REACT_CASES: &[(ReactViolation, &str)] = &[
    (ReactViolation::RulesOfHooks, "REACT-RULES-OF-HOOKS"),
    (
        ReactViolation::UnstableNestedComponent,
        "REACT-NO-UNSTABLE-NESTED-COMPONENT",
    ),
    (ReactViolation::MissingOrIndexJsxKey, "REACT-JSX-KEY"),
    (ReactViolation::DangerousJsx, "REACT-NO-DANGEROUS-JSX"),
];

#[test]
fn ts_and_react_violation_codes_remediations_and_exemptions_are_exact() {
    let mut rem_set = BTreeSet::new();
    for &(v, expected_code, expected_exempt) in TS_CASES {
        let wrapped: LangRuleViolation = v.into();
        assert_eq!(wrapped.code(), expected_code);
        assert_eq!(wrapped.to_string(), expected_code);
        assert_eq!(wrapped.is_entrypoint_exempt(), expected_exempt);
        assert!(wrapped.remediation().len() > 20);
        rem_set.insert(wrapped.remediation());
        let finding = LangRuleFinding::new(v, Span::FIRST, "ts msg");
        assert_eq!(finding.code(), expected_code);
    }
    assert_eq!(rem_set.len(), TS_CASES.len());

    let mut react_rems = BTreeSet::new();
    for &(rv, expected_code) in REACT_CASES {
        let wrapped: LangRuleViolation = rv.into();
        assert_eq!(wrapped.code(), expected_code);
        assert!(!wrapped.is_entrypoint_exempt());
        assert!(wrapped.remediation().len() > 20);
        react_rems.insert(wrapped.remediation());
        let finding = LangRuleFinding::new(rv, Span::FIRST, "react msg");
        assert_eq!(finding.code(), expected_code);
    }
    assert_eq!(react_rems.len(), REACT_CASES.len());
}

#[test]
fn biome_config_violation_and_ts_filename_helpers_are_exact() -> Result<(), gf_core::RepoPathError>
{
    let bf = ToolConfigFinding {
        violation: ToolConfigViolation::BiomeLinterDisabled,
        span: Span::FIRST,
        message: String::from("disabled"),
    };
    assert_eq!(
        (bf.tool(), bf.key()),
        (ExternalToolKind::Biome, "linter.enabled")
    );
    assert_eq!(bf.violation.to_string(), "biome:linter.enabled");
    assert!(bf.remediation().contains("enabled"));
    assert_eq!(ExternalToolKind::Oxlint.as_str(), "oxlint");
    assert_eq!(
        TypeScriptToolsConfig::default(),
        TypeScriptToolsConfig::defaults()
    );

    assert!(is_tsconfig_filename("tsconfig.json"));
    assert!(is_tsconfig_filename("tsconfig.app.json"));
    assert!(!is_tsconfig_filename("package.json"));
    assert!(is_audited_tool_config_file(&RepoPath::new(
        "apps/web/tsconfig.base.json"
    )?));
    assert!(is_audited_tool_config_file(&RepoPath::new("biome.jsonc")?));

    assert!(is_known_strict_preset("@tsconfig/strictest"));
    assert!(is_known_strict_preset("acme/tsconfig/strict"));
    assert!(is_known_strict_preset("astro/tsconfigs/strict"));
    assert!(!is_known_strict_preset("./tsconfig.base.json"));

    assert!(is_ts_declaration_path("src/types/env.d.ts"));
    assert!(is_ts_declaration_path("src/types/mod.d.mts"));
    assert!(is_ts_declaration_path("src/types/cjs.d.cts"));
    assert!(!is_ts_declaration_path("src/domain.ts"));
    Ok(())
}
