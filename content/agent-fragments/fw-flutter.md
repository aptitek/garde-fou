## Flutter 3.x Architecture, Lifecycle & Material 3 Guardrails (`GF080`, `GF062`)

### Async BuildContext, Resource Disposal & Design Tokens

- Guard every `BuildContext` usage after an `await` gap with `if (!context.mounted) return;` (`FLUTTER-BUILD-CONTEXT-ASYNC`).
- Dispose every `TextEditingController`, `AnimationController`, `ScrollController`, `FocusNode`, and `StreamSubscription` inside `dispose()` (`FLUTTER-MISSING-DISPOSE`), and never invoke `setState()` inside `build()` (`FLUTTER-SETSTATE-IN-BUILD`).
- Confine raw `Color(0x...)` / `Colors.*` to `lib/theme/**` (`FLUTTER-HARDCODED-COLOR`), provide `tooltip:` on every `IconButton` (`FLUTTER-MISSING-A11Y`), and keep `lib/l10n/*.arb` catalogs 100% ICU-valid and key-symmetric (`GF062`).
