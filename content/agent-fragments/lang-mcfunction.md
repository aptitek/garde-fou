## Minecraft 1.21+ `.mcfunction` Data Pack Standards (`GF080`–`GF082`)

### Brigadier 1.21 Syntax, Macros & TPS Safety

- Omit leading `/` on commands (`MCFUNC-LEADING-SLASH`), keep `$` macro lines paired with `$(param)` placeholders (`MCFUNC-MACRO-SYNTAX`), and constrain `@e[...]` selectors with `type=`/`tag=`/`distance=`/`limit=` (`MCFUNC-UNBOUNDED-SELECTOR`).
- Use singular `data/<namespace>/function/` folders for Minecraft 1.21+ (`pack_format >= 48`, `MCFUNC-121-PLURAL-FOLDER`), forbid admin commands (`stop`, `op`, `deop`, `ban`, `MCFUNC-UNSAFE-ADMIN-CMD`), validate `pack.mcmeta` (`GF081`), and remediate `Spyglass` findings (`GF082`).
