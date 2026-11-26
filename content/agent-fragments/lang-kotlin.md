# Kotlin Standards (`GF080`–`GF082`)

## Null Safety, Coroutines & Domain Purity

- Forbid the non-null assertion operator `!!` in production code (`KT-NO-NOT-NULL-ASSERTION`); prefer safe calls (`?.`), Elvis (`?:`), `?.let`, or `requireNotNull(x) { "reason" }`.
- Never swallow `CancellationException` inside `suspend` functions: avoid `runCatching { ... }` in coroutines and explicitly rethrow `CancellationException` in broad `catch (e: Exception)` / `catch (e: Throwable)` handlers (`KT-SUSPEND-RUNCATCHING-CANCELLATION`).
- Enforce structured concurrency (`KT-COROUTINE-STRUCTURED-SAFETY`): forbid `GlobalScope.launch`/`async`, replace `Thread.sleep` with `delay(...)` in `suspend` blocks, keep `runBlocking` out of library code, and return cold `Flow<T>` without `suspend`.
- Eliminate empty `catch` blocks, `: Any` / `: Any?` domain types, debug `println`, and `*spread` copies inside loops (`KT-SWALLOWED-EXCEPTION-AND-ANY`), use parameterized SQL statements, and keep `detekt.yml` at `maxIssues: 0` with pinned Gradle versions (`GF081`).
- Use Gradle Kotlin DSL (`build.gradle.kts`, `settings.gradle.kts`) and Version Catalogs (`gradle/libs.versions.toml`) exclusively; never use Groovy `build.gradle`, dynamic `+`/`SNAPSHOT` versions, or plaintext `http://` wrapper URLs (`TOOLCHAIN-GRADLE-KTS-ONLY`, `GRADLE-USE-VERSION-CATALOG`).
