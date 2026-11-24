# Container & Docker Hardening (`GF030`–`GF034`, `GF080`–`GF082`)

## Multi-Stage Security & `RUN` AST Bridge

- Pin every `FROM` base image to an explicit version tag or `@sha256:...` digest (`GF030`), run the final runtime stage as an explicit non-root `USER` (`GF031`), validate `COPY --from=<stage>` aliases (`DK-DL3022`, `GF032`), and never invoke `sudo`, local non-archive `ADD`, or hardcoded secrets in `ENV`/`ARG` (`GF032`).
- Use JSON exec-array form `["app", "--flag"]` for `CMD` and `ENTRYPOINT` (`DK-DL3025`, `GF033`), configure `SHELL ["/bin/bash", "-o", "pipefail", "-c"]` (or `set -o pipefail`) before any `RUN` pipeline `|` (`DK-DL4006`, `GF033`), use absolute `WORKDIR` paths, clean package caches in the same `RUN` layer (`GF033`), and keep `.dockerignore` aligned (`GF034`).
- Shell-form and heredoc `RUN` layers are bridged into the active stage `SHELL` AST analyzer (`PosixSh`, `Bash`, or `PowerShell`, `GF080`); never bypass Hadolint via unjustified `# hadolint ignore=...` (`GF070`) or weakened `.hadolint.yaml` rules (`GF081`), and remediate `[hadolint:...]` findings (`GF082`).
