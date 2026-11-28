//! Exhaustive algebraic unit tests for [`PythonViolation`], [`ToolConfigViolation`],
//! [`ExternalToolKind`], [`LangRuleFinding`], and [`ToolConfigFinding`].

use std::collections::BTreeSet;

use gf_core::{
    DebugArtifactKind, ExceptViolationKind, ExternalToolKind, LangRuleFinding, MutantsConfigIssue,
    MypyConfigIssue, PytestCoverageIssue, PythonViolation, RuffConfigIssue, Span,
    StrykerConfigIssue, ToolConfigFinding, ToolConfigViolation, TsconfigIssue, UnsafeExecKind,
};

struct PyCase {
    violation: PythonViolation,
    code: &'static str,
    exempt: bool,
}

const fn py_case(violation: PythonViolation, code: &'static str, exempt: bool) -> PyCase {
    PyCase {
        violation,
        code,
        exempt,
    }
}

struct CfgCase {
    violation: ToolConfigViolation,
    tool: ExternalToolKind,
    key: &'static str,
}

const fn cfg_case(
    violation: ToolConfigViolation,
    tool: ExternalToolKind,
    key: &'static str,
) -> CfgCase {
    CfgCase {
        violation,
        tool,
        key,
    }
}

#[rustfmt::skip]
const PY_CASES: &[PyCase] = &[
    py_case(PythonViolation::MutableDefaultArg, "PY-B006", false),
    py_case(PythonViolation::BareOrSilentExcept(ExceptViolationKind::Bare), "PY-E722", false),
    py_case(PythonViolation::BareOrSilentExcept(ExceptViolationKind::Silent), "PY-B001", false),
    py_case(PythonViolation::UnsafeExecution(UnsafeExecKind::DynamicEval), "PY-S102", false),
    py_case(PythonViolation::UnsafeExecution(UnsafeExecKind::UnsafePickle), "PY-S301", false),
    py_case(PythonViolation::UnsafeExecution(UnsafeExecKind::UnsafeYamlLoad), "PY-S506", false),
    py_case(PythonViolation::UnsafeExecution(UnsafeExecKind::ShellSubprocess), "PY-S602", false),
    py_case(PythonViolation::ForbiddenAny, "PY-ANN401", false),
    py_case(PythonViolation::MissingPublicTypeAnnotation, "PY-ANN201", false),
    py_case(PythonViolation::LeftoverDebugArtifact(DebugArtifactKind::PrintCall), "PY-T201", true),
    py_case(PythonViolation::LeftoverDebugArtifact(DebugArtifactKind::DebuggerTrace), "PY-T100", false),
];

#[rustfmt::skip]
const CFG_CASES_A: &[CfgCase] = &[
    cfg_case(ToolConfigViolation::MypyNotStrict(MypyConfigIssue::MissingOrFalseStrict), ExternalToolKind::Mypy, "tool.mypy.strict"),
    cfg_case(ToolConfigViolation::MypyNotStrict(MypyConfigIssue::IgnoreErrors), ExternalToolKind::Mypy, "tool.mypy.ignore_errors"),
    cfg_case(ToolConfigViolation::MypyNotStrict(MypyConfigIssue::DisallowUntypedDefsFalse), ExternalToolKind::Mypy, "tool.mypy.disallow_untyped_defs"),
    cfg_case(ToolConfigViolation::MypyNotStrict(MypyConfigIssue::WarnReturnAnyFalse), ExternalToolKind::Mypy, "tool.mypy.warn_return_any"),
    cfg_case(ToolConfigViolation::MypyNotStrict(MypyConfigIssue::ProductionOverrideWeakened), ExternalToolKind::Mypy, "tool.mypy.overrides"),
    cfg_case(ToolConfigViolation::RuffWeakened(RuffConfigIssue::PyprojectBlanketIgnore), ExternalToolKind::Ruff, "tool.ruff.ignore"),
    cfg_case(ToolConfigViolation::RuffWeakened(RuffConfigIssue::PyprojectBlanketExtendIgnore), ExternalToolKind::Ruff, "tool.ruff.extend-ignore"),
    cfg_case(ToolConfigViolation::RuffWeakened(RuffConfigIssue::PyprojectEmptySelect), ExternalToolKind::Ruff, "tool.ruff.select"),
    cfg_case(ToolConfigViolation::RuffWeakened(RuffConfigIssue::StandaloneBlanketIgnore), ExternalToolKind::Ruff, "ignore"),
    cfg_case(ToolConfigViolation::RuffWeakened(RuffConfigIssue::StandaloneBlanketExtendIgnore), ExternalToolKind::Ruff, "extend-ignore"),
    cfg_case(ToolConfigViolation::RuffWeakened(RuffConfigIssue::StandaloneEmptySelect), ExternalToolKind::Ruff, "select"),
];

