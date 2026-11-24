# Internationalization & `Texte` Purity (`GF062`)

## Localization Rules

- Route all user-visible JSX copy and translatable attributes (`placeholder`, `title`, `alt`, `aria-label`) through `t("namespace.key")`.
- Keep every locale file in `locales/` 100% key-symmetric with plain text values only (zero HTML tags, `style=`, or `data:` payloads), and prune unused translation keys.
