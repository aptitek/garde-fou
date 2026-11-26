# Scripts Automation Archetype (`archetype = "scripts"`)

## Practical Root & CLI Script Standards

- Standalone `.sh`, `.bash`, `.ps1`, and PEP 723 `.py` scripts are legitimate at the repository root, under `bin/`, or in `scripts/`.
- Enforce strict mode (`set -eu`/`set -euo pipefail`, `Set-StrictMode -Version Latest`), quoted expansions (`SH-SC2086`), safe HTTPS downloads (`SH-CURL-PIPE-UNSAFE`), and valid pinned PEP 723 `# /// script` TOML dependencies (`PY-PEP723-INVALID`, `PY-PEP723-UNPINNED`).
- CLI `print(...)` output is allowed in standalone scripts; `GF065` script test coverage activates automatically once a `tests/` directory or `tdd` feature is present.
