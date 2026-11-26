//! Pure `#![no_std]` Tier 1 AST violation sum types (`GoViolation`, `KotlinViolation`,
//! `CSharpViolation`) and inter-file module/interface metadata (`CompiledModuleSummary`).

use alloc::collections::BTreeSet;
use alloc::string::String;
use alloc::vec::Vec;
use serde::{Deserialize, Serialize};

use crate::diagnostic::Span;

/// Native Go AST & runtime-safety violations (`GF080`, `GO-*`).
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
pub enum GoViolation {
    /// Returning a typed-nil concrete pointer where an `error` or interface is expected (`SA4023`).
    TypedNilInterface,
    /// `defer` inside a `for` loop causing resource leaks or mutex deadlocks.
    DeferInLoop,
    /// `defer res.Close()` registered before checking `if err != nil` (`SA5001`).
    DeferBeforeErrCheck,
    /// Discarded error (`_ = fn()`, `x, _ := fn()`) or unchecked critical write/close (`errcheck`).
    ErrcheckBlankOrIgnored,
    /// `context.Context` stored in a struct, not first param, or `Background`/`TODO` shadowing `ctx`.
    ContextMisuse,
    /// `panic(...)` in `Code` outside `main`/`init`, debug `fmt.Print*`, or `time.Sleep` in tests.
    GoroutineAndPanic,
}

impl GoViolation {
    /// Canonical rule code (`GO-*`).
    #[must_use]
    pub const fn code(self) -> &'static str {
        const CODES: [&str; 6] = [
            "GO-TYPED-NIL-INTERFACE",
            "GO-DEFER-IN-LOOP",
            "GO-DEFER-BEFORE-ERR-CHECK",
            "GO-ERRCHECK-BLANK-OR-IGNORED",
            "GO-CONTEXT-MISUSE",
            "GO-GOROUTINE-AND-PANIC",
        ];
        CODES[self as usize]
    }

    /// Actionable root-cause remediation.
    #[must_use]
    pub const fn remediation(self) -> &'static str {
        const FIXES: [&str; 6] = [
            "Return untyped `nil` explicitly instead of a typed pointer variable when returning `error` or an interface (`SA4023`).",
            "Extract the loop body into a helper function so `defer` runs at each iteration boundary instead of accumulating until function exit.",
            "Check `if err != nil { return err }` immediately before calling `defer res.Close()` (`SA5001`).",
            "Handle the returned `error` explicitly instead of discarding it with `_` or ignoring critical I/O return values.",
            "Pass `ctx context.Context` as the first parameter, never store it in a struct, and propagate `ctx` instead of `context.Background()`/`TODO()`.",
            "Return typed `error` values instead of `panic(...)` in library code, remove debug `fmt.Print*`, and avoid `time.Sleep` in tests.",
        ];
        FIXES[self as usize]
    }
}

/// Native Kotlin AST & coroutine-safety violations (`GF080`, `KT-*`).
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
pub enum KotlinViolation {
    /// Forbidden non-null assertion operator `!!` in `Code`.
    NoNotNullAssertion,
    /// `runCatching` or broad `catch (e: Exception|Throwable)` swallowing `CancellationException` in `suspend`.
    SuspendRunCatchingCancellation,
    /// `GlobalScope.launch/async`, `Thread.sleep` in `suspend`, `runBlocking` in `Code`, or `suspend fun ...: Flow<T>`.
    CoroutineStructuredSafety,
    /// Empty `catch`, `: Any` / `: Any?` domain type, debug `println`, or `*array` spread inside loops.
    SwallowedExceptionAndAny,
}

impl KotlinViolation {
    /// Canonical rule code (`KT-*`).
    #[must_use]
    pub const fn code(self) -> &'static str {
        const CODES: [&str; 4] = [
            "KT-NO-NOT-NULL-ASSERTION",
            "KT-SUSPEND-RUNCATCHING-CANCELLATION",
            "KT-COROUTINE-STRUCTURED-SAFETY",
            "KT-SWALLOWED-EXCEPTION-AND-ANY",
        ];
        CODES[self as usize]
    }

    /// Actionable root-cause remediation.
    #[must_use]
    pub const fn remediation(self) -> &'static str {
        const FIXES: [&str; 4] = [
            "Replace `!!` with safe calls (`?.`), Elvis (`?:`), `?.let`, or `requireNotNull(x) { \"reason\" }`.",
            "Avoid `runCatching` in `suspend` functions and always rethrow `CancellationException` (`if (e is CancellationException) throw e`).",
            "Use structured `CoroutineScope`, `delay(...)` instead of `Thread.sleep`, avoid `runBlocking` outside `main`, and return cold `Flow<T>` without `suspend`.",
            "Handle or rethrow exceptions explicitly, replace `: Any` with sealed/generic domain types, remove `println`, and avoid `*spread` inside loops.",
        ];
        FIXES[self as usize]
    }
}

