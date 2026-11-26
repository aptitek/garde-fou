## Modern Node.js Runtime Standards (`GF080`–`GF081`)

### Native ESM, `node:` Protocol & Non-Blocking Event Loop

- Use `pnpm` exclusively (`"packageManager": "pnpm@<exact-version>"`, `"type": "module"`, `pnpm install --frozen-lockfile`, `pnpm exec`, `pnpm dlx`); never commit `package-lock.json` or `yarn.lock` (`TOOLCHAIN-PNPM-ONLY`).
- Prefix every Node.js built-in module import with `node:` (`import { readFile } from "node:fs/promises"`, `NODE-PROTOCOL-IMPORT`).
- Never call blocking `*Sync` APIs (`readFileSync`, `execSync`, `pbkdf2Sync`, etc.) inside `async` functions or HTTP handlers (`NODE-NO-SYNC-IO-IN-HANDLER`), and replace `new Buffer(...)`, `require(...)`, and library `process.exit(...)` with modern ESM equivalents (`NODE-MODERN-ESM-HYGIENE`).
