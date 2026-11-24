# YAML 1.2, Docker Compose & CI Standards (`GF080`–`GF082`)

## Canonical YAML 1.2 & Anchor Safety

- Eliminate duplicate mapping keys and conflicting `<<: [*a, *b]` merge keys (`YAML-DUPLICATE-KEY`); explicit keys in the child mapping may cleanly override inherited `<<: *base` defaults.
- Use lowercase `true` and `false` exclusively for booleans (`YAML-TRUTHY`, exempting root `on:` in GitHub Actions workflows), quote file-mode strings (`"0755"`) or write `0o755` (`YAML-IMPLICIT-OCTAL`), quote sexagesimal-looking scalars (`"22:22"`, `YAML-SEXAGESIMAL`), and bound `&anchor` / `*alias` depth (`<= 16`, `<= 2048` nodes, `YAML-ANCHOR-BOMB`).
- Never silence YAML, Compose, or workflow analyzers via unjustified `# yamllint disable`, `# dclint disable`, `# actionlint ignore`, or `# zizmor: ignore[...]` comments (`GF070`), nor weaken `.yamllint*`, `.dclintrc*`, or `zizmor.yml` (`GF081`).
