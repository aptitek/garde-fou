# Academic & Scientific Archetype (`archetype = "academic"`)

## Reproducible Notebooks & Course Authoring

- Jupyter notebooks (`.ipynb`), Quarto documents (`.qmd`), and Markdown notes are legitimate anywhere in the repository hierarchy (`TP*/`, `chapitres/`, `notebooks/`, or root).
- Keep notebook execution strictly ordered (`NB-OUT-OF-ORDER-EXECUTION`), free of committed error tracebacks (`NB-COMMITTED-ERROR-TRACEBACK`), free of leaked secrets in outputs (`NB-SECRET-IN-OUTPUT`), and modular (`NB-OVERSIZED-CELL`).
- Console `print(...)` and `display(...)` calls are allowed in notebook/Quarto cells while `def` functions remain subject to Python AST safety and complexity limits.
