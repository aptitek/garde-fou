# Rust Quality Standards (Edition 2024)

## Safety, Error Handling & Idioms

- Forbid `unsafe`, `.unwrap()`, `.expect()`, `panic!()`, `todo!()`, `unimplemented!()`, and `dbg!()` in both production modules and tests (`GF010`).
- Propagate recoverable failures algebraically via `Result<T, E>` (or applicative `Validation<T, E>`) using `?`, and document `# Errors` on every fallible public item.
- Prefer borrowed slices (`&str`, `&[T]`) over premature `.clone()`, deterministic `BTreeMap`/`BTreeSet` when output ordering matters, and small structs (`<= 2` booleans).
- Keep `cargo clippy --workspace --all-targets --all-features -- -D warnings` at zero warnings without `#[allow(...)]`.
