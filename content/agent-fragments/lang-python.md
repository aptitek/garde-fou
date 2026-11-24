# Python Engineering Standards

## Typing, Purity & Testing

- Annotate all function signatures for `mypy --strict` and keep `ruff` clean without `# noqa` or `# type: ignore` suppressions (`GF070`).
- Avoid wildcard `from module import *` imports (`GF074`), mutable default arguments, and circular package dependencies (`GF071`).
- Prefer `dataclass(frozen=True)` or typed `Protocol` contracts (only when `>= 2` concrete implementations exist, `GF075`), and write `pytest` cases with explicit assertions (`GF076`).
