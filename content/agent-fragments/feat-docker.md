# Container & Docker Hardening (`GF030`–`GF034`, `GF080`–`GF082`)

## Multi-Stage `Dockerfile` & Compose Security

- Pin every `FROM` base image to an explicit version tag or `@sha256:...` digest (`GF030`), run the final runtime stage as an explicit non-root `USER` (`GF031`), validate `COPY --from=<stage>` aliases (`DK-DL3022`, `GF032`), and never invoke `sudo`, local non-archive `ADD`, or hardcoded secrets in `ENV`/`ARG` (`GF032`).
- Use JSON exec-array form `["app", "--flag"]` for `CMD` and `ENTRYPOINT` (`DK-DL3025`, `GF033`), configure `SHELL ["/bin/bash", "-o", "pipefail", "-c"]` (or `set -o pipefail`) before any `RUN` pipeline `|` (`DK-DL4006`, `GF033`), use absolute `WORKDIR` paths, clean package caches in the same `RUN` layer (`GF033`), and keep `.dockerignore` aligned (`GF034`).
- In Docker Compose (`compose.yaml` / `docker-compose*.yml`), forbid `privileged: true`, `host` namespaces, `user: root|0`, dangerous `cap_add` (`ALL`/`SYS_ADMIN`), `unconfined` profiles (`COMPOSE-PRIVILEGED-OR-HOST`), `docker.sock` mounts (`COMPOSE-DOCKER-SOCKET`), `:latest`/unpinned images (`COMPOSE-UNPINNED-IMAGE`), unquoted short `ports:` (`COMPOSE-UNQUOTED-PORTS`), plaintext secrets in `environment:` (`COMPOSE-SECRET-IN-ENV`), cyclic/broken `depends_on` graphs (`COMPOSE-BROKEN-DEPENDS-ON`), and orphan/undeclared `volumes`/`networks`/`secrets`/`configs` (`COMPOSE-ORPHAN-OR-UNDECLARED-RESOURCE`).
- Embedded shell snippets in `Dockerfile RUN`, Compose `sh -c`/`CMD-SHELL` (`$$` unescaped), and Hadolint/dclint configs are verified end-to-end (`GF070`, `GF080`–`GF082`).
