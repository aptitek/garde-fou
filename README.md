# Garde-fou (`garde`)

**Garde-fou** est un orchestrateur et analyseur statique polyglotte ultra-strict inspiré de l'architecture Hub-and-Spoke de Pandoc (`Tree-sitter` -> `UAST` `#![no_std]` -> Règles pures + Ponts `SARIF` / `LCOV` + Cache incrémental `BLAKE3`).

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

### via `cargo-binstall` ou compilation locale développeur

```bash
# Depuis les sources locales verrouillées :
task install
# équivalent à : cargo install --path crates/gf-cli --locked --force
```

---

## 2. Utilisation Rapide

```bash
# Initialiser un nouveau projet clé en main (façon `uv init`) ou adopter un dépôt existant
garde init
garde init mon-service --lang rust --feature docker --feature git-hooks
garde init portail-web --lang typescript --framework astro --feature docker
garde init api-commandes --lang python --framework fastapi --feature docker --feature ci-github

# Analyse incrémentale ultra-rapide sur les fichiers stagés (idéale en pre-commit + agents IA)
garde check --staged --format agent

# Analyse complète sans cache avec exigence stricte des scanners externes et porte LCOV
garde check --no-cache --strict-tools --lcov lcov.info

# Export OASIS SARIF 2.1.0 pour GitHub Code Scanning
garde check --format sarif
```

---

## 3. Catalogue des Règles (`GF000`–`GF050`)

| Règle | Portée | Description | Seuil par défaut |
| :--- | :--- | :--- | :--- |
| `GF000` | Fichier | Erreur de syntaxe CST (`ERROR` / `MISSING` `Tree-sitter` ou `Dockerfile`) | `0` |
| `GF001` | Fichier | Nombre maximal de lignes par fichier source (SRP fichier) | `<= 250` |
| `GF002` | Fonction | Nombre maximal de lignes effectives par fonction / méthode / closure | `<= 50` |
| `GF003` | Fonction | Nombre maximal de paramètres par signature (hors `self` / `cls`) | `<= 4` |
| `GF004` | Fonction | Complexité cognitive maximale (avec pénalité d'imbrication) | `<= 10` |
| `GF005` | Fonction | Complexité cyclomatique `McCabe` maximale | `<= 10` |
| `GF006` | Fonction | Profondeur maximale d'imbrication de flux de contrôle | `<= 4` |
| `GF007` | Fichier | Couplage sortant maximal (`imports` / `use` par fichier) | `<= 15` |
| `GF008` | Type | Nombre maximal de méthodes par `struct` / `impl` / `trait` / `class` | `<= 10` |
| `GF010` | Fichier | Constructions paniquantes ou non sûres interdites (`unsafe`, `unwrap`, `expect`, `panic!`, `todo!`, `dbg!`) | `0` |
| `GF020` | Workspace | Duplication structurelle d'AST Type-2 (identifiants/littéraux renommés, empreinte `BLAKE3` 128 bits) | `>= 15` lignes & `>= 40` nœuds |
| `GF021` | Workspace | Export public mort (`pub` / `export` / symbole Python public non référencé hors points d'entrée) | `0` |
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

---

## 4. Architecture Interne

| Crate | Environnement | Rôle |
| :--- | :--- | :--- |
| `gf-core` | `#![no_std]` + `alloc` | Modèle algébrique pur (`Span`, `RepoPath`, `UastDocument`, `ProjectContext`, `gitignore`, `lcov`, `Validation`) |
| `gf-rules` | `#![no_std]` + `alloc` | Moteur de règles statiques locales, globales (`GF020`/`GF021`), Docker (`GF030`–`GF034`) et sécurité (`GF040`/`GF050`) |
| `gf-parsers` | `std` pur (0 I/O) | Extracteurs `Tree-sitter` 100 % Safe Rust (`Rust`, `TS`/`TSX`/`JS`, `Python`), `Dockerfile` et détecteur `GF040` |
| `gf-engine` | Shell impératif | Cache `BLAKE3` `.garde-fou-cache/v2/`, `garde init`, découverte Git, orchestration `SARIF` (`gitleaks`, `osv-scanner`) |
| `gf-cli` | Binaire `garde` | Interface `clap` et formateurs (`human`, `agent` Markdown, `sarif` 2.1.0) |
