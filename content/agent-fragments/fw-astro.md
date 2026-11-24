# Astro Content & Islands Architecture

## Zero-JS-First Routing

- Treat `src/pages/**` as thin routing entrypoints delegating business logic to `src/` modules and editorial copy to `Contenus` (`content/`).
- Ship static HTML by default, hydrating interactive islands selectively while keeping styles in `Forme` and static media in `Asset`.
