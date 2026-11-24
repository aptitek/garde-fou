//! Stage 1 Python symmetric positive/negative AST & `pyproject.toml` corpus (`GF070`, `GF075`, `GF080`, `GF081`),
//! proving exact rule codes, closed sum-type variants, 1-based line numbers, and lexical string/docstring immunity.

mod fixtures;

use fixtures::{CLEAN_SECURITY_PY, DATA_MODELS_IMMUNITY_PY, DIRTY_SECURITY_PY, WEAK_PYPROJECT};
use gf_core::{
    Config, DebugArtifactKind, ExceptViolationKind, MypyConfigIssue, ProjectContext,
    PytestCoverageIssue, PythonViolation, RepoPath, RuffConfigIssue, ToolConfigViolation,
    UastDocument, UnsafeExecKind,
};
use gf_parsers::parse_source;
use gf_rules::{evaluate_document, evaluate_workspace_sorted};

type TestRes = Result<(), Box<dyn std::error::Error>>;

const EXPECTED_SECURITY_FINDINGS: &[(PythonViolation, &str, u32)] = &[
    (PythonViolation::ForbiddenAny, "PY-ANN401", 3),
    (PythonViolation::ForbiddenAny, "PY-ANN401", 4),
    (
        PythonViolation::UnsafeExecution(UnsafeExecKind::DynamicEval),
        "PY-S102",
        5,
    ),
    (
        PythonViolation::UnsafeExecution(UnsafeExecKind::UnsafePickle),
        "PY-S301",
        6,
    ),
    (
        PythonViolation::UnsafeExecution(UnsafeExecKind::UnsafeYamlLoad),
        "PY-S506",
        7,
    ),
    (
        PythonViolation::UnsafeExecution(UnsafeExecKind::ShellSubprocess),
        "PY-S602",
        8,
    ),
    (
        PythonViolation::LeftoverDebugArtifact(DebugArtifactKind::PrintCall),
        "PY-T201",
        9,
    ),
    (
        PythonViolation::LeftoverDebugArtifact(DebugArtifactKind::DebuggerTrace),
        "PY-T100",
        10,
    ),
    (
        PythonViolation::LeftoverDebugArtifact(DebugArtifactKind::DebuggerTrace),
        "PY-T100",
        11,
    ),
    (
        PythonViolation::BareOrSilentExcept(ExceptViolationKind::Silent),
        "PY-B001",
        14,
    ),
    (
        PythonViolation::BareOrSilentExcept(ExceptViolationKind::Bare),
        "PY-E722",
        16,
    ),
];

fn parse_py(rel: &str, src: &str) -> Result<UastDocument, Box<dyn std::error::Error>> {
    Ok(parse_source(&RepoPath::new(rel)?, src)?)
}

fn py_codes_and_lines(doc: &UastDocument) -> Vec<(PythonViolation, &'static str, u32)> {
    doc.lang_rule_findings
        .iter()
        .map(|f| (f.violation, f.code(), f.span.start_line()))
        .collect()
}

#[test]
fn symmetric_mutable_defaults_and_public_annotations() -> TestRes {
    let cfg = Config::strict_defaults();
    let ok = "def ok(items: list[str] | None = None) -> list[str]:\n    return items or []\n";
    assert_eq!(
        evaluate_document(&parse_py("src/ok.py", ok)?, &cfg),
        Vec::new()
    );

    let bad = "def bad(items: list[str] = []) -> list[str]:\n    return items\n@cached\nasync def bad_async(cache: dict[str, int] = dict()) -> int:\n    return len(cache)\n";
    assert_eq!(
        py_codes_and_lines(&parse_py("src/bad.py", bad)?),
        vec![
            (PythonViolation::MutableDefaultArg, "PY-B006", 1),
            (PythonViolation::MutableDefaultArg, "PY-B006", 4),
        ]
    );

    let ok_cls = "class Boxed:\n    def __init__(self, count: int) -> None:\n        self.count = count\n    @classmethod\n    def from_raw(cls, raw: str) -> \"Boxed\":\n        return cls(len(raw))\ndef _priv(x):\n    return x\n";
    assert_eq!(
        evaluate_document(&parse_py("src/cls.py", ok_cls)?, &cfg),
        Vec::new()
    );

    let bad_ann = "def compute(x, y: int) -> int:\n    return int(x) + y\ndef variadic(*args, **kwargs):\n    return len(args) + len(kwargs)\n";
    assert_eq!(
        py_codes_and_lines(&parse_py("src/ann.py", bad_ann)?),
        vec![
            (PythonViolation::MissingPublicTypeAnnotation, "PY-ANN201", 1),
            (PythonViolation::MissingPublicTypeAnnotation, "PY-ANN201", 3),
            (PythonViolation::MissingPublicTypeAnnotation, "PY-ANN201", 3),
            (PythonViolation::MissingPublicTypeAnnotation, "PY-ANN201", 3),
        ]
    );
    Ok(())
}

