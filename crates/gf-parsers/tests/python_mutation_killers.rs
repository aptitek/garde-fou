//! Targeted unit tests in `gf-parsers` killing all remaining Python AST, `pytest`,
//! export/fixture, and `pyproject.toml`/`mutants.toml` boundary mutants.

use gf_core::{
    Config, MutantsConfigIssue, MypyConfigIssue, PytestCoverageIssue, RepoPath, ToolConfigViolation,
};
use gf_parsers::parse_source;
use gf_rules::evaluate_document;

type TestRes = Result<(), Box<dyn std::error::Error>>;

const VALID_PYTEST_FIXTURE: &str = "\
import pytest
from dataclasses import dataclass
@dataclass
class TestDataFixture:
    value: int
class HelperEmpty:
    pass
class HelperWithTest:
    def test_in_helper(self) -> None:
        assert compute() == 'expected'
class TestUnitSuite:
    def test_valid_assertions(self) -> None:
        assert compute()
        assert '' == compute()
        assert [] == compute_list()
        self.assertEqual(compute(), 'ok')
        self.assertTrue(compute())
        with pytest.raises(ValueError):
            x = compute()
    def test_only_is_not_sentinel(self) -> None:
        assert compute() is not SENTINEL
    def test_only_mock_assert(self) -> None:
        mock_svc.assert_called_once()
def test_top_valid() -> None:
    with pytest.raises(RuntimeError):
        compute()
";

const BAD_PYTEST_FIXTURE: &str = "\
import pytest
def test_no_assert() -> None:
    compute()
def test_weak_is_not_none() -> None:
    assert compute() is not None
def test_string_truthy() -> None:
    assert 'nonempty'
def test_empty_str_not_truthy() -> None:
    assert ''
def test_list_truthy() -> None:
    assert [1]
def test_empty_list_not_truthy() -> None:
    assert []
def test_vacuous_ellipsis_raises() -> None:
    with pytest.raises(ValueError):
        # comment
        ...
