# Infrastructure & Stacks Archetype (`archetype = "infra"`)

## Container & Operations Hardening

- Organize declarative stacks under the repository root, `services/`, `stacks/`, or `deploy/` without requiring an application `src/` tree.
- Enforce multi-stage non-root `Dockerfile` hardening (`GF030`–`GF034`), pinned Docker Compose services (`COMPOSE-*`), YAML 1.2 canonical booleans (`YAML-*`), and secret-free manifests (`GF040`).
