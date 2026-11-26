---
name: garde-fou
description: Operate the `garde` static analysis and architectural governance CLI (`GF000`–`GF082`), remediate SRP/DRY/KISS/YAGNI/6-domain/TDD/ecosystem diagnostics at their root cause, and synchronize `AGENTS.md`, `.agents/skills/`, and `garde.lock`.
---

# Garde-fou (`garde`) Agent Skill

Use this skill whenever working inside a repository governed by `garde-fou.toml`. Run `garde check --staged --format agent` continuously during implementation so every violation is fixed structurally rather than masked.

## CLI Commands

- `garde check --staged --format agent`: fast `BLAKE3`-cached check on staged Git changes with agent-ready Markdown remediation.
- `garde check --no-cache`: full workspace evaluation from scratch (AST, 6-domain segregation, Tarjan DAG, `[[boundaries]]`, 3-tier ecosystem tool ingestion, `SARIF`, and `LCOV`, with `--color auto|always|never`).
- `garde check --watch` (`-w`): real-time debounced filesystem watch mode re-running incremental `BLAKE3` checks on every save (`r` reload, `q`/`Ctrl+C` quit).
- `garde tui` (or `garde check -i`): 3-tab interactive Bubble Tea cockpit (`[1] Diagnostics (Live)`, `[2] Projet & Garde-fous` lossless checkbox configurator, `[3] Cache` prune/clean; `/` filter, `w` Live/Pause watch, `r` reload, `a` agent Markdown preview, `y` `OSC 52` clipboard copy).
- `garde config [PATH]`: opens the interactive cockpit directly on Tab `[2] Projet & Garde-fous` (lossless `toml_edit` patch of `garde-fou.toml` + `garde sync`).
- `garde cache [status|prune|clean] [PATH]`: inspect `.garde-fou-cache/` disk usage, prune obsolete schema versions & orphaned entries, or wipe the cache cleanly.
- `garde uninstall [--project] [--self] [--purge-skills] [--dry-run] [-y] [PATH]` (and alias `garde eject [PATH]`): surgically detach Garde-fou managed blocks while preserving human content and custom skills, or uninstall the `garde` binary safely (`mise` / standalone).
- `garde sync`: regenerate the `AGENTS.md` managed block, refresh `.agents/skills/garde-fou/SKILL.md`, fetch contextual `skills.sh` skills, and update `garde.lock` (`--offline` skips network).
- `garde lock`: seal `garde-fou.toml` cryptographically into `garde.lock` (`GF070`).
- `garde commit-msg <MSG_FILE>`: validate a Git commit message against Conventional Commits (`GF077`).
- `garde init [PATH]`: bootstrap or non-destructively adopt a repository (`-i` Gum-style interactive wizard, `-y` non-interactive `Minimal` defaults, `--archetype`, `--lang`, `--feature`, `--framework`, `--offline`).

## Diagnostic Reference & Root-Cause Remediation (`GF000`–`GF082`)

