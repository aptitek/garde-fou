//! Closed algebraic sum types for Tier 1 TypeScript & React 19 AST violations (`GF080`).

use serde::{Deserialize, Serialize};

/// Sub-category of hazardous `async` callbacks losing rejections or cleanup functions (`GF080`).
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
pub enum AsyncHazardKind {
    /// `new Promise(async (...) => ...)` swallowing rejections (`TS-NO-ASYNC-PROMISE-EXECUTOR`).
    AsyncPromiseExecutor,
    /// `useEffect(async () => ...)` / `useLayoutEffect(async () => ...)` (`TS-NO-ASYNC-EFFECT`).
    AsyncEffectCallback,
}

impl AsyncHazardKind {
    const fn code(self) -> &'static str {
        match self {
            Self::AsyncPromiseExecutor => "TS-NO-ASYNC-PROMISE-EXECUTOR",
            Self::AsyncEffectCallback => "TS-NO-ASYNC-EFFECT",
        }
    }

    const fn remediation(self) -> &'static str {
        match self {
            Self::AsyncPromiseExecutor => {
                "Avoid `async` executor functions inside `new Promise(...)`; return the inner promise directly or call `reject(err)` explicitly."
            }
            Self::AsyncEffectCallback => {
                "Define an inner `async function run() { ... }` inside `useEffect(() => { void run(); return cleanup; }, deps)` so the effect returns `void | (() => void)`."
            }
        }
    }
}

/// Sub-category of leftover TypeScript/JavaScript debug or console artifacts (`TS-NO-CONSOLE-DEBUG`).
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
pub enum TsDebugKind {
    /// `console.{log,debug,info,trace}` in `Code` outside CLI/server `entrypoints`.
    ConsoleCall,
    /// `debugger;` statement anywhere in `Code`.
    DebuggerStmt,
}

impl TsDebugKind {
    /// Actionable root-cause remediation snippet.
    #[must_use]
    pub const fn remediation(self) -> &'static str {
        match self {
            Self::ConsoleCall => {
                "Use a structured logger or restrict console output to declared CLI/server `entrypoints`."
            }
            Self::DebuggerStmt => "Remove `debugger;` statements before committing.",
        }
    }
}

const REM_TS_ANY: &str = "Replace `any` with `unknown`, generic type parameters `<T>`, discriminated unions, or explicit schema validation.";
const REM_TS_DBL: &str = "Eliminate `as unknown as T`, `as any as T`, and `<T>expr` casts; validate unknown payloads via type guards or `satisfies`.";
const REM_TS_OPT: &str = "Never combine `?.` with `!` (`a?.b!` throws `TypeError` on `undefined`) or double `!!`; handle `undefined` explicitly via `??` or narrowing.";

/// Closed sum type of native Tier 1 TypeScript AST violations (`GF080`).
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
pub enum TypescriptViolation {
    /// Explicit `any` keyword in `DomainKind::Code` (`TS-NO-EXPLICIT-ANY`).
    ExplicitAny,
    /// Double assertion `as unknown as T` / `as any as T` or legacy `<Foo>expr` (`TS-NO-DOUBLE-ASSERTION`).
    DoubleOrAngleAssertion,
    /// Non-null assertion on optional chain (`a?.b!`, `(a?.b)!`, `a?.b!.c`) or `x!!` (`TS-NO-NON-NULL-OPTIONAL-CHAIN`).
    NonNullOptionalChain,
    /// Hazardous `async` Promise executor or React effect (`TS-NO-ASYNC-PROMISE-EXECUTOR` / `TS-NO-ASYNC-EFFECT`).
    AsyncHazard(AsyncHazardKind),
    /// Leftover `console.{log,debug,info,trace}` or `debugger;` (`TS-NO-CONSOLE-DEBUG`).
    ConsoleOrDebugger(TsDebugKind),
}

