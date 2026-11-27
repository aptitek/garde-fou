# Garde-fou (`garde`)

> **English** | [Français](README.fr.md)

**Garde-fou** is an ultra-strict polyglot static analyzer and governance orchestrator inspired by Pandoc's Hub-and-Spoke architecture (`Tree-sitter` -> `#![no_std]` `UAST` -> Pure rules + Per-language tool ingestion + `SARIF` / `LCOV` bridges + Incremental `BLAKE3` `v22` cache).

The CLI binary is named **`garde`** (`garde.exe` on Windows) and reads the strict `garde-fou.toml` configuration.

---

## 1. Installation

### Via `mise` (recommended)

Globally or directly inside a project's `mise.toml`:

```bash
mise use -g github:ArchiPiXL/garde-fou
```

```toml
# mise.toml
[tools]
"github:ArchiPiXL/garde-fou" = "latest"
```

### Linux & macOS One-Liner (mandatory `SHA256SUMS` verification)

```bash
curl -fsSL https://github.com/ArchiPiXL/garde-fou/releases/latest/download/install.sh | sh
```

With explicit options (`--version`, `--install-dir`):

```bash
curl -fsSL https://github.com/ArchiPiXL/garde-fou/releases/latest/download/install.sh | sh -s -- --version v0.3.0 --install-dir "$HOME/.local/bin"
```

### Windows PowerShell 5.1+ / `pwsh` 7+ One-Liner (mandatory `SHA256SUMS` verification)

```powershell
irm https://github.com/ArchiPiXL/garde-fou/releases/latest/download/install.ps1 | iex
```

### Local Developer Install (`task install`)

```bash
task install
# equivalent to: cargo install --path crates/gf-cli --locked --force
```

---

## 2. Supported Languages, Features & Frameworks

### Analyzed Languages & Formats (23 `LanguageId` + specialized formats)

| Category | Languages / Formats | Slugs (`--lang` / `garde-fou.toml`) |
| :--- | :--- | :--- |
| **Systems & Backend** | Rust (Edition 2024), Go, Kotlin / JVM, C# / .NET, PHP 8.4, Python | `rust`, `go`, `kotlin`, `csharp`, `php`, `python` |
| **Web, UI & Mobile** | TypeScript / JavaScript (`TS`/`TSX`/`JS`/`JSX`), Astro, HTML5, CSS / SCSS, Twig 3, Dart 3.6+ / Flutter 3.x | `typescript`, `astro`, `html`, `css`, `twig`, `dart` |
| **Databases** | Multi-dialect SQL (`PostgreSQL`, `MariaDB`, `SQLite`), Oracle PL/SQL | `sql`, `plsql` |
| **Automation & Infra** | POSIX Shell / Bash, PowerShell, `Dockerfile` / `Containerfile`, YAML 1.2 (Compose & GitHub Actions), TOML, XML / SVG | `shell`, `powershell`, `yaml`, `toml`, `xml` |
| **Content, Knowledge & Diagrams** | Markdown / GFM / MDX, Jupyter (`.ipynb`), Quarto (`.qmd`), Obsidian Canvas (`.canvas`), Mermaid v11, PlantUML | `markdown`, `mermaid`, `plantuml` |
| **Localization & Modding** | JSON / JSONC, GNU Gettext (`.po`/`.pot`), Project Fluent (`.ftl`), ARB / XLIFF, Minecraft 1.21+ `.mcfunction` | `mcfunction` |

### Cross-Cutting Features (`features`)

| Feature (`--feature`) | Capabilities & Guardrails |
| :--- | :--- |
| `docker` | `Dockerfile` / `Containerfile` security (`GF030`–`GF034`), `.dockerignore`, embedded `RUN` AST bridges |
| `git-hooks` | `lefthook` Git hooks (`pre-commit`, `commit-msg` Conventional Commits `GF077`, `pre-push`) |
| `ci-github` | GitHub Actions hardening (`permissions`, SHA/tag pinning, anti-injection, `actionlint`/`zizmor`) |
| `task-runner` | `Taskfile.yml`, `justfile`, and reproducible `mise.toml` toolchain pinning |
| `ui` | `Code`/`Forme` segregation, Design Tokens (`GF060`–`GF061`), and static `a11y` checks (`GF066`) |
| `i18n` | Key-symmetric HTML-free `Texte` dictionaries (`GF062`), ICU / Gettext / Fluent / ARB / XLIFF |
| `content` | Editorial Markdown / MDX purity (`GF063`), YAML 1.2 frontmatter, heading hierarchy, and anchors |
| `tdd` | TDD Fortress (`GF076`): `LCOV` line coverage (`GF050`), mutation score (`GF052`), `1:1` test parity |
| `storybook` | Mandatory `.stories.tsx` companion parity (`GF076`) |
| `panda-css` | Zero-runtime Panda CSS guardrails (`panda.config.*`, `styled-system/`) |
| `m3e` | Material Design 3 Expressive (tonal pairing, Spatial/Effects springs, `48dp` touch targets) |
| `w3c` | W3C HTML5 content models, WAI-ARIA 1.2, and WCAG 2.2 AA/AAA compliance |
| `ai-ml` | ML / Deep Learning & LLM Agent governance (`PyTorch`, `Keras 3`, `LangChain`/`LangGraph`) |
| `diagrams` | Diagrams-as-Code (`Mermaid` v11 & `PlantUML`) |

