# Astro Content & Islands Architecture (`GF021`, `GF063`, `GF080`)

## Zero-JS-First Routing & Selective Hydration

- Treat `src/pages/**/*.astro`, `src/layouts/**/*.astro`, `src/middleware.ts`, and `src/content.config.ts` as thin routing and schema entrypoints delegating business logic to `src/` modules and editorial copy to `Contenus` (`content/`).
- Keep frontmatter strictly server-only (`ASTRO-FRONTMATTER-CLIENT-API`), forbid `set:html` (`ASTRO-NO-SET-HTML`), pass only serializable data props to `client:*` islands (`ASTRO-NON-SERIALIZABLE-ISLAND-PROP`), and prefer `client:visible` / `client:idle` over `client:load` / `<script is:inline>` (`ASTRO-CLIENT-LOAD-BUDGET`).
