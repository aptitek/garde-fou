# Garde-fou (`garde`)

**Garde-fou** est un orchestrateur et analyseur statique polyglotte ultra-strict inspiré de l'architecture Hub-and-Spoke de Pandoc (`Tree-sitter` -> `UAST` `#![no_std]` -> Règles pures + Ponts `SARIF` / `LCOV` + Cache incrémental `BLAKE3` `v3`).

Le binaire CLI s'appelle **`garde`** (`garde.exe` sous Windows) et lit la configuration stricte `garde-fou.toml`.

---

## 1. Installation

### Via `mise` (recommandé)

En global ou directement dans le `mise.toml` d'un projet :

```bash
mise use -g github:aptitek/garde-fou
```

```toml
# mise.toml
[tools]
"github:aptitek/garde-fou" = "latest"
```

### One-Liner Linux & macOS (vérification `SHA256SUMS` obligatoire)

```bash
curl -fsSL https://github.com/aptitek/garde-fou/releases/latest/download/install.sh | sh
```

Avec options explicites (`--version`, `--install-dir`) :

```bash
curl -fsSL https://github.com/aptitek/garde-fou/releases/latest/download/install.sh | sh -s -- --version v0.1.0 --install-dir "$HOME/.local/bin"
```

### One-Liner Windows PowerShell 5.1+ / `pwsh` 7+ (vérification `SHA256SUMS` obligatoire)

```powershell
irm https://github.com/aptitek/garde-fou/releases/latest/download/install.ps1 | iex
```

### Installation développeur locale (`task install`)

```bash
task install
# équivalent à : cargo install --path crates/gf-cli --locked --force
```

---

## 2. Ségrégation Architecturale des 6 Domaines

`garde` impose une séparation hermétique entre six domaines orthogonaux (configurables via `[domains]` dans `garde-fou.toml`) :

1. **`Code` (`src/**`, `crates/**`, `app/**`)** : logique métier et structure sémantique pure (zéro CSS-in-JS, zéro littéral couleur/px, zéro `<svg>` inline, accessibilité statique `a11y` vérifiée).
2. **`Scripts` (`scripts/**`, `tools/**`)** : outillage autonome soumis aux plafonds SRP/secrets, jamais importé par `Code`, et obligatoirement couvert par des tests automatisés.
3. **`Forme` (`styles/**`, `tokens/**`, `panda.config.*`)** : Design Tokens primitifs confinés dans `tokens/` et feuilles CSS consommant exclusivement `var(--token)` sans `!important` ni texte traduisible.
4. **`Texte` (`locales/**`, `i18n/**`, `messages/**`)** : dictionnaires `i18n` purs (zéro HTML), strictement symétriques entre langues et synchronisés avec les appels `t("...")` du `Code`.
5. **`Contenus` (`content/**`, `courses/**`, `docs/**`)** : Markdown/MDX éditorial pur (zéro HTML brut hors blocs de code, hiérarchie de titres séquentielle, images avec `alt`).
6. **`Asset` (`assets/**`, `public/**`, `static/**`)** : médias et SVG confinés, sains (SVG sans `<script>` ni `on*=`), sous plafond de taille (`1 MiB`), sans liens brisés ni fichiers orphelins.

---

## 3. Utilisation Rapide

```bash
# Initialiser un projet clé en main (Rust, Web 6-domaines, Python/FastAPI, Docker)
garde init
garde init mon-service --lang rust --feature docker,git-hooks
garde init portail-ui --lang typescript --framework astro --feature ui,i18n,content,docker
garde init api-commandes --lang python --framework fastapi --feature docker,ci-github

# Analyse incrémentale ultra-rapide sur les fichiers stagés (idéale en pre-commit + agents IA)
garde check --staged --format agent

# Analyse complète sans cache avec exigence stricte des scanners externes et porte LCOV
garde check --no-cache --strict-tools --lcov lcov.info

# Export OASIS SARIF 2.1.0 pour GitHub Code Scanning
garde check --format sarif
```

---

## 4. Catalogue des Règles (`GF000`–`GF066`)

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

---

## 5. Architecture Interne

| Crate | Environnement | Rôle |
| :--- | :--- | :--- |
| `gf-core` | `#![no_std]` + `alloc` | Modèle algébrique pur (`Span`, `RepoPath`, `DomainKind`, `UastDocument`, `ProjectContext`, `gitignore`, `lcov`, `Validation`) |
| `gf-rules` | `#![no_std]` + `alloc` | Moteur de règles locales, globales (`GF020`/`GF021`), Docker (`GF030`–`GF034`), sécurité (`GF040`/`GF050`) et 6-domaines (`GF060`–`GF066`) |
| `gf-parsers` | `std` pur (0 I/O) | Extracteurs `Tree-sitter` Safe Rust (`Rust`, `TS`/`TSX`/`JS`, `Python`), `CSS`, `Markdown`/`MDX`, `i18n`, `SVG`, `Dockerfile` et `GF040` |
| `gf-engine` | Shell impératif | Cache `BLAKE3` `.garde-fou-cache/v3/`, `garde init` 6-domaines, découverte légère d'assets binaires, orchestration `SARIF` |
| `gf-cli` | Binaire `garde` | Interface `clap` et formateurs (`human`, `agent` Markdown, `sarif` 2.1.0) |
