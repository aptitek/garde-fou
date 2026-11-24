# Git Hooks & Commit Hygiene (`GF077`)

## Local Gate Discipline

- Keep `lefthook.yml` hooks active (`pre-commit` running `garde check --staged --format agent`, `commit-msg` running `garde commit-msg`, and `pre-push` running `garde check --no-cache`).
- Format commit headers strictly as `<type>(<scope>): <description>` within `max_header_len` and never bypass hooks.