class TestBadUnit:
    def test_eq_taut(self) -> None:
        self.assertEqual('a', \"a\")
    def test_true_taut(self) -> None:
        self.assertTrue(True)
    def test_weak_unit(self) -> None:
        self.assertIsNotNone(compute())
";

#[test]
fn pytest_inspector_scalar_truthy_unittest_and_vacuous_class_boundaries() -> TestRes {
    let valid_doc = parse_source(&RepoPath::new("tests/test_valid.py")?, VALID_PYTEST_FIXTURE)?;
    let names: Vec<&str> = valid_doc
        .test_cases
        .iter()
        .map(|t| t.name.as_str())
        .collect();
    assert_eq!(
        names,
        vec![
            "test_in_helper",
            "TestUnitSuite::test_valid_assertions",
            "TestUnitSuite::test_only_is_not_sentinel",
            "TestUnitSuite::test_only_mock_assert",
            "test_top_valid"
        ]
    );
    assert_eq!(
        evaluate_document(&valid_doc, &Config::strict_defaults()),
        Vec::new()
    );

    let prod_test_cls = parse_source(
        &RepoPath::new("src/domain.py")?,
        "class TestDomainModel:\n    pass\n",
    )?;
    assert_eq!(prod_test_cls.test_cases, Vec::new());

    let bad_doc = parse_source(&RepoPath::new("tests/test_bad.py")?, BAD_PYTEST_FIXTURE)?;
    let lines: Vec<u32> = evaluate_document(&bad_doc, &Config::strict_defaults())
        .iter()
        .map(|d| d.span.start_line())
        .collect();
    assert_eq!(lines, vec![2, 4, 6, 10, 14, 19, 21, 23]);
    Ok(())
}

const OUTER_CLASS_PY: &str = "\
from typing import cast
class Outer:
    def m1(self) -> int:
        return 1
    @classmethod
    def m2(cls) -> int:
        return 2
    def not_cls_receiver(cls: object, x: int) -> int:
        return x
class WithDecoratedInner:
    @dec
    class Inner:
        pass
def immutable_default(items: tuple[int, ...] = tuple()) -> int:
    try:
        return cast(int, len(items))
    except ValueError:
        return 42
    except KeyError:
        return 'recovered'
";

const EXPORTS_FIXTURE_PY: &str = "\
import pytest
@decorator
def pub_dec() -> int:
    return 1
@pytest.fixture
def my_fix() -> int:
    return 2
@fixture
def bare_fix() -> int:
    return 3
class PubClass:
    pass
def _private_fn() -> int:
    return 4
";

#[test]
fn python_class_methods_exports_fixtures_cast_and_guardrail_edges() -> TestRes {
    let doc = parse_source(&RepoPath::new("src/cls.py")?, OUTER_CLASS_PY)?;
    assert_eq!(doc.types[0].name, "Outer");
    assert_eq!(doc.types[0].method_count, 3);
    assert_eq!(doc.types[1].method_count, 0);
    let ncr = doc
        .callables
        .iter()
        .find(|c| c.name == "not_cls_receiver")
        .map(|c| c.param_count);
    assert_eq!(ncr, Some(2));
    assert_eq!(doc.lang_rule_findings, Vec::new());

    let exp_doc = parse_source(&RepoPath::new("src/exp.py")?, EXPORTS_FIXTURE_PY)?;
    let syms: Vec<&str> = exp_doc.exports.iter().map(|e| e.symbol.as_str()).collect();
    assert_eq!(syms, vec!["pub_dec", "PubClass"]);

    let d_doc = parse_source(
        &RepoPath::new("src/dall.py")?,
        "__all__ = [\"\", \"alpha\"]\n\ndef alpha() -> int:\n    return 1\n",
    )?;
    assert_eq!(d_doc.exports.len(), 1);
    assert_eq!(d_doc.exports[0].symbol, "alpha");
    assert_eq!(d_doc.exports[0].span.start_line(), 3);

    for p in ["tests/foo_test.py", "tests/test_foo.py"] {
        let td = parse_source(&RepoPath::new(p)?, "def pub() -> int:\n    return 1\n")?;
        assert_eq!(td.exports, Vec::new());
    }
    Ok(())
}

#[test]
fn pyproject_mypy_overrides_section_spans_addopts_and_mutants_toml_edges() -> TestRes {
    let span_toml = "[tool.mypy]\nstrict = true\n[[tool.mypy.overrides]]\nmodule = \"pkg.core\"\nstrict = false\n[tool.other]\nignore_errors = true\n";
    let span_doc = parse_source(&RepoPath::new("pyproject.toml")?, span_toml)?;
    assert_eq!(span_doc.tool_config_findings.len(), 1);
    assert_eq!(span_doc.tool_config_findings[0].span.start_line(), 3);
    let exact_ov = "[tool.mypy]\nstrict = true\n[[tool.mypy.overrides]]\nmodule = \"pkg.core\"\nignore_errors = true\n";
    let exact_doc = parse_source(&RepoPath::new("pyproject.toml")?, exact_ov)?;
    assert_eq!(exact_doc.tool_config_findings[0].span.start_line(), 5);

    for opt in ["--no-cov=pkg", "-p no:cov", "-pno:cov"] {
        let t =
            format!("[tool.mypy]\nstrict = true\n[tool.pytest.ini_options]\naddopts = \"{opt}\"\n");
        let d = parse_source(&RepoPath::new("pyproject.toml")?, &t)?;
        assert_eq!(
            d.tool_config_findings[0].violation,
            ToolConfigViolation::PytestOrCoverageWeakened(PytestCoverageIssue::PytestNoCov)
        );
    }

    let ok_overrides = "[tool.mypy]\nstrict = true\n[[tool.mypy.overrides]]\nmodule = \"test_helpers\"\nignore_errors = true\n[[tool.mypy.overrides]]\nmodule = \"conftest\"\nignore_errors = true\n[[tool.mypy.overrides]]\nmodule = \"pkg.tests\"\nignore_errors = true\n[[tool.mypy.overrides]]\nmodule = [\"tests.unit\", \"conftest\"]\nignore_errors = true\n[[tool.mypy.overrides]]\nmodule = \"pkg.core\"\nwarn_unused_ignores = true\n";
    let ok_doc = parse_source(&RepoPath::new("pyproject.toml")?, ok_overrides)?;
    assert_eq!(ok_doc.tool_config_findings, Vec::new());

    for bad_ov in [
        "module = []\nignore_errors = true",
        "module = \"pkg.core\"\nstrict = false",
        "module = \"pkg.core\"\ndisallow_untyped_defs = false",
    ] {
        let src = format!("[tool.mypy]\nstrict = true\n[[tool.mypy.overrides]]\n{bad_ov}\n");
        let d = parse_source(&RepoPath::new("pyproject.toml")?, &src)?;
        assert_eq!(
            d.tool_config_findings[0].violation,
            ToolConfigViolation::MypyNotStrict(MypyConfigIssue::ProductionOverrideWeakened)
        );
    }

    let mut_doc = parse_source(
        &RepoPath::new("mutants.toml")?,
        "examine_globs = []\nexamine_re = []\nexclude_globs = [\"./src/**\"]\n",
    )?;
    assert_eq!(mut_doc.tool_config_findings.len(), 3);
    assert_eq!(
        mut_doc.tool_config_findings[0].violation,
        ToolConfigViolation::MutantsScopeWeakened(MutantsConfigIssue::EmptyExamineGlobs)
    );
    Ok(())
}
