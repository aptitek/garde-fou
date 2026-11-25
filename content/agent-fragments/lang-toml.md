# TOML & Manifest Reproducibility Standards (`GF080`–`GF082`)

## Table Clarity, Immutable Pins & Workspace Inheritance

- Group dotted keys and subtables contiguously without out-of-order table reopenings, and expand inline tables exceeding `4` keys into standard `[table]` sections (`TOML-CONFUSING-TABLE-REOPEN`).
- Never use wildcard `"*"` versions, `"latest"` tool pins in `mise.toml`, or `git` dependencies without an immutable `rev = "<sha>"` or release `tag` in `Cargo.toml` and `pyproject.toml` (`TOML-UNPINNED-GIT-OR-WILDCARD`).
- Inherit shared Cargo workspace dependencies via `<crate>.workspace = true` in member manifests instead of duplicating version strings (`TOML-WORKSPACE-BYPASS`), and never silence `taplo` via blanket `exclude = ["**/*.toml"]` (`GF081`).
