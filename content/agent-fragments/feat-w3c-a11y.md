# W3C HTML5, WAI-ARIA 1.2 & WCAG 2.2 AA/AAA Standards (`GF066`, `GF080`)

## Structural Validity & Assistive Technology Compliance

- Enforce WHATWG/W3C HTML5 content models (`W3C-INTERACTIVE-NESTING`, `W3C-VOID-ELEMENT-CHILDREN`, `W3C-CONTENT-MODEL`, `W3C-LANDMARK-AND-ID-UNIQUENESS`) across `.html`, `.astro`, `.tsx`, and `.jsx`.
- Use only valid WAI-ARIA 1.2 roles and `aria-*` attributes with well-typed values and required role properties (`role="checkbox" | "switch"` requires `aria-checked`, `role="slider"` requires `aria-valuenow`/`aria-valuemin`/`aria-valuemax`) without redundant roles (`<button role="button">`, `A11Y-ARIA-INVALID`).
- Never place `aria-hidden="true"` on focusable controls (`A11Y-HIDDEN-FOCUSABLE`), avoid positive `tabIndex`, `autoFocus`, or uncompensated `outline: none` (`A11Y-TABINDEX-AND-FOCUS-RING`), label every form control, caption `<video>`/`<audio>`, and guard spatial CSS animations with `@media (prefers-reduced-motion: reduce)` (`A11Y-MEDIA-FORM-MOTION`).
