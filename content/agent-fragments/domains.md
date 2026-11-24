# Six-Domain Segregation Law (`GF060`–`GF066`)

## Strict Domain Responsibilities

- **`Code`**: pure business logic, state transitions, and semantic UI structure. Zero CSS-in-JS, inline `style=`, raw `#hex` colors, inline `<svg>`, `data:` URIs, or imports from `Scripts`.
- **`Scripts`**: standalone developer and CI automation (`scripts/`, `tools/`), subject to file/secret limits and mandatory automated test coverage (`GF065`).
- **`Forme`**: stylesheets and themes (`styles/`, `tokens/`). Primitive `#hex`/`px`/`oklch` values live exclusively inside token files; component stylesheets consume `var(--token)` without `!important` (`GF061`).
- **`Texte`**: pure plain-text `i18n` dictionaries (`locales/`), kept 100% key-symmetric across locales with zero HTML markup (`GF062`).
- **`Contenus`**: editorial Markdown/MDX (`content/`, `courses/`, `docs/`) as Single Source of Truth, free of raw HTML, hooks, or stylesheet imports (`GF063`).
- **`Asset`**: confined static media and sanitized SVGs (`assets/`, `public/`, `static/`) within `max_asset_bytes`, with zero broken links or unreferenced orphans (`GF064`).
