---
name: garde-fou
description: Operate the `garde` static analysis and architectural governance CLI (`GF000`–`GF082`), remediate SRP/DRY/KISS/YAGNI/6-domain/TDD/ecosystem diagnostics at their root cause, and synchronize `AGENTS.md`, `.agents/skills/`, and `garde.lock`.
---

# Garde-fou (`garde`) Agent Skill

Use this skill whenever working inside a repository governed by `garde-fou.toml`. Run `garde check --staged --format agent` continuously during implementation so every violation is fixed structurally rather than masked.

## CLI Commands

- `garde check --staged --format agent`: fast `BLAKE3`-cached check on staged Git changes with agent-ready Markdown remediation.
- `garde check --no-cache`: full workspace evaluation from scratch (AST, 6-domain segregation, Tarjan DAG, `[[boundaries]]`, 3-tier ecosystem tool ingestion, `SARIF`, and `LCOV`).
- `garde sync`: regenerate the `AGENTS.md` managed block, refresh `.agents/skills/garde-fou/SKILL.md`, fetch contextual `skills.sh` skills, and update `garde.lock` (`--offline` skips network).
- `garde lock`: seal `garde-fou.toml` cryptographically into `garde.lock` (`GF070`).
- `garde commit-msg <MSG_FILE>`: validate a Git commit message against Conventional Commits (`GF077`).
- `garde init [PATH]`: bootstrap or non-destructively adopt a repository (`--lang`, `--feature`, `--framework`, `--offline`).

## Diagnostic Reference & Root-Cause Remediation (`GF000`–`GF082`)

| Rule | Category | Root-Cause Fix |
| :--- | :--- | :--- |
| `GF000` | Syntax Error | Fix malformed syntax instead of commenting out broken blocks. |
| `GF001`–`GF008` | SRP & Complexity | Split large files (`<= max_file_lines`), extract pure helpers (`<= max_function_lines`), flatten nesting with guard clauses (`<= max_cognitive_complexity`), group parameters (`<= max_parameters`), and cap methods/imports. |
| `GF010` | Forbidden Constructs | Replace `unwrap`/`expect`/`panic!`/`unsafe` with algebraic `Result` or `Validation` propagation (`?`). |
| `GF020` | Type-2 AST Clone | Parameterize duplicated control-flow structures into one shared function; never rename identifiers cosmetically. |
| `GF021` | Dead Export | Remove unused `pub`/`export` visibility or delete dead code outside declared `entrypoints`. |
| `GF030`–`GF034` | Docker Hardening | Pin `FROM` tags/digests, set non-root `USER`, avoid `sudo`/`ADD`/inline secrets, clean layer caches, and maintain `.dockerignore`. |
| `GF040`–`GF043` | Secrets & Supply Chain | Move credentials to runtime secret stores and remediate vulnerable dependencies reported by `gitleaks` / `osv-scanner`. |
| `GF050` | Coverage Gate | Add meaningful unit/property tests until `LCOV` line coverage meets `min_line_coverage`. |
| `GF060`–`GF066` | 6-Domain Segregation | Keep `Code`, `Scripts` (tested), `Forme` (Design Tokens), `Texte` (`i18n` symmetry), `Contenus` (pure Markdown), and `Asset` (bounded & referenced) strictly separated with accessible JSX (`a11y`). |
| `GF070` | Governance & Integrity | Remove inline linter bypasses, never edit `garde-fou.toml` without authorization (`garde lock`), and run `garde sync` whenever context or config changes. |
| `GF071`–`GF072` | DAG & Boundaries | Break circular imports via leaf `types` modules (Tarjan SCC) and respect `[[boundaries]]` / Atomic Design layers. |
| `GF073`–`GF075` | KISS & YAGNI | Eliminate nested ternaries, Law of Demeter chains (`a.b.c.d.e`), wildcard `export *`, multi-boolean flag params, `TODO` markers, and single-implementation traits. |
| `GF076`–`GF077` | TDD & Git Commits | Ensure every test has non-tautological assertions (plus companion tests/`.stories` when active) and format commits as `<type>(<scope>): <summary>`. |
| `GF080` | Language AST Idioms & Security | Fix native language anti-patterns (`PY-B006`, `PY-E722`/`PY-B001`, `PY-S102`/`S301`/`S506`/`S602`, `PY-ANN201`/`ANN401`, `PY-T100`/`T201`, `TS-NO-EXPLICIT-ANY`, `TS-NO-DOUBLE-ASSERTION`, `TS-NO-NON-NULL-OPTIONAL-CHAIN`, `TS-NO-ASYNC-PROMISE-EXECUTOR`, `TS-NO-ASYNC-EFFECT`, `TS-NO-CONSOLE-DEBUG`, `REACT-RULES-OF-HOOKS`, `REACT-NO-UNSTABLE-NESTED-COMPONENT`, `REACT-JSX-KEY`, `REACT-NO-DANGEROUS-JSX`). |
| `GF081` | Tool Config Hardening | Keep `[tool.mypy] strict = true`, avoid `ignore`/`extend-ignore = ["ALL"]` or `--no-cov` in `pyproject.toml`/`ruff.toml`, enforce `"strict": true` across `tsconfig*.json` (`extends`-aware), and keep `"linter.enabled": true` in `biome.json`. |
| `GF082` | External Tool Ingestion | Resolve type-checker and linter errors (`[ruff:...]`, `[mypy:...]`, `[pyright:...]`, `[oxlint:...]`, `[biome:...]`, `[tsc:...]`) at their source rather than suppressing rules. |
