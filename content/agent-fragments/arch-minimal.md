# Minimal Governance Archetype (`archetype = "minimal"`)

## Proportional Zero-Language Baseline

- Zero programming languages are assumed until source files or manifests are added; `garde` still enforces syntax integrity (`GF000`), secret scanning (`GF040`), `garde.lock` sealing (`GF070`), and Conventional Commits (`GF077`).
- Add languages, frameworks, or infrastructure progressively (`garde init --lang ...` or `garde sync`) without unnecessary scaffolding.
