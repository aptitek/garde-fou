# CSS & Design Tokens Standards (`GF061`, `GF081`–`GF082`)

## Specification-Grade Stylesheet & Token Purity

- Confine primitive colors (`#hex`, `rgb/hsl/oklch/color-mix`, and CSS Color 4 named colors) and raw physical/pixel dimensions (`px`, `pt`, `cm`, `mm`, `in`) exclusively to `tokens/` (`forme_token_globs`), consuming `var(--token)` in stylesheets (`CSS-RAW-COLOR`, `CSS-RAW-DIMENSION-OR-ZINDEX`).
- Never leak raw colors or dimensions inside `var(--x, #fff)` fallbacks (`CSS-FALLBACK-TOKEN-LEAK`), avoid `#id` selectors (`CSS-NO-ID-SELECTOR`), `!important` (`CSS-NO-IMPORTANT`), runtime `@import` outside token entrypoints (`CSS-NO-AT-IMPORT`), and `transition: all` (`CSS-NO-TRANSITION-ALL`).
- Never silence `stylelint` or `biome` via unjustified `/* stylelint-disable */` comments (`GF070`) or weakened `.stylelintrc` rules (`GF081`).
