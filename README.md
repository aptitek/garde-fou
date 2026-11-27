# Garde-fou (`garde`)

**Garde-fou** est un orchestrateur et analyseur statique polyglotte ultra-strict inspiré de l'architecture Hub-and-Spoke de Pandoc (`Tree-sitter` -> `UAST` `#![no_std]` -> Règles pures + Ingestion d'outils par langage + Ponts `SARIF` / `LCOV` + Cache incrémental `BLAKE3` `v22`).

Le binaire CLI s'appelle **`garde`** (`garde.exe` sous Windows) et lit la configuration stricte `garde-fou.toml`.

---

## 1. Installation

### Via `mise` (recommandé)

En global ou directement dans le `mise.toml` d'un projet :

```bash
mise use -g github:ArchiPiXL/garde-fou
```

```toml
# mise.toml
[tools]
"github:ArchiPiXL/garde-fou" = "latest"
```

### One-Liner Linux & macOS (vérification `SHA256SUMS` obligatoire)

```bash
curl -fsSL https://github.com/ArchiPiXL/garde-fou/releases/latest/download/install.sh | sh
```

Avec options explicites (`--version`, `--install-dir`) :

```bash
curl -fsSL https://github.com/ArchiPiXL/garde-fou/releases/latest/download/install.sh | sh -s -- --version v0.3.0 --install-dir "$HOME/.local/bin"
```

### One-Liner Windows PowerShell 5.1+ / `pwsh` 7+ (vérification `SHA256SUMS` obligatoire)

```powershell
irm https://github.com/ArchiPiXL/garde-fou/releases/latest/download/install.ps1 | iex
```

### Installation développeur locale (`task install`)

```bash
task install
# équivalent à : cargo install --path crates/gf-cli --locked --force
```

---

## 2. Langages, Fonctionnalités & Frameworks Supportés

### Langages & Formats Analysés (23 `LanguageId` + formats spécialisés)

| Catégorie | Langages / Formats | Slugs (`--lang` / `garde-fou.toml`) |
| :--- | :--- | :--- |
| **Systèmes & Backend** | Rust (Edition 2024), Go, Kotlin / JVM, C# / .NET, PHP 8.4, Python | `rust`, `go`, `kotlin`, `csharp`, `php`, `python` |
| **Web, UI & Mobile** | TypeScript / JavaScript (`TS`/`TSX`/`JS`/`JSX`), Astro, HTML5, CSS / SCSS, Twig 3, Dart 3.6+ / Flutter 3.x | `typescript`, `astro`, `html`, `css`, `twig`, `dart` |
| **Bases de Données** | SQL multi-dialectes (`PostgreSQL`, `MariaDB`, `SQLite`), Oracle PL/SQL | `sql`, `plsql` |
| **Automatisation & Infra** | POSIX Shell / Bash, PowerShell, `Dockerfile` / `Containerfile`, YAML 1.2 (Compose & GitHub Actions), TOML, XML / SVG | `shell`, `powershell`, `yaml`, `toml`, `xml` |
| **Contenus, Savoir & Diagrammes** | Markdown / GFM / MDX, Jupyter (`.ipynb`), Quarto (`.qmd`), Obsidian Canvas (`.canvas`), Mermaid v11, PlantUML | `markdown`, `mermaid`, `plantuml` |
| **Localisation & Modding** | JSON / JSONC, GNU Gettext (`.po`/`.pot`), Project Fluent (`.ftl`), ARB / XLIFF, Minecraft 1.21+ `.mcfunction` | `mcfunction` |

### Fonctionnalités Transversales (`features`)

| Feature (`--feature`) | Capacités & Garde-fous |
| :--- | :--- |
| `docker` | Sécurité `Dockerfile` / `Containerfile` (`GF030`–`GF034`), `.dockerignore`, bridges AST `RUN` |
| `git-hooks` | Hooks Git `lefthook` (`pre-commit`, `commit-msg` Conventional Commits `GF077`, `pre-push`) |
| `ci-github` | Durcissement GitHub Actions (`permissions`, pinning SHA/tag, anti-injection, `actionlint`/`zizmor`) |
| `task-runner` | Intégration `Taskfile.yml`, `justfile` et verrouillage reproductible `mise.toml` |
| `ui` | Ségrégation `Code`/`Forme`, Design Tokens (`GF060`–`GF061`) et accessibilité statique `a11y` (`GF066`) |
| `i18n` | Dictionnaires `Texte` symétriques sans HTML (`GF062`), ICU / Gettext / Fluent / ARB / XLIFF |
| `content` | Pureté éditoriale Markdown / MDX (`GF063`), frontmatter YAML 1.2, hiérarchie de titres et ancres |
| `tdd` | Forteresse TDD (`GF076`) : couverture `LCOV` (`GF050`), mutations (`GF052`), parité tests `1:1` |
| `storybook` | Parité obligatoire `.stories.tsx` (`GF076`) |
| `panda-css` | Garde-fous Panda CSS zéro-runtime (`panda.config.*`, `styled-system/`) |
| `m3e` | Material Design 3 Expressive (appariement tonal, ressorts Spatial/Effects, cibles `48dp`) |
| `w3c` | Modèles de contenu W3C HTML5, WAI-ARIA 1.2 et conformité WCAG 2.2 AA/AAA |
| `ai-ml` | Gouvernance ML / Deep Learning & Agents LLM (`PyTorch`, `Keras 3`, `LangChain`/`LangGraph`) |
| `diagrams` | Diagrammes-as-Code (`Mermaid` v11 & `PlantUML`) |

