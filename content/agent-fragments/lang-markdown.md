# CommonMark, GFM & MDX Standards (`GF063`, `GF081`–`GF082`)

## Editorial Purity, Heading Slugs & Link Integrity

- Place editorial `.md` and `.mdx` documents inside `content/`, `courses/`, or `docs/`, validate optional YAML frontmatter (`---`), and maintain a single `# H1` with sequential heading increments (`MD-HEADING-HIERARCHY`).
- Forbid raw HTML tags outside fenced/inline code (`MD-RAW-HTML`; standard `<https://...>` autolinks are valid), provide non-empty link URLs and image `alt` text in `[text](url)` order (`MD-EMPTY-OR-REVERSED-LINK`), and ensure every `[text][ref]`, `#anchor`, and relative `./doc.md#anchor` resolves to an existing GitHub-slugged heading (`MD-UNRESOLVED-REF-OR-ANCHOR`).
- Keep `.mdx` files free of `export` logic, hooks, `.css` imports, inline `style={{...}}`, and `dangerouslySetInnerHTML` (`MD-FORBIDDEN-MDX-LOGIC`), and never disable `MD001` or `MD033` in `.markdownlint*` / `rumdl.toml` (`GF081`).
