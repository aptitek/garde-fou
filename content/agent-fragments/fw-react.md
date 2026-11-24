# React 19 & JSX Component Architecture

## Rules of Hooks, Pure Rendering & Intrinsic Accessibility

- Keep presentation components small (`<= 50` effective lines per component), side-effect free during render, typed via pure `Props` contracts, and styled through semantic `Forme` classes (`GF060`).
- Obey React 19 Rules of Hooks (`REACT-RULES-OF-HOOKS`, `GF080`): call `use*` Hooks unconditionally at the top level of `PascalCase` components or custom `use*` Hooks before any early `return` (only React 19 `use(...)` / `React.use(...)` may run conditionally), and never pass `async` callbacks to `useEffect` / `useLayoutEffect` (`TS-NO-ASYNC-EFFECT`).
- Never declare nested `PascalCase` components inside another component (`REACT-NO-UNSTABLE-NESTED-COMPONENT`), never omit `key`, use shorthand `<>...</>`, or pass array `index` as `key` in `.map`/`flatMap`/`Array.from` (`REACT-JSX-KEY`), and forbid `dangerouslySetInnerHTML`, `javascript:` URLs, and unguarded `<a target="_blank">` (`REACT-NO-DANGEROUS-JSX`).
- Enforce intrinsic lowercase DOM accessibility (`GF066`): provide explicit `type` and label on `<button>`, meaningful `alt` (or `alt=""` with `aria-hidden="true"` / `role="presentation"`) on `<img>`, and keyboard/ARIA roles on interactive static elements without penalizing custom `PascalCase` components.
