# Design Tokens, Atomic Design & UI Accessibility

## Visual & Interactive Architecture

- Enforce strict **Atomic Design** import direction: `atoms -> molecules -> organisms -> templates -> pages` (`GF072`); `atoms` and `molecules` must never import `api`, `services`, or `store` layers directly.
- Define colors, spacing, and typography primitives solely in `tokens/` (`forme_token_globs`) and consume `var(--...)` in `styles/` (`GF060`, `GF061`).
- Guarantee static `a11y` (`GF066`): use `<button type="button">` instead of `<div onClick>`, supply `alt` on every `<img>`, and ensure icon-only buttons carry `aria-label`.
