# Changelogs

> **English** | [Français](CHANGELOGS.fr.md)

All notable changes to **Garde-fou** (`garde`) are documented in this file.

The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/) and [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.3.0] - 2026-11-26

### Added

- **TUI Charm-style, Live Watch & Pilotage CLI (`gf-cli`)**:
  - Pure Safe Rust terminal renderer inspired by Lipgloss and Miette source snippets with strict `NO_COLOR` and `--format agent|sarif` isolation.
  - Real-time watch mode (`garde check --watch`) and multi-tab interactive dashboard (`garde tui` / `garde check -i`) with OSC 52 clipboard copy.
  - Lossless interactive checkbox configurator (`garde config`) powered by `toml_edit` to toggle archetypes, languages, frameworks, features, and policies without losing TOML comments.
  - Cache management subcommands (`garde cache status|prune|clean`) and surgical uninstaller (`garde uninstall --project|--self`, `garde eject`).
  - Native FR/EN CLI localization (`--locale auto|fr|en`, `locales/fr.toml`, `locales/en.toml`) dogfooded under `GF062`.
- **Exclusive Modern Toolchains Doctrine (`GF081`)**:
  - Enforces `pnpm` for JS/TS/Node (forbidding `npm`/`yarn`), `uv` for Python (forbidding `pip`/`poetry`/`pipenv`/`setup.py`), and Gradle Kotlin DSL (`build.gradle.kts` + `libs.versions.toml`, forbidding Groovy `build.gradle`).
- **Compiled Languages — Go, Kotlin & C# 12/13 (`GF071`, `GF075`, `GF080`–`GF082`)**:
  - Native `Tree-sitter` AST analyzers for Go (`staticcheck` quirks, implicit structural interface satisfaction), Kotlin (coroutines, `runCatching`, Gradle `.kts`), and C# 12/13 (primary constructors `CS9124`, `async`/`CancellationToken`), plus `golangci-lint`, `go vet`, `detekt`, `ktlint`, and `dotnet build` ingestion.
- **Node.js ESM & MERN TypeScript Stack**:
  - Guardrails for `node:` protocol imports, Express 5 middleware/security, and MongoDB/Mongoose anti-NoSQL injection.
- **Python Industrial Ecosystems (`uv` exclusive)**:
  - Native rules for **Django 5.2/6.0**, **NiceGUI** (`io_bound`/`cpu_bound`/`app.storage`), **PyTorch / TensorFlow / Keras 3** (anti-pickle/HDF5, VRAM autograd, anti-data leakage), and **LangChain / LangGraph** (`StateGraph` `END`, `recursion_limit`, anti-RCE/SSTI).
- **Advanced SQL & Procedural Databases (`GF080`–`GF082`)**:
  - **MariaDB** (`utf8mb4`, Online DDL, implicit `COMMIT`), **PostgreSQL Advanced** (cross-table RLS, `SECURITY DEFINER`, PL/pgSQL `INTO STRICT`), **Oracle PL/SQL** (`AUTHID CURRENT_USER`, `FORALL`, `BULK COLLECT LIMIT`, `utPLSQL`), and `zpa-cli` JSON ingestion.
- **Diagrams-as-Code & Universal `i18n` v2 (`GF062`, `GF080`)**:
  - Pure Safe Rust **Mermaid v11** and **PlantUML** structural/anti-XSS/anti-SSRF analyzers for standalone files and Markdown/Quarto fenced code blocks.
  - Universal `i18n` v2 engine supporting **ICU MessageFormat**, placeholder parity (`I18N-PLACEHOLDER-MISMATCH`), Gettext `.po`/`.pot`, Flutter `.arb`, Fluent `.ftl`, and Symfony XLIFF `.xlf`/`.xliff`.
- **PHP 8.4, Symfony 7, Doctrine ORM 3 & Twig 3**:
  - Pure Safe Rust PHP 8.4 (`declare(strict_types=1)`, anti-`eval`/`unserialize`, superglobals, loose equality), Symfony 7 & Doctrine ORM 3 (service locator, DQL/SQL injection, `flush()` in loops, `#[IsGranted]`, non-`final` entities), Twig 3 (`|raw`, inline style/script, `|trans`), PSR-4 workspace cycle graph (`GF071`), and `PHPStan`/`Psalm` JSON parsers.
- **Dart 3.6+ & Flutter 3.x**:
  - Nested `/* /* */ */` block-comment lexer, `use_build_context_synchronously` (`FLUTTER-BUILD-CONTEXT-ASYNC`), `dispose()` lifecycle checks, Material 3 theme token enforcement, `IconButton` a11y tooltips, generated `*.g.dart`/`*.freezed.dart` exemptions, `package:` import graph (`GF071`), and `dart analyze` JSON parser.
- **Minecraft Modding (Kilt / Fabric / Quilt / NeoForge / Mixin) & `.mcfunction` 1.21+ Data Packs**:
  - Mixin `@Overwrite` / `@Redirect` / client-in-common / tick-thread `Thread.sleep` guardrails, Brigadier 1.21+ `.mcfunction` syntax (`$` / `$(param)` macros, singular `data/<ns>/function/`, unbounded `@e` lag selectors, mutual recursion cycles `GF071`), `pack.mcmeta` / `fabric.mod.json` / `*.mixins.json` Tier 2 audit, and `Spyglass` JSON parser.

