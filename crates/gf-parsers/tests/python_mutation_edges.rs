//! Mutation-killing boundary tests across Python typing/exports/calls/contracts,
//! `pytest` inspection, `pyproject.toml`/`ruff.toml` float & override edges,
//! `ToolParseError`, and relative/submodule Python import resolution.

use std::path::Path;

use gf_core::{
    Config, ContractImpl, ExternalToolKind, LangRuleFinding, ProjectContext, RepoPath,
    RuffConfigIssue, ToolConfigViolation,
};
use gf_parsers::{
    ToolParseError, parse_mypy_ndjson, parse_pyright_json, parse_ruff_json, parse_source,
};
use gf_rules::{evaluate_document, evaluate_workspace_sorted};

type TestRes = Result<(), Box<dyn std::error::Error>>;

const EDGE_SVC_PY: &str = "\
from pickle import loads
from pdb import set_trace
import json, yaml
class PublicSvc:
    def __init_subclass__(cls, tag: str) -> None:
        pass
    @staticmethod
    def bad_static(self, count: int) -> int:
        return count
    def sep_method(self, a: int, /, *, b: int) -> int:
        def local_untyped(z):
            return z
        return local_untyped(a) + b
class _InternalSvc:
    def helper(self, untyped):
        return untyped
def check_calls(raw: bytes, text: str) -> object:
    _ = json.loads(text)
    _ = yaml.load(text, Loader=CSafeLoader)
    yaml.load(\"SafeLoader\")
    yaml.load(text, Loader=123)
    set_trace()
    return loads(raw)
";

#[test]
fn python_receivers_separators_private_classes_and_bare_imports_boundaries() -> TestRes {
    let doc = parse_source(&RepoPath::new("src/svc.py")?, EDGE_SVC_PY)?;
    let callables: Vec<(&str, u32)> = doc
        .callables
        .iter()
        .map(|c| (c.name.as_str(), c.param_count))
        .collect();
    assert!(callables.contains(&("__init_subclass__", 1)));
    assert!(callables.contains(&("bad_static", 2)));
    assert!(callables.contains(&("sep_method", 2)));

    let codes: Vec<&str> = doc
        .lang_rule_findings
        .iter()
        .map(LangRuleFinding::code)
        .collect();
    assert_eq!(
        codes,
        vec!["PY-ANN201", "PY-S506", "PY-S506", "PY-T100", "PY-S301"]
    );
    assert_eq!(doc.lang_rule_findings[0].span.start_line(), 8);

    let mut cfg = Config::strict_defaults();
    cfg.entrypoints = vec![String::from("src/cli.py")];
    let cli_doc = parse_source(
        &RepoPath::new("src/cli.py")?,
        "def run() -> None:\n    print(\"ok\")\n    breakpoint()\n",
    )?;
    let cli_diags = evaluate_document(&cli_doc, &cfg);
    assert_eq!(cli_diags.len(), 1);
    assert!(cli_diags[0].message.contains("[PY-T100]"));
    Ok(())
}

#[test]
fn python_exports_multi_imports_structural_protocol_and_abcmeta_boundaries() -> TestRes {
    let multi_imp = "import pkg.alpha as pa, pkg.beta\nfrom pkg.gamma import One as O, Two\n__all__ = []\ndef ignored() -> int:\n    return 1\n";
    let doc = parse_source(&RepoPath::new("src/imp.py")?, multi_imp)?;
    let mods: Vec<&str> = doc
        .imports
        .iter()
        .map(|i| i.module_source.as_str())
        .collect();
    assert_eq!(mods, vec!["pkg.alpha", "pkg.beta", "pkg.gamma"]);
    assert_eq!(doc.imports[2].symbols, vec!["One", "Two"]);
    assert_eq!(doc.exports, Vec::new());

    let aug_all = "__all__ = ['alpha']\n__all__ += ('beta', 'alpha')\ndef alpha() -> int:\n    return 1\ndef beta() -> int:\n    return 2\ndef hidden() -> int:\n    return 3\n";
    let aug_doc = parse_source(&RepoPath::new("src/aug.py")?, aug_all)?;
    let exp_names: Vec<&str> = aug_doc.exports.iter().map(|e| e.symbol.as_str()).collect();
    assert_eq!(exp_names, vec!["alpha", "beta"]);
    let ct = parse_source(
        &RepoPath::new("tests/conftest.py")?,
        "def h() -> int:\n    return 1\n",
    )?;
    assert_eq!(ct.exports, Vec::new());

    let contracts_py = "import abc\nfrom typing import Protocol, TypeVar\nT = TypeVar('T')\nclass Sink(Protocol[T]):\n    def push(self, item: T) -> None: ...\nclass BaseJob(metaclass=abc.ABCMeta):\n    def execute(self) -> None: ...\nclass MemSink:\n    def push(self, item: int) -> None:\n        pass\nsink_a: Sink[int] = MemSink()\n";
    let c_doc = parse_source(&RepoPath::new("src/contracts.py")?, contracts_py)?;
    assert_eq!(c_doc.contracts.len(), 2);
    assert!(c_doc.contract_impls.contains(&ContractImpl {
        contract: String::from("Sink"),
        implementor: String::from("MemSink"),
    }));
    Ok(())
}

#[test]
fn pytest_vacuous_classes_fail_calls_string_tautologies_and_parent_imports() -> TestRes {
    let py_tests = "import pytest\nclass TestVacuous:\n    pass\nclass TestWithInit:\n    def __init__(self) -> None:\n        pass\nclass TestSuite:\n    def test_fail_call(self) -> None:\n        pytest.fail('explicit')\n    def test_vacuous_raises(self) -> None:\n        with pytest.raises(ValueError):\n            pass\n    def test_quote_tautology(self) -> None:\n        assert 'same' == \"same\"\n    def test_literal_truthy(self) -> None:\n        assert 42\n";
    let t_doc = parse_source(&RepoPath::new("tests/test_edge.py")?, py_tests)?;
    let lines: Vec<u32> = evaluate_document(&t_doc, &Config::strict_defaults())
        .iter()
        .map(|d| d.span.start_line())
        .collect();
    assert_eq!(lines, vec![2, 10, 13, 15]);

    let d1 = parse_source(
        &RepoPath::new("src/pkg/sub/leaf.py")?,
        "from ..core import x\nfrom . import peer\n_ = (x, peer)\n",
    )?;
    let d2 = parse_source(
        &RepoPath::new("src/pkg/core.py")?,
        "from .sub.peer import y\nx: int = y\n",
    )?;
    let d3 = parse_source(
        &RepoPath::new("src/pkg/sub/peer.py")?,
        "from .leaf import _\ny: int = 1\n",
    )?;
    let ws = evaluate_workspace_sorted(
        &[d1, d2, d3],
        &ProjectContext::default(),
        &Config::strict_defaults(),
    );
    assert!(ws.iter().any(|d| d.rule_id == "GF071"));
    Ok(())
}

#[test]
fn standalone_ruff_and_float_coverage_boundaries() -> TestRes {
    let mut cfg = Config::strict_defaults();
    cfg.coverage.min_line_coverage = Some(85);
    let r_ok = parse_source(
        &RepoPath::new("ruff.toml")?,
        "select = []\nextend-select = [\"E\"]\n",
    )?;
    assert_eq!(evaluate_document(&r_ok, &cfg), Vec::new());

    let r_bad = parse_source(
        &RepoPath::new("ruff.toml")?,
        "extend-ignore = [\"*\"]\nselect = []\n",
    )?;
    assert_eq!(
        r_bad.tool_config_findings[0].violation,
        ToolConfigViolation::RuffWeakened(RuffConfigIssue::StandaloneBlanketExtendIgnore)
    );
    assert_eq!(
        r_bad.tool_config_findings[1].violation,
        ToolConfigViolation::RuffWeakened(RuffConfigIssue::StandaloneEmptySelect)
    );

    let c84 = parse_source(
        &RepoPath::new("pyproject.toml")?,
        "[tool.coverage.report]\nfail_under = 84.9\n",
    )?;
    let c85 = parse_source(
        &RepoPath::new("pyproject.toml")?,
        "[tool.coverage.report]\nfail_under = 85.0\n",
    )?;
    assert_eq!(evaluate_document(&c84, &cfg).len(), 1);
    assert_eq!(evaluate_document(&c85, &cfg), Vec::new());
    Ok(())
}

#[test]
fn tool_output_path_edges_and_parse_error_display_are_exact() -> TestRes {
    let root = Path::new("C:/repo");
    let neighbor = parse_ruff_json(
        r#"[{"code":null,"message":"m","filename":"C:\\repo_other\\a.py","location":null,"fix":{"message":"   "}}]"#,
        root,
    )?;
    assert_eq!(neighbor[0].path.as_str(), "repo_other/a.py");
    assert!(
        neighbor[0]
            .remediation
            .contains("Resolve the Ruff violation")
    );
    assert_eq!(
        (
            neighbor[0].line,
            neighbor[0].column,
            neighbor[0].native_code.as_str()
        ),
        (1, 1, "syntax")
    );

    let mp = parse_mypy_ndjson(
        "Found 1\n{\"file\":\"C:\\\\repo\",\"line\":null,\"column\":null,\"message\":\"m\",\"hint\":\"   \",\"code\":null,\"severity\":\"ERROR\"}\n",
        root,
    )?;
    assert!(
        mp[0]
            .remediation
            .contains("Add strict Python type annotations")
    );
    assert_eq!(
        (mp[0].path.as_str(), mp[0].line, mp[0].native_code.as_str()),
        ("pyproject.toml", 1, "type-arg")
    );

    let pr = parse_pyright_json(
        r#"{"generalDiagnostics":[{"file":"file:///Users/dev/repo/src/a.py","severity":"Error","message":"m","rule":null,"range":null}]}"#,
        Path::new("/Users/dev/repo"),
    )?;
    assert_eq!(
        (pr[0].path.as_str(), pr[0].line, pr[0].native_code.as_str()),
        ("src/a.py", 1, "type-error")
    );
    let rel_pr = parse_pyright_json(
        r#"{"generalDiagnostics":[{"file":"./a.py","severity":"Error","message":"m","rule":null,"range":null}]}"#,
        Path::new(""),
    )?;
    assert_eq!(rel_pr[0].path.as_str(), "a.py");
    assert_parse_error_messages();
    Ok(())
}

fn assert_parse_error_messages() {
    let e1 = ToolParseError::MalformedJson {
        tool: ExternalToolKind::Ruff,
        detail: String::from("eof"),
    };
    let e2 = ToolParseError::InvalidPath {
        tool: ExternalToolKind::Mypy,
        raw_path: String::from(".."),
    };
    assert!(e1.to_string().contains("malformed ruff JSON: eof"));
    assert!(e2.to_string().contains("invalid mypy file path `..`"));
}
