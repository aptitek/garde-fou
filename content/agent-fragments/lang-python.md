# Python Engineering Standards (`GF080`–`GF082`)

## Typing, Security & Ecosystem Tooling

- Annotate all function signatures for `mypy --strict` without explicit `: Any` in `Code` (`PY-ANN401`, `GF080`), and keep `ruff` / `mypy` / `pyright` at zero diagnostics (`GF082`) without `# noqa` or `# type: ignore` suppressions (`GF070`).
- Never weaken `[tool.mypy] strict = true` or set `ignore = ["ALL"]` in `pyproject.toml` / `ruff.toml` (`GF081`).
- Forbid mutable default arguments (`PY-B006`), bare or silent `except` blocks (`PY-E722`, `PY-B001`), `eval`/`exec` (`PY-S102`), unsafe `pickle`/`yaml.load` (`PY-S301`), `subprocess(..., shell=True)` (`PY-S602`), and debug `print(...)` in `Code` outside declared CLI `entrypoints` (`PY-T201`, `GF080`).
- Avoid wildcard `from module import *` imports (`GF074`) and circular package dependencies (`GF071`); prefer `dataclass(frozen=True)`, typed `Protocol` contracts (`>= 2` concrete implementations, `GF075`), and `pytest` tests with explicit assertions (`GF076`).