/// Native C# / .NET AST & async/CLR-safety violations (`GF080`, `CS-*`).
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
pub enum CSharpViolation {
    /// Primary constructor parameter double-captured (`CS9124`) or mutated inside a member body.
    PrimaryCtorDoubleCapture,
    /// `async void` method or local function in `Code`.
    NoAsyncVoid,
    /// Blocking `.Result`, `.Wait()`, or `.GetAwaiter().GetResult()` causing thread-pool starvation.
    SyncOverAsync,
    /// `using` on unawaited `*Async(...)` (`MA0129`) or unpropagated `CancellationToken` (`MA0040`).
    DisposeAndCancellationSafety,
    /// `throw ex;` destroying original stacktrace (`CA2200`), empty `catch`, `GC.Collect()`, `Thread.Sleep` in `async`, or `Console.Write*` in `Code`.
    RethrowDestroysStack,
}

impl CSharpViolation {
    /// Canonical rule code (`CS-*`).
    #[must_use]
    pub const fn code(self) -> &'static str {
        const CODES: [&str; 5] = [
            "CS-PRIMARY-CTOR-DOUBLE-CAPTURE",
            "CS-NO-ASYNC-VOID",
            "CS-SYNC-OVER-ASYNC",
            "CS-DISPOSE-AND-CANCELLATION-SAFETY",
            "CS-RETHROW-DESTROYS-STACK",
        ];
        CODES[self as usize]
    }

    /// Actionable root-cause remediation.
    #[must_use]
    pub const fn remediation(self) -> &'static str {
        const FIXES: [&str; 5] = [
            "Reference the initialized field/property instead of capturing a primary constructor parameter twice (`CS9124`), and never mutate primary ctor parameters.",
            "Return `Task` or `ValueTask` instead of `async void` so exceptions are observable and awaitable.",
            "Use `await` end-to-end instead of `.Result`, `.Wait()`, or `.GetAwaiter().GetResult()`.",
            "Await `*Async` calls before `using`/`await using` (`MA0129`) and forward `CancellationToken` to inner awaited operations (`MA0040`).",
            "Use bare `throw;` to preserve stacktrace (`CA2200`), handle exceptions explicitly, and avoid `GC.Collect()`, `Thread.Sleep` in `async`, or `Console.Write*` in libraries.",
        ];
        FIXES[self as usize]
    }
}

/// Closed sum type grouping Go, Kotlin, and C# Tier 1 AST violations (`GF080`).
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
pub enum CompiledLangViolation {
    /// Go native AST violation (`GO-*`).
    Go(GoViolation),
    /// Kotlin native AST violation (`KT-*`).
    Kotlin(KotlinViolation),
    /// C# native AST violation (`CS-*`).
    CSharp(CSharpViolation),
}

impl CompiledLangViolation {
    /// Canonical rule code (`GO-*`, `KT-*`, `CS-*`).
    #[must_use]
    pub const fn code(self) -> &'static str {
        match self {
            Self::Go(v) => v.code(),
            Self::Kotlin(v) => v.code(),
            Self::CSharp(v) => v.code(),
        }
    }

    /// Actionable root-cause remediation.
    #[must_use]
    pub const fn remediation(self) -> &'static str {
        match self {
            Self::Go(v) => v.remediation(),
            Self::Kotlin(v) => v.remediation(),
            Self::CSharp(v) => v.remediation(),
        }
    }
}

/// Go structural interface declaration (`type Repo interface { Find(...) }`) for implicit `GF075` matching.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct StructuralInterfaceDecl {
    /// Declared interface name.
    pub name: String,
    /// Source span of the declaration.
    pub span: Span,
    /// Set of required method names on the interface.
    pub methods: BTreeSet<String>,
}

/// Go concrete type method set extracted from `func (r *T) Method(...)` declarations.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ConcreteTypeMethods {
    /// Base concrete type identifier (`T` normalized from `T` or `*T`).
    pub type_name: String,
    /// Set of method names defined with receiver `T` or `*T`.
    pub methods: BTreeSet<String>,
}

/// Cross-file module, package/namespace, and structural interface metadata for Go, Kotlin, and C#.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct CompiledModuleSummary {
    /// Declared package (`package a.b.c` in Kotlin, `package foo` in Go) or C# `namespace A.B.C`.
    pub package_or_namespace: Option<String>,
    /// Root Go module import path parsed from `go.mod` (e.g. `"example.com/service"`).
    pub go_module_path: Option<String>,
    /// Non-empty Go interfaces declared in this file for structural `GF075` satisfaction.
    pub go_interfaces: Vec<StructuralInterfaceDecl>,
    /// Go concrete receiver method sets declared in this file.
    pub go_concrete_types: Vec<ConcreteTypeMethods>,
}