### Frameworks & Écosystèmes (`frameworks`) & Archétypes

- **Web, Edge & Node.js** : `react`, `astro`, `nextjs`, `react-router`, `cloudflare-workers`, `drizzle`, `nodejs`, `express`, `mongoose`
- **Python & IA/ML** : `fastapi`, `django`, `nicegui`, `pytorch`, `tensorflow`, `langchain`
- **Backend, Mobile & Bases de Données** : `cargo-workspace`, `symfony`, `doctrine`, `flutter`, `postgresql`, `mariadb`, `docker-compose`
- **Savoir, Académique & Modding** : `obsidian`, `quarto`, `jupyter`, `minecraft-mod`, `kilt`, `datapack`
- **6 Archétypes Proportionnels (`[project] archetype`)** : `minimal`, `scripts`, `infra`, `knowledge`, `academic`, `software`

### Fonctionnalités Clés du CLI (`garde`)

- **Analyse polyglotte 3 niveaux** : AST natif zéro-dépendance (`GF080`), audit anti-affaiblissement des configs d'outils (`GF081`), et ingestion unifiée d'outils externes / `SARIF` 2.1.0 (`GF082`, `gitleaks`, `osv-scanner`).
- **Expérience développeur & agents IA** : TUI interactif (`garde tui`), mode surveillance temps réel (`garde check --watch`), configurateur sans perte (`garde config`), sorties `human` / `agent` Markdown / `sarif`, CLI bilingue FR/EN, synchronisation `AGENTS.md` + `.agents/skills/` (`garde sync`) et scellement cryptographique `BLAKE3` (`garde lock`).

---

## 3. Ségrégation Architecturale des 6 Domaines

`garde` impose une séparation hermétique entre six domaines orthogonaux (configurables via `[domains]` dans `garde-fou.toml`) :

1. **`Code` (`src/**`, `crates/**`, `app/**`)** : logique métier et structure sémantique pure (zéro CSS-in-JS, zéro littéral couleur/px, zéro `<svg>` inline, accessibilité statique `a11y` vérifiée).
2. **`Scripts` (`scripts/**`, `tools/**`)** : outillage autonome soumis aux plafonds SRP/secrets, jamais importé par `Code`, et obligatoirement couvert par des tests automatisés.
3. **`Forme` (`styles/**`, `tokens/**`, `panda.config.*`)** : Design Tokens primitifs confinés dans `tokens/` et feuilles CSS consommant exclusivement `var(--token)` sans `!important` ni texte traduisible.
4. **`Texte` (`locales/**`, `i18n/**`, `messages/**`)** : dictionnaires `i18n` purs (zéro HTML), strictement symétriques entre langues et synchronisés avec les appels `t("...")` du `Code`.
5. **`Contenus` (`content/**`, `courses/**`, `docs/**`)** : Markdown/MDX éditorial pur (zéro HTML brut hors blocs de code, hiérarchie de titres séquentielle, images avec `alt`).
6. **`Asset` (`assets/**`, `public/**`, `static/**`)** : médias et SVG confinés, sains (SVG sans `<script>` ni `on*=`), sous plafond de taille (`1 MiB`), sans liens brisés ni fichiers orphelins.

---

## 4. Utilisation Rapide