impl TypescriptViolation {
    /// Canonical ecosystem rule identifier (`"TS-..."`).
    #[must_use]
    pub const fn code(self) -> &'static str {
        match self {
            Self::ExplicitAny => "TS-NO-EXPLICIT-ANY",
            Self::DoubleOrAngleAssertion => "TS-NO-DOUBLE-ASSERTION",
            Self::NonNullOptionalChain => "TS-NO-NON-NULL-OPTIONAL-CHAIN",
            Self::AsyncHazard(k) => k.code(),
            Self::ConsoleOrDebugger(_) => "TS-NO-CONSOLE-DEBUG",
        }
    }

    /// Actionable root-cause remediation snippet.
    #[must_use]
    pub const fn remediation(self) -> &'static str {
        match self {
            Self::ExplicitAny => REM_TS_ANY,
            Self::DoubleOrAngleAssertion => REM_TS_DBL,
            Self::NonNullOptionalChain => REM_TS_OPT,
            Self::AsyncHazard(k) => k.remediation(),
            Self::ConsoleOrDebugger(k) => k.remediation(),
        }
    }

    /// Returns `true` when exempted inside declared CLI/server `entrypoints`.
    #[must_use]
    pub const fn is_entrypoint_exempt(self) -> bool {
        matches!(self, Self::ConsoleOrDebugger(TsDebugKind::ConsoleCall))
    }
}

/// Closed sum type of native Tier 1 React 19 & JSX AST violations (`GF080`).
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
pub enum ReactViolation {
    /// Conditional, looped, post-early-return, callback, or non-component Hook call (`REACT-RULES-OF-HOOKS`).
    RulesOfHooks,
    /// React component declared inside another component body (`REACT-NO-UNSTABLE-NESTED-COMPONENT`).
    UnstableNestedComponent,
    /// Missing `key`, shorthand `<>` in iterator, or array-index `key={idx}` (`REACT-JSX-KEY`).
    MissingOrIndexJsxKey,
    /// `dangerouslySetInnerHTML`, `javascript:` URI, or unsafe `target="_blank"` (`REACT-NO-DANGEROUS-JSX`).
    DangerousJsx,
}

impl ReactViolation {
    /// Canonical ecosystem rule identifier (`"REACT-..."`).
    #[must_use]
    pub const fn code(self) -> &'static str {
        match self {
            Self::RulesOfHooks => "REACT-RULES-OF-HOOKS",
            Self::UnstableNestedComponent => "REACT-NO-UNSTABLE-NESTED-COMPONENT",
            Self::MissingOrIndexJsxKey => "REACT-JSX-KEY",
            Self::DangerousJsx => "REACT-NO-DANGEROUS-JSX",
        }
    }

    /// Actionable root-cause remediation snippet.
    #[must_use]
    pub const fn remediation(self) -> &'static str {
        match self {
            Self::RulesOfHooks => {
                "Call React Hooks unconditionally at the top level of a React function component or custom `use*` Hook before any early `return` (only React 19 `use(...)` may run conditionally)."
            }
            Self::UnstableNestedComponent => {
                "Hoist nested `PascalCase` component definitions to module scope and pass dynamic values via explicit `props`."
            }
            Self::MissingOrIndexJsxKey => {
                "Provide a stable domain identifier `key={item.id}` on iterated JSX elements or `<Fragment key={item.id}>` — never omit `key`, use `<>`, or pass array `index`."
            }
            Self::DangerousJsx => {
                "Avoid `dangerouslySetInnerHTML` and `javascript:` URLs; add `rel=\"noopener noreferrer\"` on external `<a target=\"_blank\">` links."
            }
        }
    }
}

/// Returns `true` if `spec` names a well-known external TypeScript preset with `"strict": true`.
#[must_use]
pub fn is_known_strict_preset(spec: &str) -> bool {
    let s = spec.trim();
    s.starts_with("@tsconfig/") || s.contains("tsconfig/strict") || s.contains("tsconfigs/strict")
}

/// Returns `true` for ambient TypeScript declaration files (`*.d.ts`, `*.d.mts`, `*.d.cts`).
#[must_use]
pub fn is_ts_declaration_path(path: &str) -> bool {
    let lower = path.to_ascii_lowercase();
    let Some((stem, ext)) = lower.rsplit_once('.') else {
        return false;
    };
    let ends_with_d = stem.rsplit_once('.').is_some_and(|(_, sub)| sub == "d");
    matches!(ext, "ts" | "mts" | "cts") && (ends_with_d || stem.contains(".d."))
}
