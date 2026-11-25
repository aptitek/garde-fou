# Design Tokens, Atomic Design & UI Accessibility

## Visual & Interactive Architecture

- Enforce strict **Atomic Design** import direction: `atoms -> molecules -> organisms -> templates -> pages` (`GF072`); `atoms` and `molecules` must never import `api`, `services`, or `store` layers directly.
- Define colors (`#hex`, `rgb/hsl/oklch/color-mix`, CSS Color 4 named colors), physical/pixel dimensions (`px`, `pt`, `cm`, `mm`, `in`), and `z-index` scales solely in `tokens/` (`forme_token_globs`), consuming `var(--token)` without raw `var(--x, #fff)` fallbacks (`CSS-FALLBACK-TOKEN-LEAK`), `#id` selectors (`CSS-NO-ID-SELECTOR`), `!important` (`CSS-NO-IMPORTANT`), runtime `@import` (`CSS-NO-AT-IMPORT`), or `transition: all` (`CSS-NO-TRANSITION-ALL`, `GF060`, `GF061`).
- Sanitize `.svg` assets after XML entity decoding against DTD/XXE subsets (`XML-FORBID-DTD-XXE`), `<script>`/`<foreignObject>` (`SVG-DANGEROUS-ELEMENT`), `on*=` and SMIL `<set>`/`<animate*>` XSS (`SVG-EVENT-OR-SMIL-XSS`), and unsafe `href`/`<use>`/`<style>` rules (`SVG-UNSAFE-HREF-OR-USE`, `GF064`).
- Guarantee static `a11y` (`GF066`): use `<button type="button">` instead of `<div onClick>`, supply `alt` on every `<img>`, and ensure icon-only buttons carry `aria-label`.