```bash
# Initialiser un projet clé en main (assistant interactif style Gum sur TTY, ou drapeaux déclaratifs)
garde init
garde init -i
garde init -y
garde init mon-service --lang rust --feature docker,git-hooks
garde init portail-ui --lang typescript --framework astro --feature ui,i18n,content,docker
garde init api-commandes --lang python --framework fastapi --feature docker,ci-github

# Poste de pilotage interactif 3 onglets (Diagnostics Live, Configurateur cases à cocher, Cache)
garde tui
garde check -i

# Surveillance temps réel sur sauvegarde (poll 250 ms + debounce 120 ms, zéro dépendance FFI)
garde check --watch

# Configurateur interactif sans perte de garde-fou.toml (archétype radio + cases à cocher)
garde config

# Gestion du cache BLAKE3 (.garde-fou-cache/)
garde cache status
garde cache prune
garde cache clean

# Désinstallation chirurgicale (projet, binaire ou assistant interactif 3 choix)
garde uninstall --project --dry-run
garde eject -y
garde uninstall --self --dry-run

# Analyse incrémentale ultra-rapide sur les fichiers stagés (idéale en pre-commit + agents IA)
garde check --staged --format agent

# Analyse complète sans cache avec exigence stricte des scanners externes, porte LCOV et contrôle couleur
garde check --no-cache --strict-tools --lcov lcov.info --color auto

# Export OASIS SARIF 2.1.0 pour GitHub Code Scanning
garde check --format sarif

# Synchroniser le bloc modulaire AGENTS.md, les skills .agents/skills/ (skills.sh) et garde.lock
garde sync

# Sceller cryptographiquement garde-fou.toml dans garde.lock (BLAKE3)
garde lock

# Valider un message de commit Conventional Commits (hook Git commit-msg)
garde commit-msg .git/COMMIT_EDITMSG
```

---

## 5. Catalogue des Règles (`GF000`–`GF082`)

