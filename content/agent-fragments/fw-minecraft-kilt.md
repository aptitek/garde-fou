## Minecraft Modding (Kilt / Fabric / Quilt / NeoForge / Mixin) Guardrails (`GF080`–`GF081`)

### Side Safety, MixinExtras & Tick-Thread Hygiene

- Forbid `@Overwrite` Mixins (`MCMOD-MIXIN-OVERWRITE`); use `@Inject` or MixinExtras `@WrapOperation` / `@ModifyExpressionValue` for cross-mod compatibility.
- Keep `net.minecraft.client.*` imports strictly inside `src/client/` (`MCMOD-CLIENT-IN-COMMON`), never call `Thread.sleep()` on the tick thread (`MCMOD-SERVER-THREAD-BLOCK`), and keep `fabric.mod.json`, `neoforge.mods.toml`, and `*.mixins.json` valid with `compatibilityLevel` (`GF081`).
