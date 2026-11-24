//! Exhaustive algebraic unit tests for [`PythonViolation`], [`ToolConfigViolation`],
//! [`ExternalToolKind`], [`LangRuleFinding`], and [`ToolConfigFinding`].

use std::collections::BTreeSet;

use gf_core::{
    DebugArtifactKind, ExceptViolationKind, ExternalToolKind, LangRuleFinding, MutantsConfigIssue,
    MypyConfigIssue, PytestCoverageIssue, PythonViolation, RuffConfigIssue, Span,
    StrykerConfigIssue, ToolConfigFinding, ToolConfigViolation, TsconfigIssue, UnsafeExecKind,
};

const PY_CASES: &[(PythonViolation, &str, bool)] = &[
    (PythonViolation::MutableDefaultArg, "PY-B006", false),
    (
        PythonViolation::BareOrSilentExcept(ExceptViolationKind::Bare),
        "PY-E722",
        false,
    ),
    (
        PythonViolation::BareOrSilentExcept(ExceptViolationKind::Silent),
        "PY-B001",
        false,
    ),
    (
        PythonViolation::UnsafeExecution(UnsafeExecKind::DynamicEval),
        "PY-S102",
        false,
    ),
    (
        PythonViolation::UnsafeExecution(UnsafeExecKind::UnsafePickle),
        "PY-S301",
        false,
    ),
    (
        PythonViolation::UnsafeExecution(UnsafeExecKind::UnsafeYamlLoad),
        "PY-S506",
        false,
    ),
    (
        PythonViolation::UnsafeExecution(UnsafeExecKind::ShellSubprocess),
        "PY-S602",
        false,
    ),
    (PythonViolation::ForbiddenAny, "PY-ANN401", false),
    (
        PythonViolation::MissingPublicTypeAnnotation,
        "PY-ANN201",
        false,
    ),
    (
        PythonViolation::LeftoverDebugArtifact(DebugArtifactKind::PrintCall),
        "PY-T201",
        true,
    ),
    (
        PythonViolation::LeftoverDebugArtifact(DebugArtifactKind::DebuggerTrace),
        "PY-T100",
        false,
    ),
];

const CFG_CASES_A: &[(ToolConfigViolation, ExternalToolKind, &str)] = &[
    (
        ToolConfigViolation::MypyNotStrict(MypyConfigIssue::MissingOrFalseStrict),
        ExternalToolKind::Mypy,
        "tool.mypy.strict",
    ),
    (
        ToolConfigViolation::MypyNotStrict(MypyConfigIssue::IgnoreErrors),
        ExternalToolKind::Mypy,
        "tool.mypy.ignore_errors",
    ),
    (
        ToolConfigViolation::MypyNotStrict(MypyConfigIssue::DisallowUntypedDefsFalse),
        ExternalToolKind::Mypy,
        "tool.mypy.disallow_untyped_defs",
    ),
    (
        ToolConfigViolation::MypyNotStrict(MypyConfigIssue::WarnReturnAnyFalse),
        ExternalToolKind::Mypy,
        "tool.mypy.warn_return_any",
    ),
    (
        ToolConfigViolation::MypyNotStrict(MypyConfigIssue::ProductionOverrideWeakened),
        ExternalToolKind::Mypy,
        "tool.mypy.overrides",
    ),
    (
        ToolConfigViolation::RuffWeakened(RuffConfigIssue::PyprojectBlanketIgnore),
        ExternalToolKind::Ruff,
        "tool.ruff.ignore",
    ),
    (
        ToolConfigViolation::RuffWeakened(RuffConfigIssue::PyprojectBlanketExtendIgnore),
        ExternalToolKind::Ruff,
        "tool.ruff.extend-ignore",
    ),
    (
        ToolConfigViolation::RuffWeakened(RuffConfigIssue::PyprojectEmptySelect),
        ExternalToolKind::Ruff,
        "tool.ruff.select",
    ),
    (
        ToolConfigViolation::RuffWeakened(RuffConfigIssue::StandaloneBlanketIgnore),
        ExternalToolKind::Ruff,
        "ignore",
    ),
    (
        ToolConfigViolation::RuffWeakened(RuffConfigIssue::StandaloneBlanketExtendIgnore),
        ExternalToolKind::Ruff,
        "extend-ignore",
    ),
    (
        ToolConfigViolation::RuffWeakened(RuffConfigIssue::StandaloneEmptySelect),
        ExternalToolKind::Ruff,
        "select",
    ),
];