| Règle | Domaine / Portée | Description | Seuil par défaut |
| :--- | :--- | :--- | :--- |
| `GF000` | Fichier | Erreur de syntaxe CST (`ERROR` / `MISSING` `Tree-sitter`, `Dockerfile` ou dictionnaire `i18n`) | `0` |
| `GF001` | `Code` / `Scripts` | Nombre maximal de lignes par fichier source ou script | `<= 250` |
| `GF002` | Fonction | Nombre maximal de lignes effectives par fonction / méthode / closure | `<= 50` |
| `GF003` | Fonction | Nombre maximal de paramètres par signature (hors `self` / `cls`) | `<= 4` |
| `GF004` | Fonction | Complexité cognitive maximale (avec pénalité d'imbrication) | `<= 10` |
| `GF005` | Fonction | Complexité cyclomatique `McCabe` maximale | `<= 10` |
| `GF006` | Fonction | Profondeur maximale d'imbrication de flux de contrôle | `<= 4` |
| `GF007` | Fichier | Couplage sortant maximal (`imports` / `use` par fichier) | `<= 15` |
| `GF008` | Type | Nombre maximal de méthodes par `struct` / `impl` / `trait` / `class` | `<= 10` |
| `GF010` | Fichier | Constructions interdites (`unsafe`, `unwrap`, `expect`, `panic!`, `todo!`, `dbg!`) | `0` |
| `GF020` | Workspace | Duplication structurelle d'AST Type-2 (empreinte `BLAKE3` 128 bits normalisée `$ID`/`$LIT`) | `>= 15` lignes & `>= 40` nœuds |
| `GF021` | `Code` | Export public mort (`pub` / `export` / symbole Python public non référencé hors points d'entrée) | `0` |
| `GF030` | Docker | Image de base `FROM` non épinglée ou utilisant `:latest` | `0` |
| `GF031` | Docker | Stage final s'exécutant sous `root` (absence de `USER` non-privilégié) | `0` |
| `GF032` | Docker | Danger de sécurité (`sudo` dans `RUN`, `ADD` local non-archive, secret dans `ENV`/`ARG`) | `0` |
| `GF033` | Docker | Hygiène de layer (`WORKDIR` relatif, cache `apt`/`pip` non purgé) | `0` |
| `GF034` | Docker | Absence ou incohérence de `.dockerignore` vis-à-vis des langages détectés | `0` |
| `GF040` | Fichier | Détection native zéro dépendance de secrets / clés privées PEM (valeur systématiquement masquée) | `0` |
| `GF041` | Scanner | Secret détecté par `gitleaks` (ingestion `SARIF` 2.1.0 unifiée) | `0` |
| `GF042` | Scanner | Vulnérabilité dépendance détectée par `osv-scanner` (ingestion `SARIF` 2.1.0 unifiée) | `0` |
| `GF043` | Scanner | Binaire externe requis (`gitleaks` / `osv-scanner`) introuvable dans `PATH` (`required` / `--strict-tools`) | `0` |
| `GF050` | Workspace | Couverture de lignes `LCOV` (`lcov.info`) inférieure au seuil minimal exigé | `min_line_coverage` (`85 %`) |
| `GF060` | `Code` vs `Forme` | Feuille `.css`/`.scss` hors `Forme`, ou fuite CSS-in-JS / `style=` / `#hex` / unité CSS dans `Code` | `0` |
| `GF061` | `Forme` | Couleur brute, `px` magique ou `!important` hors `tokens/`, ou fuite `content: "Texte"` / `url(data:)` | `0` |
| `GF062` | `Texte` / `i18n` | HTML dans `Texte`, asymétrie de clés entre langues, clé `t("...")` manquante/morte, ou texte JSX hors `t()` | `0` |
| `GF063` | `Contenus` | HTML brut hors code fencé/inline, logique/style MDX interdit, saut de titres, image sans `alt`, ou `.md` égaré | `0` |
| `GF064` | `Asset` | Asset hors `assets/`, `> max_asset_bytes`, SVG dangereux (`<script>`/`on*=`), `<svg>`/`data:` inline, lien cassé ou orphelin | `0` |
| `GF065` | `Scripts` | Import de `Scripts` depuis `Code`, ou script dans `scripts/`/`tools/` dépourvu de couverture de test | `0` |
| `GF066` | `Code` (`a11y`) | `<div/span/p onClick>` sans rôle clavier, `<img>` sans `alt`, `<button>` sans `type` ou sans label accessible | `0` |
| `GF070` | Gouvernance & Intégrité | Bypass linter (`eslint-disable`, `@ts-ignore`, `#[allow]`), `garde.lock` altéré ou dérive du bloc géré `AGENTS.md` / `.agents/skills/` (`garde sync`) | `0` |
| `GF071` | Architecture | Cycle de dépendances inter-fichiers détecté par l'algorithme SCC de Tarjan (`#![no_std]`) | `0` cycle |
| `GF072` | Architecture | Violation des frontières déclaratives `[[boundaries]]` ou de la hiérarchie Atomic Design (`atoms -> molecules -> organisms`) | `0` |
| `GF073` | KISS | Ternaire imbriqué ou train d'accès de propriétés violant la Loi de Déméter (`a.b.c.d.e`) | `0` ternaire imbriqué / `<= 4` segments |
| `GF074` | YAGNI (local) | Barrel file `export *` / wildcard import, excès de drapeaux booléens ou marqueur `TODO`/`FIXME`/`HACK` dans `Code` | `<= 1` paramètre `bool` |
| `GF075` | YAGNI (global) | Contrat comportemental (`trait` / `interface` / `Protocol` avec méthodes) hors `entrypoints` ayant `<= 1` implémentation | `>= 2` implémentations |
| `GF076` | Forteresse TDD | Test sans assertion réelle, assertion tautologique (`assert!(true)`), ou absence de test/`stories` compagnon (`1:1`) | `>= 1` vraie assertion |
| `GF077` | Git Hygiene | Message de commit non conforme aux Conventional Commits (`<type>(<scope>): <description>`) ou résumé vague (`wip`, `misc`) | `<= 100` caractères |
| `GF080` | Langage AST (Tier 1) | Idiomes & sécurité AST natifs sans outil externe (`PY-B006`, `PY-E722`/`PY-B001`, `PY-S102`/`PY-S301`/`PY-S602`, `PY-ANN401` `: Any`, `PY-T201` `print`) | `0` |
| `GF081` | Configs Outils (Tier 2) | Affaiblissement de `pyproject.toml` (`[tool.mypy] strict = false`), `ruff.toml` (`ignore = ["ALL"]`) ou `tsconfig.json` (`"strict": false`) | `0` |
| `GF082` | Outils Écosystème (Tier 3) | Diagnostic ingéré depuis les vérificateurs externes (`ruff`, `mypy --strict`, `pyright`) résolus via `.venv` / `PATH` / `uv` | `0` |

---

## 6. Architecture Interne

| Crate | Environnement | Rôle |
| :--- | :--- | :--- |
| `gf-core` | `#![no_std]` + `alloc` | Modèle algébrique pur (`Span`, `RepoPath`, `DomainKind`, `BoundaryRule`, `CommitConfig`, `ToolsConfig`, `UastDocument`, `Validation`) |
| `gf-rules` | `#![no_std]` + `alloc` | Moteur de règles locales, globales (`GF020`/`GF021`), Tarjan DAG (`GF071`), `[[boundaries]]` (`GF072`), KISS/YAGNI/TDD (`GF073`–`GF076`), écosystème (`GF080`–`GF082`) |
| `gf-parsers` | `std` pur (0 I/O) | Extracteurs `Tree-sitter` Safe Rust (`Rust`, `TS`/`TSX`/`JS`, `Python` + guardrails `GF080`), audit configs `GF081`, `CSS`, `Markdown`/`MDX`, `i18n`, `SVG`, `Dockerfile` |
| `gf-engine` | Shell impératif | Cache `BLAKE3` `.garde-fou-cache/v5/`, `garde sync` (`AGENTS.md` + `skills.sh`), `garde.lock`, `garde init`, adaptateurs `uv`-aware (`GF082`), orchestration `SARIF` |
| `gf-cli` | Binaire `garde` | Sous-commandes `check`, `tui`, `init`, `sync`, `lock`, `commit-msg`, moteur TUI/Lipgloss pur Rust (`ui/`, `tui/`) et formateurs (`human`, `agent` Markdown, `sarif` 2.1.0) |
