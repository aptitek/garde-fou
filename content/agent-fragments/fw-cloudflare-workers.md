# Cloudflare Workers, D1, R2 & Wrangler Edge Standards (`GF040`, `GF080`–`GF081`)

## V8 Isolate Purity, Binding Coherence & Edge Storage

- Declare a valid `compatibility_date` (`YYYY-MM-DD`) in `wrangler.toml` / `wrangler.jsonc`, never store plaintext secrets in `[vars]` (`SECRET`, `PASSWORD`, `TOKEN`, `API_KEY`, `PRIVATE_KEY`), avoid unguarded `remote = true`, and add `"nodejs_compat"` to `compatibility_flags` whenever `node:*` modules are imported (`CF-WRANGLER-CONFIG`, `GF081`).
- Keep every `env.<BINDING>` reference synchronized with `wrangler.*` bindings (`d1_databases`, `r2_buckets`, `kv_namespaces`, `queues`, `ai`, `durable_objects`, `services`, `vars`) and avoid unused `d1`/`r2` declarations (`CF-BINDING-COHERENCE`).
- Never call `fetch`, `crypto.getRandomValues`, `crypto.randomUUID`, `crypto.subtle`, `Date.now`, `new Date`, `setTimeout`, or `setInterval` at module top-level outside request handlers, and never mutate top-level `let`/`var` state across requests (`CF-GLOBAL-SCOPE-IMPURE`).
- Forbid incompatible Node APIs (`fs`, `child_process`, `cluster`, `dgram`, `v8`, `CF-FORBIDDEN-NODE-API`), batch D1 writes via `await env.DB.batch([...])` instead of awaiting `env.DB.prepare(...).run()` inside loops while keeping bound `?` parameters `<= 100` (`CF-D1-N-PLUS-ONE-OR-LIMIT`), and sanitize R2 keys against `..` traversal without mutating read-only `request.cf` (`CF-R2-STREAM-AND-KEY-SAFETY`).
