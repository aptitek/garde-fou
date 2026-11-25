# Editorial `Contenus` Markdown Purity (`GF063`)

## Content Authoring Rules

- Place all editorial `.md` and `.mdx` documents inside `content/`, `courses/`, or `docs/` (`contenus_globs`), and keep optional YAML frontmatter (`---`) 100% valid YAML 1.2 without duplicate keys.
- Use a single `# H1` per document with sequential heading levels (`H1 -> H2 -> H3`, `MD-HEADING-HIERARCHY`), provide non-empty link destinations and image `alt` text in `[text](url)` order (`MD-EMPTY-OR-REVERSED-LINK`), and ensure every `[text][ref]`, `#anchor`, and `./doc.md#anchor` resolves to an existing GitHub-slugged heading (`MD-UNRESOLVED-REF-OR-ANCHOR`).
- Never embed raw HTML tags outside code spans/fences (`MD-RAW-HTML`; standard `<https://...>` autolinks are valid), and keep `.mdx` free of `export` logic, hooks, `.css` imports, `style={{...}}`, and `dangerouslySetInnerHTML` (`MD-FORBIDDEN-MDX-LOGIC`).
