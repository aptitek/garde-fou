# Journal des Modifications (Changelogs)

> [English](CHANGELOGS.md) | **Français**

Tous les changements notables apportés à **Garde-fou** (`garde`) sont documentés dans ce fichier.

Le format suit [Keep a Changelog](https://keepachangelog.com/fr/1.1.0/) et [Semantic Versioning](https://semver.org/lang/fr/).

## [0.4.0] - 2026-11-28

### Ajouté

- **Garde-fous Tuples Anonymes & Dépaquetage Global Dangereux (`GF073`)** :
  - Ajout des paramètres configurables `[clean_code]` `max_tuple_arity` (défaut `2`, autorisant les paires `(k, v)` et bloquant les tuples anonymes de 3+ éléments ; `0` pour tout interdire) et `forbid_global_tuple_unpacking` (défaut `true`).
  - Extracteurs AST/records natifs pour **Rust**, **Python** (avec exemption `except (...)`), **TypeScript/JavaScript**, **C#** (instructions top-level incluses) et **Dart 3+**.
- **Forteresse de Tests de Mutation Polyglotte à 100% (`GF052` / `GF081`)** :
  - Ajout du registre canonique `MutationStackSpec`, des tâches `Taskfile.yml` `mutate`, des entrées `.gitignore` et du tableau `AGENTS.md` pour les 8 langages de programmation : **Rust** (`cargo-mutants`), **TypeScript/JS** (`Stryker`), **Python** (`mutmut`), **Go** (`gremlins`), **Kotlin/JVM** (`PITest`), **C#/.NET** (`Stryker.NET`), **PHP** (`Infection`) et **Dart** (`mutation_test`).
  - Ajout de l'ingestion native des rapports Go `gremlins` JSON, Python `mutmut` JSON et Kotlin/JVM `PITest` XML en complément de `cargo-mutants` et `mutation-testing-elements` (`Stryker` / `Stryker.NET` / `Infection`), plus durcissement `GF081` de `stryker-config.json` et `infection.json`/`infection.json5`.
  - Relèvement du taux de destruction de mutants par défaut (`DEFAULT_MIN_MUTATION_SCORE` et défaut CLI `--mutants`) à **100%**.

### Modifié

- Refactorisation de tous les tuples anonymes de 3+ éléments du workspace Garde-fou en structures nommées et paires imbriquées pour un dogfooding à 100% sous `GF073` (`max_tuple_arity = 2`).
- Relèvement du seuil `[mutation] min_mutation_score` dans `garde-fou.toml` de `95%` à `100%`.

## [0.3.0] - 2026-11-26

### Ajouté

- **TUI style Charm, Live Watch & Pilotage CLI (`gf-cli`)** :
  - Moteur de rendu terminal pur Rust sûr inspiré de Lipgloss et Miette avec isolation stricte `NO_COLOR` et `--format agent|sarif`.
  - Mode surveillance temps réel (`garde check --watch`) et tableau de bord interactif multi-onglets (`garde tui` / `garde check -i`) avec copie presse-papiers OSC 52.
  - Configurateur interactif sans perte par cases à cocher (`garde config`) propulsé par `toml_edit` pour activer/désactiver archétypes, langages, frameworks, fonctionnalités et politiques sans perdre les commentaires TOML.
  - Sous-commandes de gestion du cache (`garde cache status|prune|clean`) et désinstalleur chirurgical (`garde uninstall --project|--self`, `garde eject`).
  - Localisation native FR/EN du CLI (`--locale auto|fr|en`, `locales/fr.toml`, `locales/en.toml`) vérifiée en dogfooding sous `GF062`.
- **Doctrine Exclusive des Chaînes d'Outils Modernes (`GF081`)** :
  - Impose `pnpm` pour JS/TS/Node (interdisant `npm`/`yarn`), `uv` pour Python (interdisant `pip`/`poetry`/`pipenv`/`setup.py`) et Gradle Kotlin DSL (`build.gradle.kts` + `libs.versions.toml`, interdisant Groovy `build.gradle`).
- **Langages Compilés — Go, Kotlin & C# 12/13 (`GF071`, `GF075`, `GF080`–`GF082`)** :
  - Analyseurs AST natifs `Tree-sitter` pour Go (`staticcheck`, satisfaction structurelle implicite d'interfaces), Kotlin (coroutines, `runCatching`, Gradle `.kts`) et C# 12/13 (constructeurs primaires `CS9124`, `async`/`CancellationToken`), plus ingestion `golangci-lint`, `go vet`, `detekt`, `ktlint` et `dotnet build`.
- **Node.js ESM & Stack MERN TypeScript** :
  - Garde-fous pour les imports protocole `node:`, middlewares/sécurité Express 5 et anti-injection NoSQL MongoDB/Mongoose.
- **Écosystèmes Industriels Python (`uv` exclusif)** :
  - Règles natives pour **Django 5.2/6.0**, **NiceGUI** (`io_bound`/`cpu_bound`/`app.storage`), **PyTorch / TensorFlow / Keras 3** (anti-pickle/HDF5, autograd VRAM, anti-fuite de données) et **LangChain / LangGraph** (`StateGraph` `END`, `recursion_limit`, anti-RCE/SSTI).
- **SQL Avancé & Bases de Données Procédurales (`GF080`–`GF082`)** :
  - **MariaDB** (`utf8mb4`, Online DDL, `COMMIT` implicite), **PostgreSQL Avancé** (RLS multi-tables, `SECURITY DEFINER`, PL/pgSQL `INTO STRICT`), **Oracle PL/SQL** (`AUTHID CURRENT_USER`, `FORALL`, `BULK COLLECT LIMIT`, `utPLSQL`) et ingestion JSON `zpa-cli`.
- **Diagrammes-as-Code & `i18n` Universel v2 (`GF062`, `GF080`)** :
  - Analyseurs pur Rust sûr **Mermaid v11** et **PlantUML** (structure, anti-XSS/anti-SSRF) pour fichiers autonomes et blocs de code fencés Markdown/Quarto.
  - Moteur `i18n` universel v2 supportant **ICU MessageFormat**, parité des variables (`I18N-PLACEHOLDER-MISMATCH`), Gettext `.po`/`.pot`, Flutter `.arb`, Fluent `.ftl` et Symfony XLIFF `.xlf`/`.xliff`.
- **PHP 8.4, Symfony 7, Doctrine ORM 3 & Twig 3** :
  - Analyseurs pur Rust sûr PHP 8.4 (`declare(strict_types=1)`, anti-`eval`/`unserialize`, superglobales, égalité lâche), Symfony 7 & Doctrine ORM 3 (service locator, injection DQL/SQL, `flush()` en boucle, `#[IsGranted]`, entités non-`final`), Twig 3 (`|raw`, style/script inline, `|trans`), graphe de cycles PSR-4 (`GF071`) et parseurs JSON `PHPStan`/`Psalm`.
- **Dart 3.6+ & Flutter 3.x** :
  - Lexer de commentaires imbriqués `/* /* */ */`, `use_build_context_synchronously` (`FLUTTER-BUILD-CONTEXT-ASYNC`), vérifications de cycle de vie `dispose()`, tokens de thème Material 3, tooltips a11y `IconButton`, exemptions `*.g.dart`/`*.freezed.dart`, graphe d'imports `package:` (`GF071`) et parseur JSON `dart analyze`.
- **Modding Minecraft (Kilt / Fabric / Quilt / NeoForge / Mixin) & Data Packs `.mcfunction` 1.21+** :
  - Garde-fous Mixin (`@Overwrite`, `@Redirect`, client-en-commun, `Thread.sleep` sur le thread tick), syntaxe Brigadier 1.21+ `.mcfunction` (macros `$` / `$(param)`, dossier singulier `data/<ns>/function/`, sélecteurs `@e` non bornés, cycles de récursion mutuelle `GF071`), audit Tier 2 (`pack.mcmeta`, `fabric.mod.json`, `*.mixins.json`) et parseur JSON `Spyglass`.

### Modifié

- Mise à niveau du schéma de cache incrémental `BLAKE3` vers `v22` pour couvrir tous les nouveaux extracteurs et métadonnées de workspace.
- Regroupement des violations d'écosystèmes Tier 1 dans `gf-core` (`extended_violations.rs` + `lang_rule_from.rs`) afin de préserver les invariants DAG acyclique (`GF071`) et fan-out (`GF007`).
- Reconnaissance de `CHANGELOGS.md` et des variantes localisées (`README.<lang>.md`, `CHANGELOGS.<lang>.md`) comme documents de gouvernance racine sous `GF063`.

## [0.2.1] - 2026-11-26

### Ajouté

- **Arsenal Proportionnel & 6 Archétypes de Workspace** :
  - Refonte de `garde init` avec zéro langage par défaut sur invocation vide et adaptation topologique pour `minimal`, `scripts` (PEP 723, scripts racine), `infra` (Docker Compose sans Rust), `knowledge` (Obsidian OFM `[[WikiLinks]]`, `![[embeds]]`, `.canvas`), `academic` (Jupyter `.ipynb` & Quarto `.qmd`) et `software`.

### Modifié

- Relèvement de la MSRV (`rust-version`) à `1.100` et épinglage de `garde-fou` dans `mise.toml`.

## [0.2.0] - 2026-11-26

### Ajouté

- **Ségrégation Architecturale des 6 Domaines (`GF060`–`GF066`)** :
  - Séparation hermétique des domaines `Code`, `Scripts`, `Forme`, `Texte`, `Contenus` et `Asset` avec détection de fuites inter-domaines, design tokens, symétrie `i18n`, pureté Markdown, intégrité des assets et `a11y` statique.
- **Forteresses Anti-Bypass, Tarjan DAG, Boundaries, KISS/YAGNI & TDD/Mutations (`GF052`, `GF070`–`GF077`)** :
  - Détecteur AST universel de contournements/suppressions et sceau cryptographique `BLAKE3` `garde.lock` (`garde lock`, `GF070`).
  - Détecteur déterministe `#![no_std]` de cycles de dépendances Tarjan (`GF071`) et contraintes `[[boundaries]]` / Atomic Design (`GF072`).
  - Détecteurs KISS/YAGNI (`GF073`–`GF075`), forteresse TDD d'assertions/parité (`GF076`), hook Conventional Commits (`garde commit-msg`, `GF077`) et porte de mutation (`GF052`).
- **Cœur Agentique (`garde sync`)** :
  - Assemblage modulaire de `AGENTS.md` à partir de fragments embarqués, installation `skills.sh` et vérification supply-chain.
- **Pipeline d'Ingestion d'Écosystèmes 3 Niveaux (`GF080`–`GF082`)** :
  - Analyseurs AST natifs, durcissement de configurations Tier 2 et parseurs d'outils externes Tier 3 pour Python (`ruff`, `mypy`, `pyright`), TypeScript & React 19 (`oxlint`, `biome`, `tsc`, JSONC `tsconfig`), POSIX Shell / Bash / PowerShell & pont AST `Dockerfile RUN` (`shellcheck`, `hadolint`, `PSScriptAnalyzer`), YAML 1.2 / Docker Compose / GitHub Actions (`yamllint`, `dclint`, `actionlint`, `zizmor`), CSS / XML / SVG / TOML / Markdown (`stylelint`, `xmllint`, `taplo`, `rumdl`, `markdownlint`), SQL & migrations PostgreSQL sans coupure (`squawk`, `sqruff`, `sqlfluff`), et stacks Web/Edge (Astro, React Router v7, Panda CSS, Material 3 Expressive, W3C HTML5 / WCAG 2.2 AA, Cloudflare Workers / Wrangler, Drizzle ORM).

## [0.1.0] - 2026-11-23

### Ajouté

- **Architecture Functional Core / Imperative Shell** :
  - Cœur `#![no_std]` + `alloc` (`gf-core`, `gf-rules`), parseurs `Tree-sitter` -> `UAST` sans E/S (`gf-parsers`), moteur de cache incrémental `BLAKE3` (`gf-engine`) et binaire CLI `garde` (`gf-cli`) avec formateurs `human`, `agent` et `sarif`.
- **Règles Fondamentales de Gouvernance (`GF000`–`GF050`)** :
  - Métriques structurelles SRP (`GF000`–`GF009`), détection de clones AST Type-2 (`GF020`), exports publics morts (`GF021`), sécurité Docker (`GF030`–`GF034`), détection native de secrets & portes `.gitignore` / Gitleaks / OSV-Scanner (`GF040`–`GF043`), et porte de couverture LCOV (`GF050`).
- **Distribution & Pipeline de Release Multi-OS** :
  - Scaffolding `garde init`, intégration `mise`, installeurs one-liner vérifiés par `SHA256SUMS` (`scripts/install.sh`, `scripts/install.ps1`) et workflow GitHub Actions multi-OS (`x86_64-unknown-linux-musl`, `aarch64-unknown-linux-gnu`, `x86_64-apple-darwin`, `aarch64-apple-darwin`, `x86_64-pc-windows-msvc`).

[0.3.0]: https://github.com/ArchiPiXL/garde-fou/compare/v0.2.1...v0.3.0
[0.2.1]: https://github.com/ArchiPiXL/garde-fou/compare/v0.2.0...v0.2.1
[0.2.0]: https://github.com/ArchiPiXL/garde-fou/compare/v0.1.0...v0.2.0
[0.1.0]: https://github.com/ArchiPiXL/garde-fou/releases/tag/v0.1.0
