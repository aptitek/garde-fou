# TypeScript & JavaScript Standards

## Strict Typing & Module Hygiene

- Maintain `"strict": true`, `"noUncheckedIndexedAccess": true`, and `"exactOptionalPropertyTypes": true` in `tsconfig.json`; never escape the type system with `any` or `@ts-ignore` (`GF070`).
- Use explicit named imports and exports; avoid barrel `export * from "..."` re-exports (`GF074`) and circular module imports (`GF071`).
- Model data shapes with pure `type` or property-only `interface` DTOs; introduce behavioral interfaces with methods only when at least two concrete implementations exist (`GF075`).
