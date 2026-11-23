# AGENTS.md — Règles Inviolables pour Garde-fou

Ce dépôt implémente **Garde-fou** (`garde-fou`), un orchestrateur et analyseur statique polyglotte inspiré de l'architecture Hub-and-Spoke de Pandoc (`Tree-sitter` -> `UAST` -> Règles pures + Runners `SARIF` + Cache `BLAKE3`).
Le projet s'applique ses propres règles dès le premier commit (**dogfooding intégral**).

---

## 1. Architecture : Functional Core, Imperative Shell

Les frontières entre crates sont vérifiées par le compilateur et par les tests d'architecture (`tests/architecture.rs`) :

| Crate | Environnement | Responsabilité | E/S disque, env, process | Écriture console (`stdout`/`stderr`) |
| :--- | :--- | :--- | :--- | :--- |
| `gf-core` | `#![no_std]` + `alloc` | Types algébriques purs (`Span`, `RepoPath`, `Uast`, `Diagnostic`, `Config`, `Validation`) | Interdit | Interdit |
| `gf-rules` | `#![no_std]` + `alloc` | Moteur de règles statiques pures (dépend **uniquement** de `gf-core`) | Interdit | Interdit |
| `gf-parsers` | `std` pur (sans I/O) | Détecteurs de contexte et ponts `Tree-sitter` -> `Uast` | Interdit | Interdit |
| `gf-engine` | Shell impératif | Cache `BLAKE3`, scope Git, scaffolding `init`, runners `SARIF` externes | Autorisé | Interdit |
| `gf-cli` | Binaire `garde-fou` | Parsing CLI (`clap`) et reporters (`human`, `agent`, `sarif`) | Autorisé | Autorisé **uniquement** dans `src/output.rs` |

- `gf-core` et `gf-rules` DOIVENT conserver `#![no_std]` et `extern crate alloc;` en tête de `src/lib.rs` (`#[cfg(test)] extern crate std;` est toléré uniquement pour les tests unitaires).
- Aucune dépendance externe exigeant `std` n'est autorisée dans `gf-core` ni `gf-rules`.

---

## 2. Qualité Rust (Edition 2024) — Zéro Compromis

1. **Constructions strictement interdites** (en production comme dans les tests) :
   - `unsafe` (`unsafe_code = "forbid"`)
   - `.unwrap()`, `.expect()`, `panic!()`, `todo!()`, `unimplemented!()`, `dbg!()`
   - `println!()` / `eprintln!()` hors de `crates/gf-cli/src/output.rs`
2. **Gestion d'erreurs algébrique** :
   - Utiliser exclusivement `Result<T, E>` ou `gf_core::validation::Validation<T, E>`.
   - Dans les tests (`#[test]`), retourner `Result<(), E>` et propager avec `?`.
3. **Limites SRP (Single Responsibility Principle)** :
   - Maximum **250 lignes** par fichier `.rs` (vérifié par `tests/architecture.rs`).
   - Maximum **50 lignes** par fonction (`too-many-lines-threshold = 50`).
   - Complexité cognitive maximale de **10** (`cognitive-complexity-threshold = 10`).
   - Maximum **4 paramètres** par fonction (`too-many-arguments-threshold = 4`).
   - Maximum **2 booléens** par structure (`max-struct-bools = 2`).
4. **Documentation obligatoire** :
   - Chaque item public doit posséder une documentation Rustdoc (`///`), y compris une section `# Errors` pour toute fonction publique retournant `Result`.

---

## 3. Discipline de Développement & Anti-Triche

- **Interdiction absolue** d'utiliser `git commit --no-verify` ou de contourner `lefthook`.
- **Interdiction absolue** d'utiliser `#[allow(...)]` (`allow_attributes = "deny"`). Seul `#[expect(..., reason = "...")]` est toléré pour une frontière architecturale explicite et documentée (ex. `crates/gf-cli/src/output.rs`).
- **TDD & Couverture** : maintenir **>= 85 %** de couverture de lignes (`task cov`) et vérifier les mutations sur chaque diff (`task mutate`).
- Avant toute finalisation, exécuter `task check:full` et corriger 100 % des diagnostics.
