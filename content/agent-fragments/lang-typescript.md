# TypeScript & JavaScript Standards

## Strict Typing, Safety & Module Hygiene

- Enforce `"strict": true`, `"noUncheckedIndexedAccess": true`, and `"exactOptionalPropertyTypes": true` in `tsconfig.json` (or inherit via `"extends"` without `"strict": false` / `"noImplicitAny": false` / `"strictNullChecks": false`, `GF081`); keep `"linter.enabled": true` in `biome.json`.
- Forbid explicit `any` (`TS-NO-EXPLICIT-ANY`), double or angle-bracket assertions `as unknown as T` / `<T>expr` (`TS-NO-DOUBLE-ASSERTION`), optional-chain non-null assertions `a?.b!` / `x!!` (`TS-NO-NON-NULL-OPTIONAL-CHAIN`), `new Promise(async ...)` (`TS-NO-ASYNC-PROMISE-EXECUTOR`), and leftover `console.{log,debug,info,trace}` / `debugger;` in `Code` (`TS-NO-CONSOLE-DEBUG`, `GF080`).
- Never use `@ts-ignore` or `@ts-nocheck`; `@ts-expect-error` requires `:` or `--` with a `>= 10`-character justification (`GF070`).
- Separate `import type` from runtime imports: runtime imports must form a strict acyclic DAG (`GF071`), while architectural `[[boundaries]]` and Atomic Design (`GF072`) govern both value and `import type` edges across ESM `.js` -> `.ts`/`.tsx` and `tsconfig` `paths` aliases.
- Model data shapes with pure `type` or property-only `interface` DTOs; introduce behavioral interfaces with methods only when at least two concrete implementations exist (`GF075`), and remediate `[oxlint:...]`, `[biome:...]`, and `[tsc:...]` diagnostics (`GF082`) at their source.
