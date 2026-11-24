# POSIX Shell & Bash Standards (`GF080`–`GF082`)

## Strict Mode, Quoting & Portability

- Enable `set -eu` in POSIX `#!/bin/sh` scripts and `set -euo pipefail` (or `set -eu` plus `set -o pipefail`) in `#!/bin/bash` scripts (`SH-STRICT-MODE`, `GF080`).
- Double-quote `$var`, `${var}`, and `$(cmd)` expansions in command arguments and `for x in ...` lists (`SH-SC2086`, `SH-SC2046`); scalar assignments `a=$b`, `case $a in`, `[[ ... ]]`, and `$(( ... ))` are word-split-safe.
- Separate scope declarations (`local`, `export`, `readonly`) from `$(cmd)` assignments so exit codes are never masked (`SH-SC2155`), guard recursive `rm` expansions with `${var:?}` (`SH-SC2115`), check `cd` failures (`SH-SC2164`), avoid `eval` (`SH-NO-EVAL`), and keep `#!/bin/sh` scripts 100% free of bashisms such as `[[ ... ]]`, `(( ... ))`, arrays, `function`, `source`, `<<<`, `${var//a/b}`, `local`, and unguarded `set -o pipefail` (`SH-SC3000`).
- Never use `# shellcheck disable=all` or `.shellcheckrc` global range disables (`GF070`, `GF081`); single-code `# shellcheck disable=SCxxxx -- <reason>` requires a `>= 10`-character justification, and `[shellcheck:...]` findings (`GF082`) must be fixed at their root cause.