| Rule | Category | Root-Cause Fix |
| :--- | :--- | :--- |
| `GF000` | Syntax Error | Fix malformed syntax instead of commenting out broken blocks. |
| `GF001`–`GF008` | SRP & Complexity | Split large files (`<= max_file_lines`), extract pure helpers (`<= max_function_lines`), flatten nesting with guard clauses (`<= max_cognitive_complexity`), group parameters (`<= max_parameters`), and cap methods/imports. |
| `GF010` | Forbidden Constructs | Replace `unwrap`/`expect`/`panic!`/`unsafe` with algebraic `Result` or `Validation` propagation (`?`). |
| `GF020` | Type-2 AST Clone | Parameterize duplicated control-flow structures into one shared function; never rename identifiers cosmetically. |
| `GF021` | Dead Export | Remove unused `pub`/`export` visibility or delete dead code outside declared `entrypoints`. |
| `GF030`–`GF034` | Docker Hardening | Pin `FROM` tags/digests, set non-root `USER`, validate `COPY --from=<stage>` (`DK-DL3022`), use JSON exec `CMD`/`ENTRYPOINT` (`DK-DL3025`), guard `RUN` pipes with `-o pipefail` (`DK-DL4006`), avoid `sudo`/`ADD`/inline secrets, clean layer caches, and maintain `.dockerignore`. |
| `GF040`–`GF043` | Secrets & Supply Chain | Move credentials to runtime secret stores and remediate vulnerable dependencies reported by `gitleaks` / `osv-scanner`. |
| `GF050` | Coverage Gate | Add meaningful unit/property tests until `LCOV` line coverage meets `min_line_coverage`. |
| `GF060`–`GF066` | 6-Domain Segregation | Keep `Code` (Panda CSS build-time semantic tokens only when `panda-css` active), `Scripts` (tested), `Forme` (Design Tokens: `CSS-*`, `PANDA-*`, `M3E-*`), `Texte` (`i18n` symmetry), `Contenus` (CommonMark/MDX: `MD-*`), and `Asset` (bounded, referenced & XXE/SMIL-sanitized SVG: `SVG-*`) strictly separated with W3C HTML5 & WCAG 2.2 AA accessibility (`W3C-*`, `A11Y-*`). |
| `GF070` | Governance & Integrity | Remove inline linter bypasses, never edit `garde-fou.toml` without authorization (`garde lock`), and run `garde sync` whenever context or config changes. |
| `GF071`–`GF072` | DAG & Boundaries | Break circular imports via leaf `types` modules (Tarjan SCC) and respect `[[boundaries]]` / Atomic Design / React Router server-client layers (`RR-SERVER-LEAK-TO-CLIENT`). |
| `GF073`–`GF075` | KISS & YAGNI | Eliminate nested ternaries, Law of Demeter chains (`a.b.c.d.e`), wildcard `export *`, multi-boolean flag params, `TODO` markers, and single-implementation traits. |
| `GF076`–`GF077` | TDD & Git Commits | Ensure every test has non-tautological assertions (plus companion tests/`.stories` when active) and format commits as `<type>(<scope>): <summary>`. |
| `GF080` | Language AST Idioms & Security | Fix native AST/schema anti-patterns across Python (`PY-*`, PEP 723 `PY-PEP723-*`), TypeScript/React (`TS-*`, `REACT-*`), Go (`GO-*`), Kotlin (`KT-*`), C# (`CS-*`), Astro (`ASTRO-*`), React Router (`RR-*`), Panda/M3E (`PANDA-*`, `M3E-*`), W3C/WCAG (`W3C-*`, `A11Y-*`), Cloudflare Workers/D1/R2 (`CF-*`), Drizzle ORM (`DRIZZLE-*`), Shell (`SH-*`, `SH-CURL-PIPE-UNSAFE`), PowerShell (`PS-*`), YAML/Compose/CI (`YAML-*`, `COMPOSE-*`, `GHA-*`), XML (`XML-*`), TOML (`TOML-*`), SQL/migrations (`SQL-*`, `MIG-*`), Jupyter (`NB-*`), Quarto (`QUARTO-*`), and Obsidian (`OBS-*`). |
| `GF081` | Tool Config Hardening | Keep strict settings intact across `pyproject.toml`, `ruff.toml`, `tsconfig*.json`, `biome.json`, `go.mod`, `.golangci.yml`, `detekt.yml`, `build.gradle.kts`, `*.csproj`, `Directory.Build.props`, `.editorconfig`, `panda.config.*`, `wrangler.*`, `.shellcheckrc`, `.hadolint.yaml`, `PSScriptAnalyzerSettings.psd1`, `.yamllint*`, `.dclintrc*`, `zizmor.yml`, `.stylelintrc*`, `.markdownlint*`, `rumdl.toml`, `taplo.toml`, `svgo.config.*`, `.squawk.toml`, `.sqruff`, `.sqlfluff`, and `drizzle.config.*`. |
| `GF082` | External Tool Ingestion | Resolve external verifier diagnostics (`ruff`, `mypy`, `pyright`, `oxlint`, `biome`, `tsc`, `golangci-lint`, `go-vet`, `detekt`, `ktlint`, `dotnet-build`, `astro-check`, `html-validate`, `wrangler-types`, `shellcheck`, `hadolint`, `psscriptanalyzer`, `yamllint`, `dclint`, `actionlint`, `zizmor`, `stylelint`, `rumdl`, `markdownlint`, `taplo`, `xmllint`, `squawk`, `sqruff`, `sqlfluff`) at their root cause. |

