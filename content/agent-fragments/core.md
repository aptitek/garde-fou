# Core Engineering Guardrails (Zero Technical Debt)

## Architectural & Code Discipline

- Respect **Functional Core, Imperative Shell**: isolate pure deterministic domain transformations from filesystem, network, and process side effects.
- Enforce **SRP** (single responsibility per file and callable), **DRY** (extract genuine structural duplicates instead of renaming variables cosmetically), **KISS** (flat control flow, guard clauses, no nested ternaries or Law of Demeter property trains), and **YAGNI** (no speculative barrel `export *` or single-implementation interfaces).
- Never silence or bypass static analysis: inline suppression comments, unjustified lint attributes, `TODO`/`FIXME` placeholders in production code, and `--no-verify` Git commits are strictly forbidden (`GF070`, `GF074`).
- Every automated test must assert real observable behavior or algebraic invariants (`GF076`); assertion-free smoke stubs and constant tautologies are blocked.

## Mandatory Verification Loop

1. Run `garde check --staged --format agent` before staging or committing changes and resolve 100% of reported violations at their root cause.
2. Never weaken thresholds in `garde-fou.toml` without explicit human authorization; `garde.lock` (`GF070`) and `AGENTS.md` (`garde sync`) seal project governance.
3. Write Conventional Commit headers (`<type>(<scope>): <imperative summary>`, `GF077`) that state what changed and why.
