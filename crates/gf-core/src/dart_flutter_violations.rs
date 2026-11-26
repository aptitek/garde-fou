//! Pure `#![no_std]` Tier 1 violations for Dart 3.6+ and Flutter 3.x (`GF080`).

use core::fmt;
use serde::{Deserialize, Serialize};

/// Native Dart 3.6+ null-safety, async, and idiom violations (`DART-*`).
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
pub enum DartViolation {
    /// Explicit `dynamic` type weakening sound null-safety and static typing.
    ExplicitDynamic,
    /// `print(...)` call in production Dart code instead of structured logging / `debugPrint`.
    AvoidPrint,
    /// Empty `catch (...) {}` block silently swallowing exceptions.
    EmptyCatch,
    /// Deprecated `dart:html` import instead of Wasm-compatible `package:web/web.dart`.
    DeprecatedDartHtml,
    /// Hardcoded `// ignore_for_file:` silencing lints globally.
    IgnoreForFileBypass,
}

impl DartViolation {
    /// Canonical Dart rule code (`DART-*`).
    #[must_use]
    pub const fn code(self) -> &'static str {
        const CODES: [&str; 5] = [
            "DART-EXPLICIT-DYNAMIC",
            "DART-AVOID-PRINT",
            "DART-EMPTY-CATCH",
            "DART-DEPRECATED-HTML",
            "DART-IGNORE-FOR-FILE",
        ];
        CODES[self as usize]
    }

    /// Actionable root-cause remediation.
    #[must_use]
    pub const fn remediation(self) -> &'static str {
        const REMS: [&str; 5] = [
            "Replace `dynamic` with `Object?`, a concrete type, or a sealed class hierarchy.",
            "Replace `print(...)` with `debugPrint(...)` or a structured logger (`package:logging`).",
            "Handle or rethrow the caught exception in `catch` instead of leaving an empty block.",
            "Migrate `import 'dart:html'` to `import 'package:web/web.dart'` for Dart 3 / Wasm compatibility.",
            "Remove `// ignore_for_file:` directives and fix lint diagnostics at their root cause.",
        ];
        REMS[self as usize]
    }
}

/// Native Flutter 3.x async-context, lifecycle, Material 3, and accessibility violations (`FLUTTER-*`).
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
pub enum FlutterViolation {
    /// Using `BuildContext` across an `await` gap without guarding `if (!mounted) return;` / `context.mounted`.
    BuildContextAcrossAsyncGap,
    /// `State` holds disposable controllers/subscriptions without releasing them in `dispose()`.
    MissingDispose,
    /// Hardcoded `Color(0x...)` or `Colors.*` outside `lib/theme/**` instead of `Theme.of(context).colorScheme`.
    HardcodedColorOutsideTheme,
    /// Calling `setState(...)` directly inside `Widget build(...)`.
    SetStateInBuild,
    /// `IconButton` missing `tooltip:` accessibility label.
    MissingIconButtonTooltip,
}

impl FlutterViolation {
    /// Canonical Flutter rule code (`FLUTTER-*`).
    #[must_use]
    pub const fn code(self) -> &'static str {
        const CODES: [&str; 5] = [
            "FLUTTER-BUILD-CONTEXT-ASYNC",
            "FLUTTER-MISSING-DISPOSE",
            "FLUTTER-HARDCODED-COLOR",
            "FLUTTER-SETSTATE-IN-BUILD",
            "FLUTTER-MISSING-A11Y",
        ];
        CODES[self as usize]
    }

    /// Actionable root-cause remediation.
    #[must_use]
    pub const fn remediation(self) -> &'static str {
        const REMS: [&str; 5] = [
            "Guard `BuildContext` usage after `await` with `if (!context.mounted) return;` (or `if (!mounted) return;`).",
            "Override `dispose()` and call `.dispose()` / `.cancel()` on every controller and `StreamSubscription`.",
            "Use `Theme.of(context).colorScheme` Material 3 tokens instead of raw `Color(0x...)` / `Colors.*` outside `lib/theme/`.",
            "Never call `setState()` inside `build()`; move state updates into event callbacks or lifecycle hooks.",
            "Provide a localized `tooltip:` on every `IconButton` for screen-reader and hover accessibility.",
        ];
        REMS[self as usize]
    }
}

/// Unified closed sum type for Dart 3.6+ and Flutter 3.x (`GF080`).
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
pub enum DartFlutterViolation {
    /// Native Dart 3 violation.
    Dart(DartViolation),
    /// Native Flutter 3 violation.
    Flutter(FlutterViolation),
}

impl DartFlutterViolation {
    /// Canonical rule code.
    #[must_use]
    pub const fn code(self) -> &'static str {
        match self {
            Self::Dart(v) => v.code(),
            Self::Flutter(v) => v.code(),
        }
    }

    /// Actionable remediation.
    #[must_use]
    pub const fn remediation(self) -> &'static str {
        match self {
            Self::Dart(v) => v.remediation(),
            Self::Flutter(v) => v.remediation(),
        }
    }
}

impl fmt::Display for DartFlutterViolation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.code())
    }
}