#[rustfmt::skip]
const CFG_CASES_B: &[CfgCase] = &[
    cfg_case(ToolConfigViolation::PytestOrCoverageWeakened(PytestCoverageIssue::PytestNoCov), ExternalToolKind::Pytest, "tool.pytest.ini_options.addopts"),
    cfg_case(ToolConfigViolation::PytestOrCoverageWeakened(PytestCoverageIssue::CoverageFailUnderTooLow), ExternalToolKind::Coverage, "tool.coverage.report.fail_under"),
    cfg_case(ToolConfigViolation::TsconfigNotStrict(TsconfigIssue::MissingOrFalseStrict), ExternalToolKind::Tsc, "compilerOptions.strict"),
    cfg_case(ToolConfigViolation::TsconfigNotStrict(TsconfigIssue::NoImplicitAnyFalse), ExternalToolKind::Tsc, "compilerOptions.noImplicitAny"),
    cfg_case(ToolConfigViolation::TsconfigNotStrict(TsconfigIssue::StrictNullChecksFalse), ExternalToolKind::Tsc, "compilerOptions.strictNullChecks"),
    cfg_case(ToolConfigViolation::MutantsScopeWeakened(MutantsConfigIssue::EmptyExamineGlobs), ExternalToolKind::CargoMutants, "examine_globs"),
    cfg_case(ToolConfigViolation::MutantsScopeWeakened(MutantsConfigIssue::EmptyExamineRe), ExternalToolKind::CargoMutants, "examine_re"),
    cfg_case(ToolConfigViolation::MutantsScopeWeakened(MutantsConfigIssue::BroadExcludeGlobs), ExternalToolKind::CargoMutants, "exclude_globs"),
    cfg_case(ToolConfigViolation::StrykerWeakened(StrykerConfigIssue::EmptyMutate), ExternalToolKind::Stryker, "mutate"),
    cfg_case(ToolConfigViolation::StrykerWeakened(StrykerConfigIssue::DisabledBreakThreshold), ExternalToolKind::Stryker, "thresholds.break"),
];

#[test]
fn python_violation_codes_remediations_display_and_entrypoint_exemption_are_bijective() {
    let mut remediations = BTreeSet::new();
    for c in PY_CASES {
        assert_eq!(c.violation.code(), c.code);
        assert_eq!(c.violation.to_string(), c.code);
        assert_eq!(c.violation.is_entrypoint_exempt(), c.exempt);
        assert!(c.violation.remediation().len() > 20);
        remediations.insert(c.violation.remediation());
        let finding = LangRuleFinding::new(c.violation, Span::FIRST, "msg");
        assert_eq!(
            (finding.code(), finding.remediation()),
            (c.code, c.violation.remediation())
        );
    }
    assert_eq!(remediations.len(), PY_CASES.len());
}

#[test]
fn tool_config_violation_tools_keys_remediations_and_display_are_exact() {
    for c in CFG_CASES_A.iter().chain(CFG_CASES_B.iter()) {
        let f = ToolConfigFinding {
            violation: c.violation,
            span: Span::FIRST,
            message: String::from("m"),
        };
        assert_eq!((f.tool(), f.key()), (c.tool, c.key));
        assert_eq!(c.violation.to_string(), format!("{}:{}", c.tool, c.key));
        assert!(f.remediation().len() > 20);
    }
    assert_eq!(ExternalToolKind::Pytest.as_str(), "pytest");
    assert_eq!(ExternalToolKind::Coverage.as_str(), "coverage");
}