### Frameworks & Ecosystems (`frameworks`) & Archetypes

- **Web, Edge & Node.js**: `react`, `astro`, `nextjs`, `react-router`, `cloudflare-workers`, `drizzle`, `nodejs`, `express`, `mongoose`
- **Python & AI/ML**: `fastapi`, `django`, `nicegui`, `pytorch`, `tensorflow`, `langchain`
- **Backend, Mobile & Databases**: `cargo-workspace`, `symfony`, `doctrine`, `flutter`, `postgresql`, `mariadb`, `docker-compose`
- **Knowledge, Academic & Modding**: `obsidian`, `quarto`, `jupyter`, `minecraft-mod`, `kilt`, `datapack`
- **6 Proportional Archetypes (`[project] archetype`)**: `minimal`, `scripts`, `infra`, `knowledge`, `academic`, `software`

### Key CLI Capabilities (`garde`)

- **3-Tier Polyglot Analysis**: zero-dependency native AST rules (`GF080`), anti-weakening tool config audits (`GF081`), and unified external verifier / `SARIF` 2.1.0 ingestion (`GF082`, `gitleaks`, `osv-scanner`).
- **Developer & AI Agent Experience**: interactive TUI (`garde tui`), live watch mode (`garde check --watch`), lossless configurator (`garde config`), `human` / `agent` Markdown / `sarif` reporters, bilingual EN/FR CLI, `AGENTS.md` + `.agents/skills/` sync (`garde sync`), and `BLAKE3` cryptographic sealing (`garde lock`).

---

## 3. Six-Domain Architectural Segregation

`garde` enforces hermetic separation across six orthogonal domains (configurable via `[domains]` in `garde-fou.toml`):

1. **`Code` (`src/**`, `crates/**`, `app/**`)**: pure business logic and semantic structure (zero CSS-in-JS, zero raw `#hex`/`px` literals, zero inline `<svg>`, static `a11y` verified).
2. **`Scripts` (`scripts/**`, `tools/**`)**: standalone tooling subject to SRP/secret caps, never imported by `Code`, and covered by automated tests.
3. **`Forme` (`styles/**`, `tokens/**`, `panda.config.*`)**: primitive Design Tokens confined to `tokens/` and stylesheets consuming `var(--token)` exclusively without `!important` or translatable copy.
4. **`Texte` (`locales/**`, `i18n/**`, `messages/**`)**: pure `i18n` dictionaries (zero HTML), strictly key-symmetric across locales and synchronized with `t("...")` calls in `Code`.
5. **`Contenus` (`content/**`, `courses/**`, `docs/**`)**: pure editorial Markdown/MDX (zero raw HTML outside code blocks, sequential heading hierarchy, images with `alt`).
6. **`Asset` (`assets/**`, `public/**`, `static/**`)**: confined media and sanitized SVGs (no `<script>` or `on*=`), within size limits (`1 MiB`), with zero broken links or orphan files.

---

## 4. Quick Start