#[test]
fn symmetric_any_unsafe_exec_except_and_debug_artifacts() -> TestRes {
    let cfg = Config::strict_defaults();
    assert_eq!(
        evaluate_document(&parse_py("src/s.py", CLEAN_SECURITY_PY)?, &cfg),
        Vec::new()
    );
    assert_eq!(
        py_codes_and_lines(&parse_py("src/h.py", DIRTY_SECURITY_PY)?),
        EXPECTED_SECURITY_FINDINGS
    );
    Ok(())
}

#[test]
fn symmetric_yagni_data_models_protocol_and_lexical_immunity() -> TestRes {
    let cfg = Config::strict_defaults();
    let clean = parse_py("src/models.py", DATA_MODELS_IMMUNITY_PY)?;
    let main = parse_py(
        "src/main.py",
        "from models import UserDto, OrderDict, Coord, Role, InvoiceSchema, literal_immunity\n_ = (UserDto, OrderDict, Coord, Role, InvoiceSchema, literal_immunity)\n",
    )?;
    assert_eq!(evaluate_document(&clean, &cfg), Vec::new());
    assert_eq!(
        evaluate_workspace_sorted(&[clean, main], &ProjectContext::default(), &cfg),
        Vec::new()
    );

    let proto = parse_py(
        "src/ports.py",
        "from typing import Protocol\nclass Repository(Protocol):\n    def save(self, x: int) -> None: ...\n",
    )?;
    let entry = parse_py(
        "src/main.py",
        "from ports import Repository\n_ = Repository\n",
    )?;
    let ws = evaluate_workspace_sorted(&[proto, entry], &ProjectContext::default(), &cfg);
    assert_eq!(ws.len(), 1);
    assert_eq!(
        (ws[0].rule_id.as_str(), ws[0].span.start_line()),
        ("GF075", 2)
    );

    let bypass = "x: int = 1  # noqa\ny: int = 2  # type: ignore\nz: int = 3  # pragma: no cover\nw: int = 4  # pyright: ignore\n";
    let bp_lines: Vec<u32> = evaluate_document(&parse_py("src/bp.py", bypass)?, &cfg)
        .into_iter()
        .map(|d| d.span.start_line())
        .collect();
    assert_eq!(bp_lines, vec![1, 2, 3, 4]);
    Ok(())
}

#[test]
fn symmetric_pyproject_and_ruff_three_families() -> TestRes {
    let mut cfg = Config::strict_defaults();
    cfg.coverage.min_line_coverage = Some(85);
    let ok_pyp = "[tool.mypy]\nstrict = true\n[[tool.mypy.overrides]]\nmodule = \"tests.*\"\ndisallow_untyped_defs = false\n[tool.ruff.lint]\nselect = [\"E\", \"F\"]\n[tool.pytest.ini_options]\naddopts = [\"--strict-markers\"]\n[tool.coverage.report]\nfail_under = 90\n";
    assert_eq!(
        evaluate_document(&parse_py("pyproject.toml", ok_pyp)?, &cfg),
        Vec::new()
    );

    let bad = parse_py("pyproject.toml", WEAK_PYPROJECT)?;
    let got: Vec<(ToolConfigViolation, u32)> = bad
        .tool_config_findings
        .iter()
        .map(|f| (f.violation, f.span.start_line()))
        .collect();
    let expected = [
        (
            ToolConfigViolation::MypyNotStrict(MypyConfigIssue::MissingOrFalseStrict),
            2,
        ),
        (
            ToolConfigViolation::MypyNotStrict(MypyConfigIssue::WarnReturnAnyFalse),
            3,
        ),
        (
            ToolConfigViolation::MypyNotStrict(MypyConfigIssue::ProductionOverrideWeakened),
            6,
        ),
        (
            ToolConfigViolation::RuffWeakened(RuffConfigIssue::PyprojectBlanketExtendIgnore),
            8,
        ),
        (
            ToolConfigViolation::RuffWeakened(RuffConfigIssue::PyprojectEmptySelect),
            9,
        ),
        (
            ToolConfigViolation::PytestOrCoverageWeakened(PytestCoverageIssue::PytestNoCov),
            11,
        ),
    ];
    assert_eq!(got, expected);
    let diags = evaluate_document(&bad, &cfg);
    assert_eq!(diags.len(), 7);
    assert!(
        diags
            .iter()
            .any(|d| d.span.start_line() == 13 && d.rule_id == "GF081")
    );
    Ok(())
}
