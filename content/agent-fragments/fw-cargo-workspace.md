# Cargo Workspace Architecture

## Crate DAG & Visibility Rules

- Maintain a strict acyclic dependency graph across crates and internal modules (`GF071`) enforced by `[[boundaries]]` (`GF072`).
- Share dependency versions and strict `[workspace.lints]` at the workspace root `Cargo.toml`, and narrow internal helpers to `pub(crate)` or module-private visibility (`GF021`).
