# Go Standards (`GF080`–`GF082`)

## Error Handling, Context & Concurrency

- Return untyped `nil` explicitly instead of a typed pointer variable when returning `error` or an interface (`GO-TYPED-NIL-INTERFACE`, `SA4023`).
- Always check `if err != nil { return err }` before registering `defer res.Close()` (`GO-DEFER-BEFORE-ERR-CHECK`, `SA5001`), and never place `defer` inside a `for` loop (`GO-DEFER-IN-LOOP`).
- Handle returned errors explicitly instead of discarding them with `_` or ignoring critical I/O writes (`GO-ERRCHECK-BLANK-OR-IGNORED`).
- Pass `ctx context.Context` as the first parameter, never store it inside a `struct`, and propagate `ctx` instead of calling `context.Background()` or `context.TODO()` (`GO-CONTEXT-MISUSE`).
- Return typed `error` values instead of calling `panic(...)` in library packages, remove debug `fmt.Print*`, use parameterized SQL queries (`$1` / `?`) instead of `fmt.Sprintf`, and keep `go.mod` free of local relative `replace` directives (`GF081`).
