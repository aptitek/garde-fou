# Docker Compose Stack Hardening (`GF030`–`GF034`, `GF080`)

## Service Isolation & Reproducible Images

- Pin every `image:` to an explicit version tag or digest (never `:latest`), avoid `privileged: true` and host namespace sharing (`COMPOSE-*`), and keep secrets out of `environment:` blocks (`GF040`).
- Maintain a `.env.example` template with placeholder values and keep real `.env` files excluded in `.gitignore` and `.dockerignore`.
