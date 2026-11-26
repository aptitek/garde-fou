# Material Design 3 Expressive (`M3E`) Guardrails (`GF061`, `GF066`, `GF080`)

## Tonal Pairing, Spring Physics & 48dp Targets

- Pair every MD3 container surface with its matching `on*` ink token (`primary` <-> `onPrimary`, `primaryContainer` <-> `onPrimaryContainer`, `secondaryContainer` <-> `onSecondaryContainer`, `errorContainer` <-> `onErrorContainer`, `surfaceContainer*` <-> `onSurface` / `onSurfaceVariant`, `M3E-TONAL-PAIRING`).
- Respect physical spring separation: apply **`Spatial`** springs (`*Spatial*`, where overshoot is permitted) solely to geometric properties (`transform`, `width`, `height`, `top`, `left`, `borderRadius`) and **`Effects`** springs (`*Effects*`, zero overshoot) to `opacity`, `color`, `backgroundColor`/`bg`, `borderColor`, `fill`, and `stroke` (`M3E-SPRING-PHYSICS-MISMATCH`).
- Consume exclusively MD3 elevation (`elevation.level0` .. `level5`), shape (`none` .. `extraExtraLarge`, `full`), and typography (`display`, `headline`, `title`, `body`, `label` + `*Emphasized`) tokens (`M3E-RAW-ELEVATION-SHAPE-TYPO`), and preserve `>= 48dp` (`48px`, `>= 24px` WCAG AA minimum) interactive targets and state layers (`M3E-TOUCH-TARGET-AND-STATE-LAYER`).
