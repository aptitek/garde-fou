# Python Engineering Standards (`GF080`–`GF082`)

## Typing, Security & Ecosystem Tooling

- Annotate all public function and method parameters (excluding `self`/`cls`) and return types (`-> ...`) for `mypy --strict` (`PY-ANN201`) without explicit `: Any` or `typing.cast(Any, ...)` in `Code` (`PY-ANN401`, `GF080`), and keep `ruff` / `mypy` / `pyright` at zero diagnostics (`GF082`) without `# noqa`, `# pyright: ignore`, or `# type: ignore` suppressions (`GF070`).
- Never weaken `[tool.mypy] strict = true`, add `ignore_errors = true` on production `[[tool.mypy.overrides]]`, set `ignore`/`extend-ignore = ["ALL"]` or empty `select = []` in `pyproject.toml`/`ruff.toml`, pass `--no-cov` in `pytest` `addopts`, or lower `[tool.coverage.report] fail_under` (`GF081`).
- Forbid mutable default arguments (`PY-B006`), bare or silent `except` blocks (`PY-E722`, `PY-B001`), `eval`/`exec` (`PY-S102`), unsafe `pickle`/`marshal` (`PY-S301`), `yaml.load` without `SafeLoader` (`PY-S506`), `subprocess(..., shell=True)` (`PY-S602`), and debug `breakpoint()`/`pdb.set_trace()` (`PY-T100`) or `print(...)` (`PY-T201`) in `Code` outside declared CLI `entrypoints` (`GF080`).
- Avoid wildcard `from module import *` imports (`GF074`) and circular package dependencies (`GF071`); expose explicit `__all__` package APIs (`GF021`), prefer `dataclass(frozen=True)` and typed `Protocol`/`ABC` contracts (`>= 2` concrete implementations, `GF075`), and write `pytest` tests with real value or `pytest.raises(...)` assertions (`GF076`).
