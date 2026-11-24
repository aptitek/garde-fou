# Container & Docker Hardening (`GF030`–`GF034`)

## Multi-Stage Security Rules

- Pin every `FROM` base image to an explicit version tag or `@sha256:...` digest; never use `:latest` or unpinned images (`GF030`).
- Run the final runtime stage as an explicit non-root `USER` (`GF031`) and never invoke `sudo`, local non-archive `ADD`, or hardcoded secrets in `ENV`/`ARG` (`GF032`).
- Use absolute `WORKDIR` paths, clean package-manager caches in the same `RUN` layer (`GF033`), and keep `.dockerignore` aligned with `.git` and build caches (`GF034`).
