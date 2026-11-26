## Dart 3.6+ Quality Standards (`GF080`–`GF082`)

### Sound Null-Safety, Strict Typing & Modern Web

- Avoid explicit `dynamic` (`DART-EXPLICIT-DYNAMIC`), `print()` in production (`DART-AVOID-PRINT`), empty `catch {}` blocks (`DART-EMPTY-CATCH`), deprecated `dart:html` (`DART-DEPRECATED-HTML`), and `// ignore_for_file:` suppressions (`DART-IGNORE-FOR-FILE`).
- Enforce `strict-casts: true` and `strict-raw-types: true` in `analysis_options.yaml` and pin `environment.sdk` (`^3.6.0`) without `any` dependencies in `pubspec.yaml` (`GF081`), resolving 100% of `dart analyze` diagnostics (`GF082`).
