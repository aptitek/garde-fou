# Panda CSS Build-Time Styling Standards (`GF060`, `GF061`, `GF080`–`GF081`)

## Zero-Runtime Token Discipline

- Enable `strictTokens: true` and `strictPropertyValues: true` in `panda.config.*` (`PANDA-CONFIG-STRICT`, `GF081`), keep `styled-system/` gitignored and out of version control, and place theme definitions (`defineConfig`, `defineTokens`, `defineSemanticTokens`, `defineRecipe`, `defineSlotRecipe`) inside `DomainKind::Forme`.
- Consume build-time `styled-system/{css,patterns,recipes,jsx}` inside `DomainKind::Code` using semantic tokens exclusively: raw colors (`#hex`, `rgb`, `oklch`), raw units (`14px`), arbitrary escape hatches (`[14px]`, `[#002b36]`), `!important` (`!`), and primitive palette scales (`blue.500`, `slate.800`) are forbidden (`PANDA-ARBITRARY-OR-PRIMITIVE`, `GF060`, `GF061`).
