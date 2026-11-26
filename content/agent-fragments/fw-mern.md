## MERN TypeScript Architecture (`Express 5` + `MongoDB` / `Mongoose`, `GF072`/`GF080`)

### Runtime Validation, NoSQL Injection Defense & Strict Monorepo Isolation

- Validate every `req.body`, `req.query`, and `req.params` payload through a runtime schema parser (`Zod`, `Valibot`, `ArkType`, `TypeBox`) instead of TypeScript `as` casts (`EXPRESS-UNVALIDATED-REQUEST-CAST`).
- Wrap untrusted MongoDB filter values in `{ $eq: value }`, forbid `$where` / `$function` / `$accumulator` (`MONGO-NOSQL-INJECTION`), avoid unbounded `deleteMany({})` / `find()` without `.limit(...)` / loop N+1 queries (`MONGO-UNBOUNDED-QUERY-OR-MUTATION`), and index every Mongoose `ref:` field without `Schema.Types.Mixed` (`MONGO-SCHEMA-STRICTNESS`).
- Restrict `cors({ origin, credentials })` to explicit allowlists, never send `err.stack` to clients (`EXPRESS-INSECURE-CORS-OR-ERROR-LEAK`), separate exported `app.ts` from `server.ts` (`EXPRESS-LISTEN-IN-APP-MODULE`), and never import `express`, `mongoose`, `mongodb`, or `node:fs` from frontend `apps/web/**` / `.tsx` modules (`GF072`).
