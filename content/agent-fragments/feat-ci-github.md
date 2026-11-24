# GitHub Actions CI/CD Gates (`GF080`–`GF082`)

## Continuous Verification & Workflow Hardening

- Run `garde check --no-cache` alongside test and supply-chain gates on every pull request and `main` push with minimal `permissions: { contents: read }` at workflow root, elevating write scopes only on the specific job that needs them (`GHA-PERMISSIONS-AND-PINNING`).
- Pin every external `uses: owner/repo@<tag-or-40-char-sha>` (never `@main`, `@master`, `@HEAD`, or `@latest`, `GHA-PERMISSIONS-AND-PINNING`), never check out untrusted PR head refs under `pull_request_target` (`GHA-PWN-REQUEST`), and bind untrusted `${{ github.event... }}` values to `env:` variables instead of interpolating them directly into `run:` or `actions/github-script` (`GHA-TEMPLATE-INJECTION`).
- Embedded `run:` blocks are bridged into `Bash`, `PosixSh`, or `PowerShell` CST analyzers according to `step.shell`, `defaults.run.shell`, and `runs-on` (`GF080`), alongside `actionlint` and `zizmor` (`GF081`–`GF082`).