```bash
# Bootstrap a turnkey project (interactive Gum-style wizard on TTY, or declarative flags)
garde init
garde init -i
garde init -y
garde init my-service --lang rust --feature docker,git-hooks
garde init ui-portal --lang typescript --framework astro --feature ui,i18n,content,docker
garde init orders-api --lang python --framework fastapi --feature docker,ci-github

# 3-tab interactive cockpit (Live Diagnostics, Checkbox Configurator, Cache)
garde tui
garde check -i

# Real-time watch mode on save (250 ms poll + 120 ms debounce, zero FFI dependencies)
garde check --watch

# Lossless interactive configurator for garde-fou.toml (radio archetype + checkboxes)
garde config

# BLAKE3 cache management (.garde-fou-cache/)
garde cache status
garde cache prune
garde cache clean

# Surgical uninstall (project, binary, or 3-choice interactive wizard)
garde uninstall --project --dry-run
garde eject -y
garde uninstall --self --dry-run

# Ultra-fast incremental check on staged files (ideal for pre-commit + AI agents)
garde check --staged --format agent

# Full uncached check with strict external scanners, LCOV gate, and color control
garde check --no-cache --strict-tools --lcov lcov.info --color auto

# OASIS SARIF 2.1.0 export for GitHub Code Scanning
garde check --format sarif

# Sync modular AGENTS.md block, .agents/skills/ (skills.sh), and garde.lock
garde sync

# Cryptographically seal garde-fou.toml into garde.lock (BLAKE3)
garde lock

# Validate a Conventional Commits message (Git commit-msg hook)
garde commit-msg .git/COMMIT_EDITMSG
```

---

## 5. Rule Catalog (`GF000`–`GF082`)

