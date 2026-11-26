# Quarto Scientific Publishing (`_quarto.yml`, `.qmd`, `GF080`)

## Structural Integrity & Executable Cells

- Ensure every file referenced in `_quarto.yml` (`chapters`, `appendices`, `sidebar`, `navbar`, `bibliography`, `csl`, local `.scss`/`.css` themes) exists on disk (`QUARTO-MISSING-REFERENCED-FILE`).
- Balance all Pandoc fenced divs `:::` (`QUARTO-UNCLOSED-DIV`), keep `{{< include ... >}}` / `{{< embed ... >}}` targets valid (`QUARTO-BROKEN-SHORTCODE-TARGET`), match every `@fig-`/`@tbl-`/`@sec-`/`@eq-` cross-reference to a `#| label:` or `{#...}` anchor (`QUARTO-BROKEN-CROSSREF`), and keep `#| ...` cell options valid YAML.
- Partial `_*.qmd` files are exempt from requiring a top-level `# H1` heading; executable cells (`{python}`, `{bash}`, `{sh}`, `{sql}`) are bridged into their native AST analyzers.