## Modern Exclusive Toolchains & MERN TypeScript (`GF072`, `GF080`–`GF081`)

- **JS/TS/Node (`pnpm`)**: Enforce `TOOLCHAIN-PNPM-ONLY` (pinned `packageManager: pnpm@x.y.z`, `"type": "module"`, zero `npm`/`yarn`/lockfiles), `NODE-PROTOCOL-IMPORT` (`node:*`), `NODE-NO-SYNC-IO-IN-HANDLER`, and `NODE-MODERN-ESM-HYGIENE`.
- **Python (`uv`)**: Enforce `TOOLCHAIN-UV-ONLY` (`pyproject.toml` + `uv.lock`, `uv sync --locked`, zero `pip`/`poetry`/`pipenv`/`requirements.txt`/`setup.py`).
- **Kotlin/JVM (`Gradle .kts`)**: Enforce `TOOLCHAIN-GRADLE-KTS-ONLY` (`build.gradle.kts` + `settings.gradle.kts`, HTTPS wrapper, pinned versions) and `GRADLE-USE-VERSION-CATALOG` (`gradle/libs.versions.toml` + `libs.*`).
- **MERN TypeScript (`Express 5` + `Mongoose`)**: Enforce `EXPRESS-UNVALIDATED-REQUEST-CAST`, `EXPRESS-INSECURE-CORS-OR-ERROR-LEAK`, `EXPRESS-LISTEN-IN-APP-MODULE`, `MONGO-NOSQL-INJECTION`, `MONGO-UNBOUNDED-QUERY-OR-MUTATION`, `MONGO-SCHEMA-STRICTNESS`, and `GF072` frontend/backend isolation.

## Python Ecosystems: Django, NiceGUI, PyTorch/Keras & LangChain/LangGraph (`GF080`–`GF081`)

- **Django (`DJ-*`)**: Enforce `DJ-RAW-SQL-OR-UNSAFE-HTML` (no `.extra`, no dynamic `RawSQL`/`.raw`, no `mark_safe` or `format_html(f"...")`), `DJ-MODEL-AND-FORM-HYGIENE` (`DJ001` with `unique=True, blank=True` exemption, `DJ008` `__str__`, `DJ003`/`DJ007`), `DJ-ORM-N-PLUS-ONE-AND-UNBOUNDED`, and `DJ-SETTINGS-AND-AUTH-HARDENING`; exempt `**/migrations/0*.py` from `GF001`/`GF021`/`GF076` while auditing `migrations.RunSQL`.
- **NiceGUI (`NICEGUI-*`)**: Enforce `NICEGUI-BLOCKING-EVENT-LOOP`, `NICEGUI-GLOBAL-UI-OR-STATE-LEAK`, `NICEGUI-WORKER-CONTEXT-VIOLATION` (`run.io_bound`/`run.cpu_bound`/`@app.on_exception`), and `NICEGUI-SECURITY-XSS-AND-SECRET`.
- **Deep Learning (`ML-*`)**: Enforce `ML-UNSAFE-MODEL-DESERIALIZATION`, `ML-TORCH-TRAINING-AND-VRAM-BUGS`, `ML-TF-RETRACING-AND-EAGER`, and cross-cell `ML-DATA-LEAKAGE-BEFORE-SPLIT`.
- **LLM Agents (`LLM-*`)**: Enforce `LLM-RCE-AND-CVE-VECTORS`, `LLM-SYSTEM-PROMPT-INJECTION`, and `LLM-LANGGRAPH-INFINITE-LOOP-AND-BUDGET` (`StateGraph` -> `END`, `recursion_limit`, `timeout`/`max_retries`).
- **MariaDB, Advanced PostgreSQL & Oracle PL/SQL (`MARIADB-*`, `PG-*`, `PLPGSQL-*`, `PLSQL-*`)**: Enforce online DDL (`ALGORITHM=INSTANT`/`INPLACE`), `utf8mb4`, `InnoDB`, workspace RLS/Policy parity, `SECURITY DEFINER` + `SET search_path`, PL/pgSQL `INTO STRICT` + `%I`/`USING`, Oracle `AUTHID CURRENT_USER`, `BULK COLLECT LIMIT` + `FORALL`, package spec/body parity, `pgTAP` / `utPLSQL` TDD (`GF076`), and `zpa-cli` (`GF082`).