| Rule | Domain / Scope | Description | Default Threshold |
| :--- | :--- | :--- | :--- |
| `GF000` | File | CST syntax error (`Tree-sitter` `ERROR` / `MISSING`, `Dockerfile`, or `i18n` dictionary) | `0` |
| `GF001` | `Code` / `Scripts` | Maximum lines per source file or script | `<= 250` |
| `GF002` | Function | Maximum effective lines per function / method / closure | `<= 50` |
| `GF003` | Function | Maximum parameters per signature (excluding `self` / `cls`) | `<= 4` |
| `GF004` | Function | Maximum cognitive complexity (with nesting penalty) | `<= 10` |
| `GF005` | Function | Maximum McCabe cyclomatic complexity | `<= 10` |
| `GF006` | Function | Maximum control-flow nesting depth | `<= 4` |
| `GF007` | File | Maximum outgoing coupling (`imports` / `use` per file) | `<= 15` |
| `GF008` | Type | Maximum methods per `struct` / `impl` / `trait` / `class` | `<= 10` |
| `GF010` | File | Forbidden constructs (`unsafe`, `unwrap`, `expect`, `panic!`, `todo!`, `dbg!`) | `0` |
| `GF020` | Workspace | Type-2 structural AST duplication (normalized `$ID`/`$LIT` 128-bit `BLAKE3` hash) | `>= 15` lines & `>= 40` nodes |
| `GF021` | `Code` | Dead public export (`pub` / `export` / public Python symbol unreferenced outside entrypoints) | `0` |
| `GF030` | Docker | Unpinned `FROM` base image or `:latest` tag | `0` |
| `GF031` | Docker | Final stage running as `root` (missing non-privileged `USER`) | `0` |
| `GF032` | Docker | Security hazard (`sudo` in `RUN`, local non-archive `ADD`, secret in `ENV`/`ARG`) | `0` |
| `GF033` | Docker | Layer hygiene (relative `WORKDIR`, unpurged `apt`/`pip` cache) | `0` |
| `GF034` | Docker | Missing or inconsistent `.dockerignore` for detected languages | `0` |
| `GF040` | File | Native zero-dependency secret / PEM private key detection (values always masked) | `0` |
| `GF041` | Scanner | Secret detected by `gitleaks` (unified `SARIF` 2.1.0 ingestion) | `0` |
| `GF042` | Scanner | Dependency vulnerability detected by `osv-scanner` (unified `SARIF` 2.1.0 ingestion) | `0` |
| `GF043` | Scanner | Required external binary (`gitleaks` / `osv-scanner`) missing from `PATH` (`required` / `--strict-tools`) | `0` |
| `GF050` | Workspace | `LCOV` line coverage (`lcov.info`) below required minimum threshold | `min_line_coverage` (`85%`) |
| `GF060` | `Code` vs `Forme` | `.css`/`.scss` outside `Forme`, or CSS-in-JS / `style=` / `#hex` / CSS unit leak inside `Code` | `0` |
| `GF061` | `Forme` | Raw color, magic `px`, or `!important` outside `tokens/`, or `content: "Text"` / `url(data:)` leak | `0` |
| `GF062` | `Texte` / `i18n` | HTML in `Texte`, cross-locale key asymmetry, missing/dead `t("...")` key, or hardcoded JSX text | `0` |
| `GF063` | `Contenus` | Raw HTML outside fenced/inline code, forbidden MDX logic/styles, heading skip, image without `alt`, or stray `.md` | `0` |
| `GF064` | `Asset` | Asset outside `assets/`, `> max_asset_bytes`, unsafe SVG (`<script>`/`on*=`), inline `<svg>`/`data:`, broken link, or orphan | `0` |
| `GF065` | `Scripts` | Importing `Scripts` from `Code`, or script under `scripts/`/`tools/` lacking automated test coverage | `0` |
| `GF066` | `Code` (`a11y`) | `<div/span/p onClick>` without keyboard role, `<img>` without `alt`, `<button>` without `type` or accessible label | `0` |
| `GF070` | Governance & Integrity | Linter bypass (`eslint-disable`, `@ts-ignore`, `#[allow]`), tampered `garde.lock`, or `AGENTS.md`/`.agents/skills/` drift (`garde sync`) | `0` |
| `GF071` | Architecture | Inter-file dependency cycle detected via `#![no_std]` Tarjan SCC | `0` cycles |
| `GF072` | Architecture | Declarative `[[boundaries]]` violation or Atomic Design hierarchy breach (`atoms -> molecules -> organisms`) | `0` |
| `GF073` | KISS | Nested ternary or Law of Demeter property chain (`a.b.c.d.e`) | `0` nested ternaries / `<= 4` segments |
| `GF074` | YAGNI (local) | Barrel `export *` / wildcard import, excessive boolean flags, or `TODO`/`FIXME`/`HACK` marker in `Code` | `<= 1` `bool` param |
| `GF075` | YAGNI (global) | Behavioral contract (`trait` / `interface` / `Protocol` with methods) outside `entrypoints` having `<= 1` implementation | `>= 2` implementations |
| `GF076` | TDD Fortress | Assertion-free test, tautological assertion (`assert!(true)`), or missing `1:1` companion test/`.stories` | `>= 1` real assertion |
| `GF077` | Git Hygiene | Commit message violating Conventional Commits (`<type>(<scope>): <description>`) or vague summary (`wip`, `misc`) | `<= 100` chars |
| `GF080` | Language AST (Tier 1) | Native zero-dependency AST idioms & security checks (Rust, TS/React, Python, Go, Kotlin, C#, PHP, Dart, Shell, PowerShell, SQL, YAML, Docker, Markdown, Minecraft, etc.) | `0` |
| `GF081` | Tool Configs (Tier 2) | Anti-weakening audit of tool configs (`pyproject.toml`, `ruff.toml`, `tsconfig.json`, `.hadolint.yaml`, `.shellcheckrc`, `phpstan.neon`, etc.) | `0` |
| `GF082` | Ecosystem Tools (Tier 3) | Unified diagnostics ingested from external verifiers (`ruff`, `mypy`, `oxlint`, `tsc`, `shellcheck`, `hadolint`, `phpstan`, `dart analyze`, `golangci-lint`, etc.) | `0` |

---

## 6. Internal Architecture

| Crate | Environment | Responsibility |
| :--- | :--- | :--- |
| `gf-core` | `#![no_std]` + `alloc` | Pure algebraic model (`Span`, `RepoPath`, `DomainKind`, `BoundaryRule`, `CommitConfig`, `ToolsConfig`, `UastDocument`, `Validation`) |
| `gf-rules` | `#![no_std]` + `alloc` | Local & global rule engine (`GF020`/`GF021`), Tarjan DAG (`GF071`), `[[boundaries]]` (`GF072`), KISS/YAGNI/TDD (`GF073`–`GF076`), ecosystem (`GF080`–`GF082`) |
| `gf-parsers` | Pure `std` (0 I/O) | Safe Rust `Tree-sitter` extractors & analyzers for all 23 `LanguageId` + specialized formats (`GF080`/`GF081`) |
| `gf-engine` | Imperative Shell | `BLAKE3` cache `.garde-fou-cache/v22/`, `garde sync` (`AGENTS.md` + `skills.sh`), `garde.lock`, `garde init`, tool adapters (`GF082`), `SARIF` orchestration |
| `gf-cli` | `garde` Binary | Subcommands `check`, `tui`, `init`, `config`, `cache`, `sync`, `lock`, `commit-msg`, `uninstall`, pure Rust TUI engine (`ui/`, `tui/`), and reporters (`human`, `agent`, `sarif` 2.1.0) |
