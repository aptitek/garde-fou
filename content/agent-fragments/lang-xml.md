# XML, SVG Security & Manifest Standards (`GF064`, `GF080`–`GF082`)

## XXE Prevention, SVG Sanitization & Dependency Pinning

- Forbid `<!ENTITY`, `SYSTEM`/`PUBLIC` external DTDs, and internal `<!DOCTYPE ... [...]>` subsets across XML and SVG documents (`XML-FORBID-DTD-XXE`).
- Sanitize `.svg` assets after XML entity decoding against `<script>`, `<foreignObject>`, `<iframe>`, `<embed>`, `<object>`, `<handler>` (`SVG-DANGEROUS-ELEMENT`), `on*=` attributes and SMIL `<set>`/`<animate*>` mutations (`SVG-EVENT-OR-SMIL-XSS`), `javascript:`/`data:`/`http(s):` `<use>` references, and unsafe `<style>` `@import` rules (`SVG-UNSAFE-HREF-OR-USE`).
- Pin exact deterministic versions without `*`, `LATEST`, or `RELEASE` and use `https://` package repositories in `.csproj`, `Directory.Packages.props`, and `pom.xml` (`XML-UNPINNED-DEPENDENCY`).