const CFG_CASES_B: &[(ToolConfigViolation, ExternalToolKind, &str)] = &[
    (
        ToolConfigViolation::PytestOrCoverageWeakened(PytestCoverageIssue::PytestNoCov),
        ExternalToolKind::Pytest,
        "tool.pytest.ini_options.addopts",
    ),
    (
        ToolConfigViolation::PytestOrCoverageWeakened(PytestCoverageIssue::CoverageFailUnderTooLow),
        ExternalToolKind::Coverage,
        "tool.coverage.report.fail_under",
    ),
    (
        ToolConfigViolation::TsconfigNotStrict(TsconfigIssue::MissingOrFalseStrict),
        ExternalToolKind::Tsc,
        "compilerOptions.strict",
    ),
    (
        ToolConfigViolation::TsconfigNotStrict(TsconfigIssue::NoImplicitAnyFalse),
        ExternalToolKind::Tsc,
        "compilerOptions.noImplicitAny",
    ),
    (
        ToolConfigViolation::TsconfigNotStrict(TsconfigIssue::StrictNullChecksFalse),
        ExternalToolKind::Tsc,
        "compilerOptions.strictNullChecks",
    ),
    (
        ToolConfigViolation::MutantsScopeWeakened(MutantsConfigIssue::EmptyExamineGlobs),
        ExternalToolKind::CargoMutants,
        "examine_globs",
    ),
    (
        ToolConfigViolation::MutantsScopeWeakened(MutantsConfigIssue::EmptyExamineRe),
        ExternalToolKind::CargoMutants,
        "examine_re",
    ),
    (
        ToolConfigViolation::MutantsScopeWeakened(MutantsConfigIssue::BroadExcludeGlobs),
        ExternalToolKind::CargoMutants,
        "exclude_globs",
    ),
    (
        ToolConfigViolation::StrykerWeakened(StrykerConfigIssue::EmptyMutate),
        ExternalToolKind::Stryker,
        "mutate",
    ),
    (
        ToolConfigViolation::StrykerWeakened(StrykerConfigIssue::DisabledBreakThreshold),
        ExternalToolKind::Stryker,
        "thresholds.break",
    ),
];

#[test]
fn python_violation_codes_remediations_display_and_entrypoint_exemption_are_bijective() {
    let mut remediations = BTreeSet::new();
    for &(violation, expected_code, expected_exempt) in PY_CASES {
        assert_eq!(violation.code(), expected_code);
        assert_eq!(violation.to_string(), expected_code);
        assert_eq!(violation.is_entrypoint_exempt(), expected_exempt);
        assert!(violation.remediation().len() > 20);
        remediations.insert(violation.remediation());
        let finding = LangRuleFinding::new(violation, Span::FIRST, "msg");
        assert_eq!(
            (finding.code(), finding.remediation()),
            (expected_code, violation.remediation())
        );
    }
    assert_eq!(remediations.len(), PY_CASES.len());
}

#[test]
fn tool_config_violation_tools_keys_remediations_and_display_are_exact() {
    for &(violation, expected_tool, expected_key) in CFG_CASES_A.iter().chain(CFG_CASES_B.iter()) {
        let f = ToolConfigFinding {
            violation,
            span: Span::FIRST,
            message: String::from("m"),
        };
        assert_eq!((f.tool(), f.key()), (expected_tool, expected_key));
        assert_eq!(
            violation.to_string(),
            format!("{expected_tool}:{expected_key}")
        );
        assert!(f.remediation().len() > 20);
    }
    assert_eq!(ExternalToolKind::Pytest.as_str(), "pytest");
    assert_eq!(ExternalToolKind::Coverage.as_str(), "coverage");
}
