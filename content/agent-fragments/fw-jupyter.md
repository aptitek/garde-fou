# Jupyter Notebook Reproducibility (`.ipynb`, `GF080`)

## nbformat v4 Hygiene & Multi-AST Cell Analysis

- Maintain valid `nbformat >= 4` JSON notebooks with strictly increasing `execution_count` values (`NB-OUT-OF-ORDER-EXECUTION`), zero committed `error` tracebacks (`NB-COMMITTED-ERROR-TRACEBACK`), and zero secrets in cell `outputs[]` (`NB-SECRET-IN-OUTPUT`).
- Keep code cells focused (`<= max_function_lines`, `NB-OVERSIZED-CELL`); IPython line magics (`%...`) are neutralized safely while `%%bash`/`%%sh` and `%%sql` cells are bridged into Shell and SQL AST analyzers.
