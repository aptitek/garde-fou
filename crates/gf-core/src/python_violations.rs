//! Closed algebraic sum types for Tier 1 Python AST violations (`GF080`)
//! and Tier 2 ecosystem configuration weakenings (`GF081`).

use core::fmt;
use serde::{Deserialize, Serialize};

use crate::diagnostic::Span;
pub use crate::language_meta::ExternalToolKind;

/// Sub-category of bare or silent Python `except` handlers (`PY-E722` / `PY-B001`).
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
pub enum ExceptViolationKind {
    /// Bare `except:` catching `SystemExit` and `KeyboardInterrupt` (`PY-E722`).
    Bare,
    /// Typed `except ...:` body containing only `pass`/`...`/docstring (`PY-B001`).
    Silent,
}

impl ExceptViolationKind {
    const fn code(self) -> &'static str {
        match self {
            Self::Bare => "PY-E722",
            Self::Silent => "PY-B001",
        }
    }

    const fn remediation(self) -> &'static str {
        match self {
            Self::Bare => {
                "Catch a specific exception class (e.g. `except ValueError as err:`) and handle or re-raise it explicitly."
            }
            Self::Silent => {
                "Log context, return an explicit error result, or re-raise with `raise ... from err` instead of swallowing exceptions."
            }
        }
    }
}

/// Sub-category of unsafe Python execution, deserialization, or shell invocation.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
pub enum UnsafeExecKind {
    /// `eval(...)` or `exec(...)` (`PY-S102`).
    DynamicEval,
    /// `pickle.load`/`loads`, `marshal.load`/`loads`, or `shelve.open` (`PY-S301`).
    UnsafePickle,
    /// `yaml.load(...)` without `Loader=yaml.SafeLoader` / `CSafeLoader` (`PY-S506`).
    UnsafeYamlLoad,
    /// `subprocess.*(..., shell=True)` or `os.system(...)` (`PY-S602`).
    ShellSubprocess,
}

impl UnsafeExecKind {
    const fn code(self) -> &'static str {
        match self {
            Self::DynamicEval => "PY-S102",
            Self::UnsafePickle => "PY-S301",
            Self::UnsafeYamlLoad => "PY-S506",
            Self::ShellSubprocess => "PY-S602",
        }
    }

    const fn remediation(self) -> &'static str {
        match self {
            Self::DynamicEval => {
                "Replace `eval`/`exec` with `ast.literal_eval`, explicit dispatch tables, or typed schema parsing."
            }
            Self::UnsafePickle => {
                "Use `json`, `pydantic`, or typed schemas instead of untrusted `pickle`/`marshal` deserialization."
            }
            Self::UnsafeYamlLoad => {
                "Use `yaml.safe_load(stream)` or pass `Loader=yaml.SafeLoader` explicitly to `yaml.load(...)`."
            }
            Self::ShellSubprocess => {
                "Pass command arguments as a list (`subprocess.run([\"cmd\", arg], check=True)`) without `shell=True`."
            }
        }
    }
}

/// Sub-category of leftover debug or console artifacts in Python `Code`.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
pub enum DebugArtifactKind {
    /// `print(...)` or `pprint(...)` in `Code` outside CLI entrypoints (`PY-T201`).
    PrintCall,
    /// `breakpoint()` or `pdb.set_trace()` debugger hook (`PY-T100`).
    DebuggerTrace,
}

impl DebugArtifactKind {
    const fn code(self) -> &'static str {
        match self {
            Self::PrintCall => "PY-T201",
            Self::DebuggerTrace => "PY-T100",
        }
    }

    const fn remediation(self) -> &'static str {
        match self {
            Self::PrintCall => {
                "Use `logging.getLogger(__name__)` or restrict console I/O to declared CLI `entrypoints`."
            }
            Self::DebuggerTrace => {
                "Remove `breakpoint()` / `pdb.set_trace()` interactive debugger hooks before committing."
            }
        }
    }
}

/// Closed sum type of native Tier 1 Python AST violations (`GF080`).
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
pub enum PythonViolation {
    /// Mutable literal or constructor call as parameter default (`PY-B006`).
    MutableDefaultArg,
    /// Bare `except:` (`PY-E722`) or silent `except ...: pass` (`PY-B001`).
    BareOrSilentExcept(ExceptViolationKind),
    /// Arbitrary code execution, unsafe deserialization, or `shell=True` (`PY-S102`/`S301`/`S506`/`S602`).
    UnsafeExecution(UnsafeExecKind),
    /// Explicit `Any` annotation or `typing.cast(Any, ...)` in `Code` (`PY-ANN401`).
    ForbiddenAny,
    /// Public function or method in `Code` missing parameter or return type annotation (`PY-ANN201`).
    MissingPublicTypeAnnotation,
    /// Leftover `print(...)` (`PY-T201`) or `breakpoint()` / `pdb.set_trace()` (`PY-T100`) in `Code`.
    LeftoverDebugArtifact(DebugArtifactKind),
}

impl PythonViolation {
    /// Canonical ecosystem rule identifier (`"PY-..."`).
    #[must_use]
    pub const fn code(self) -> &'static str {
        match self {
            Self::MutableDefaultArg => "PY-B006",
            Self::BareOrSilentExcept(kind) => kind.code(),
            Self::UnsafeExecution(kind) => kind.code(),
            Self::ForbiddenAny => "PY-ANN401",
            Self::MissingPublicTypeAnnotation => "PY-ANN201",
            Self::LeftoverDebugArtifact(kind) => kind.code(),
        }
    }

    /// Actionable root-cause remediation guidance.
    #[must_use]
    pub const fn remediation(self) -> &'static str {
        match self {
            Self::MutableDefaultArg => {
                "Default the parameter to `None` (`param: list[T] | None = None`) and allocate a fresh container inside the function body."
            }
            Self::BareOrSilentExcept(kind) => kind.remediation(),
            Self::UnsafeExecution(kind) => kind.remediation(),
            Self::ForbiddenAny => {
                "Replace `Any` with concrete types, `TypeVar`, `TypedDict`, `dataclass`, or a `Protocol`."
            }
            Self::MissingPublicTypeAnnotation => {
                "Add explicit type annotations on all public parameters (excluding `self`/`cls`) and return types (`-> T` or `-> None`)."
            }
            Self::LeftoverDebugArtifact(kind) => kind.remediation(),
        }
    }

    /// Returns `true` when this violation is exempted inside declared CLI `entrypoints`.
    #[must_use]
    pub const fn is_entrypoint_exempt(self) -> bool {
        matches!(
            self,
            Self::LeftoverDebugArtifact(DebugArtifactKind::PrintCall)
        )
    }
}

impl fmt::Display for PythonViolation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.code())
    }
}

/// Declared `[tool.coverage.report] fail_under` floor percentage extracted from `pyproject.toml`.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct CoverageFailUnder {
    /// Floor integer value of `fail_under` (`0..=100`).
    pub floor_pct: u32,
    /// 1-based source line span of `fail_under` in `pyproject.toml`.
    pub span: Span,
}
