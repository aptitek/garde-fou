# Astro Component & Template Standards (`GF063`, `GF066`, `GF080`)

## Server Frontmatter, Hydrated Islands & Template Purity

- Keep the server frontmatter fence (`--- ... ---`) strictly server-safe: never reference browser globals (`window`, `document`, `localStorage`, `sessionStorage`, `navigator`) or React Hooks (`useState`, `useEffect`) inside `.astro` frontmatter (`ASTRO-FRONTMATTER-CLIENT-API`).
- Never use `set:html={...}` unescaped HTML injection (`ASTRO-NO-SET-HTML`), never pass non-serializable functions or class instances (`onClick={() => ...}`, `new Class()`) to hydrated islands carrying `client:load`, `client:idle`, `client:visible`, or `client:only` (`ASTRO-NON-SERIALIZABLE-ISLAND-PROP`), and prefer `client:visible` or `client:idle` over eager `client:load` / `<script is:inline>` (`ASTRO-CLIENT-LOAD-BUDGET`).
- Ensure `<style>` blocks in `.astro` consume semantic Design Tokens (`GF061`) and template markup satisfies W3C HTML5 content models and WCAG 2.2 AA accessibility (`GF066`).
