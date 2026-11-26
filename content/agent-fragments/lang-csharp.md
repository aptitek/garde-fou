# C# & .NET Standards (`GF080`–`GF082`)

## Primary Constructors, Async Safety & EF Core

- Never double-capture a C# 12 primary constructor parameter in both a field/property initializer and a member body (`CS-PRIMARY-CTOR-DOUBLE-CAPTURE`, `CS9124`), and never reassign primary constructor parameters.
- Forbid `async void` methods in production code (`CS-NO-ASYNC-VOID`) and blocking `.Result`, `.Wait()`, or `.GetAwaiter().GetResult()` calls (`CS-SYNC-OVER-ASYNC`).
- Always `await` `*Async(...)` calls before `using` / `await using` (`CS-DISPOSE-AND-CANCELLATION-SAFETY`, `MA0129`) and forward `CancellationToken` to inner awaited calls (`MA0040`).
- Rethrow caught exceptions with bare `throw;` instead of `throw ex;` (`CS-RETHROW-DESTROYS-STACK`, `CA2200`), avoid empty `catch`, `GC.Collect()`, `Thread.Sleep` in `async`, and `Console.Write*` in libraries.
- Use EF Core `FormattableString` (`FromSql($"...")` / `ExecuteSql($"...")`) rather than interpolated `FromSqlRaw($"...")` or `ExecuteSqlRaw($"...")`, and keep `<Nullable>enable</Nullable>` and .NET analyzers active in `*.csproj` / `Directory.Build.props` (`GF081`).