### Changed

- Upgraded `BLAKE3` incremental cache schema to `v22` to cover all new language extractors and workspace metadata.
- Grouped extended Tier 1 ecosystem violations in `gf-core` (`extended_violations.rs` + `lang_rule_from.rs`) to preserve strict acyclic DAG (`GF071`) and fan-out (`GF007`) invariants.
- Recognized `CHANGELOGS.md` alongside `CHANGELOG.md` as a root governance document under `GF063`.

## [0.2.1] - 2026-11-26

### Added

- **Proportional Arsenal & 6 Workspace Archetypes**:
  - Redesigned `garde init` with zero default languages on empty invocation and topological adaptation for `minimal`, `scripts` (PEP 723, root scripts), `infra` (Docker Compose without Rust), `knowledge` (Obsidian OFM `[[WikiLinks]]`, `![[embeds]]`, `.canvas`), `academic` (Jupyter `.ipynb` cells/magics/tracebacks & Quarto `.qmd` divs/shortcodes/crossrefs), and `software`.

### Changed

- Raised MSRV (`rust-version`) to `1.100` and pinned `garde-fou` in `mise.toml`.

## [0.2.0] - 2026-11-26

### Added

- **Six-Domain Architectural Segregation (`GF060`–`GF066`)**:
  - Hermetic separation of `Code`, `Scripts`, `Forme`, `Texte`, `Contenus`, and `Asset` domains with cross-domain leak, design-token, `i18n` symmetry, Markdown purity, asset integrity, and static `a11y` enforcement.
- **Anti-Bypass, Tarjan DAG, Boundaries, KISS/YAGNI & TDD/Mutation Fortresses (`GF052`, `GF070`–`GF077`)**:
  - Universal AST suppression/bypass detector and cryptographic `BLAKE3` governance seal `garde.lock` (`garde lock`, `GF070`).
  - Deterministic `#![no_std]` Tarjan circular-dependency detector (`GF071`) and `[[boundaries]]` / Atomic Design layer constraints (`GF072`).
  - KISS/YAGNI smell detectors (`GF073`–`GF075`), TDD assertion/parity fortress (`GF076`), Conventional Commits hook (`garde commit-msg`, `GF077`), and mutation testing gate (`GF052`).
- **Agentic Core (`garde sync`)**:
  - Modular `AGENTS.md` assembly from embedded fragments plus `skills.sh` installation and supply-chain verification.
- **3-Tier Ecosystem Ingestion Pipeline (`GF080`–`GF082`)**:
  - Native AST analyzers, Tier 2 config hardening, and Tier 3 external tool output parsers for Python (`ruff`, `mypy`, `pyright`), TypeScript & React 19 (`oxlint`, `biome`, `tsc`, JSONC `tsconfig`), POSIX Shell / Bash / PowerShell & `Dockerfile RUN` AST bridge (`shellcheck`, `hadolint`, `PSScriptAnalyzer`), YAML 1.2 / Docker Compose / GitHub Actions (`yamllint`, `dclint`, `actionlint`, `zizmor`), CSS / XML / SVG / TOML / Markdown (`stylelint`, `xmllint`, `taplo`, `rumdl`, `markdownlint`), SQL & PostgreSQL zero-downtime migrations (`squawk`, `sqruff`, `sqlfluff`), and Web/Edge stacks (Astro, React Router v7, Panda CSS, Material 3 Expressive, W3C HTML5 / WCAG 2.2 AA, Cloudflare Workers / Wrangler, Drizzle ORM).

## [0.1.0] - 2026-11-23

### Added

- **Functional Core / Imperative Shell Architecture**:
  - `#![no_std]` + `alloc` core (`gf-core`, `gf-rules`), I/O-free `Tree-sitter` -> `UAST` parsers (`gf-parsers`), `BLAKE3` incremental cache engine (`gf-engine`), and `garde` CLI binary (`gf-cli`) with `human`, `agent`, and `sarif` reporters.
- **Foundational Governance Rules (`GF000`–`GF050`)**:
  - SRP structural metrics (`GF000`–`GF009`), Type-2 AST clone detection (`GF020`), dead export detection (`GF021`), Docker security (`GF030`–`GF034`), native secret & `.gitignore` / Gitleaks / OSV-Scanner gates (`GF040`–`GF043`), and LCOV coverage gate (`GF050`).
- **Distribution & Multi-OS Release Pipeline**:
  - `garde init` scaffolding, `mise` integration, verified `SHA256SUMS` one-liner installers (`scripts/install.sh`, `scripts/install.ps1`), and multi-OS GitHub Actions release workflow (`x86_64-unknown-linux-musl`, `aarch64-unknown-linux-gnu`, `x86_64-apple-darwin`, `aarch64-apple-darwin`, `x86_64-pc-windows-msvc`).

[0.3.0]: https://github.com/ArchiPiXL/garde-fou/compare/v0.2.1...v0.3.0
[0.2.1]: https://github.com/ArchiPiXL/garde-fou/compare/v0.2.0...v0.2.1
[0.2.0]: https://github.com/ArchiPiXL/garde-fou/compare/v0.1.0...v0.2.0
[0.1.0]: https://github.com/ArchiPiXL/garde-fou/releases/tag/v0.1.0
