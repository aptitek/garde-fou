//! Closed algebraic sum types for Tier 1 Shell (`sh`/`bash`) and PowerShell (`.ps1`/`.psm1`)
//! AST violations (`GF080`).

use core::fmt;
use serde::{Deserialize, Serialize};

/// Detected shell grammar dialect governing `SH-STRICT-MODE` and `SH-SC3000` bashism checks.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
pub enum ShellDialect {
    /// Strict POSIX `/bin/sh` (`sh`, `dash`, `ash`, `busybox sh`).
    PosixSh,
    /// GNU Bourne-Again Shell (`bash`, `ksh`, `zsh`).
    Bash,
}

impl ShellDialect {
    /// Canonical lowercase dialect label (`"sh"` or `"bash"`).
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::PosixSh => "sh",
            Self::Bash => "bash",
        }
    }
}

impl fmt::Display for ShellDialect {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Closed sum type of native Tier 1 POSIX/Bash Shell AST violations (`GF080`).
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
pub enum ShellViolation {
    /// Executable script missing `set -eu` (`PosixSh`) or `set -euo pipefail` (`Bash`) (`SH-STRICT-MODE`).
    MissingStrictMode,
    /// Unquoted `$var` / `${var}` subject to word-splitting and pathname expansion (`SH-SC2086`).
    UnquotedExpansion,
    /// Unquoted `$(cmd)` or `` `cmd` `` subject to word-splitting (`SH-SC2046`).
    UnquotedCommandSub,
    /// `local`/`export`/`readonly` combined with `$(cmd)`, masking its exit status (`SH-SC2155`).
    MaskedReturnCode,
    /// Recursive `rm` with `$var/` or `$var/*` without `${var:?}` guard (`SH-SC2115`).
    CatastrophicRm,
    /// `cd <dir>` without `|| exit`/`|| return` or conditional check (`SH-SC2164`).
    UncheckedCd,
    /// Non-POSIX bashism used inside a `#!/bin/sh` script or `/bin/sh` `RUN` layer (`SH-SC3000`).
    PosixBashism,
    /// Dynamic `eval` command execution (`SH-NO-EVAL`, `SC2294`).
    ForbiddenEval,
    /// Unencrypted `http://` or unhardened `curl ... | sh` / `wget -O- ... | sh` pipe (`SH-CURL-PIPE-UNSAFE`).
    CurlPipeUnsafe,
}

const SHELL_SPECS: [(&str, &str); 9] = [
    (
        "SH-STRICT-MODE",
        "Enable `set -eu` at the top of POSIX `#!/bin/sh` scripts or `set -euo pipefail` in `#!/bin/bash` scripts.",
    ),
    (
        "SH-SC2086",
        "Double-quote variable expansions (`\"$var\"`, `\"${var}\"`) to prevent word-splitting and globbing.",
    ),
    (
        "SH-SC2046",
        "Double-quote command substitutions (`\"$(cmd)\"`) so output is not split on `IFS` or expanded as globs.",
    ),
    (
        "SH-SC2155",
        "Declare and assign separately (`local out; out=\"$(cmd)\"`) so a failing command substitution is not masked by `local`/`export`/`readonly`.",
    ),
    (
        "SH-SC2115",
        "Guard recursive `rm` expansions with `${var:?}` (`rm -rf \"${BUILD_DIR:?}/\"*`) so an empty variable can never expand to `/` or `/*`.",
    ),
    (
        "SH-SC2164",
        "Guard `cd` with `cd \"$dir\" || exit 1` (or `|| return 1` inside functions) or wrap it in `if cd \"$dir\"; then ...`.",
    ),
    (
        "SH-SC3000",
        "Replace Bash-only syntax (`[[ ... ]]`, `(( ... ))`, arrays, `function`, `source`, `<<<`, `${var//a/b}`, `local`, bare `set -o pipefail`) with POSIX `sh` constructs or switch to `#!/usr/bin/env bash`.",
    ),
    (
        "SH-NO-EVAL",
        "Avoid `eval`; invoke commands directly with quoted arguments (`\"$@\"`) or explicit dispatch.",
    ),
    (
        "SH-CURL-PIPE-UNSAFE",
        "Use HTTPS with `curl -fsSL` (and prefer downloading + verifying SHA-256 checksums before executing) instead of unencrypted or unhardened `curl|sh` pipes.",
    ),
];

impl ShellViolation {
    /// All [`ShellViolation`] variants in deterministic order.
    pub const ALL: [Self; 9] = [
        Self::MissingStrictMode,
        Self::UnquotedExpansion,
        Self::UnquotedCommandSub,
        Self::MaskedReturnCode,
        Self::CatastrophicRm,
        Self::UncheckedCd,
        Self::PosixBashism,
        Self::ForbiddenEval,
        Self::CurlPipeUnsafe,
    ];

    /// Canonical ecosystem rule identifier (`"SH-..."`).
    #[must_use]
    pub const fn code(self) -> &'static str {
        SHELL_SPECS[self as usize].0
    }

    /// Actionable root-cause remediation snippet.
    #[must_use]
    pub const fn remediation(self) -> &'static str {
        SHELL_SPECS[self as usize].1
    }
}

/// Closed sum type of native Tier 1 PowerShell AST violations (`GF080`).
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
pub enum PowerShellViolation {
    /// Executable `.ps1` script missing `Set-StrictMode -Version Latest` or `$ErrorActionPreference = 'Stop'` (`PS-STRICT-HEADER`).
    MissingStrictHeader,
    /// `$expr -eq $null` / `$expr -ne $null` placing `$null` on RHS (`PS-NULL-LEFT`).
    NullOnRight,
    /// `Invoke-Expression` or `iex` dynamic code execution (`PS-NO-IEX`).
    AvoidInvokeExpression,
    /// Sensitive parameter (`$Password`, `$Secret`, `$ApiKey`, etc.) typed `[string]` or untyped (`PS-PLAIN-SECRET-PARAM`).
    PlainTextSecretParam,
    /// Empty `catch {}` block silently swallowing exceptions (`PS-EMPTY-CATCH`).
    EmptyCatchBlock,
    /// Global variable read/write via `$global:...` (`PS-GLOBAL-VAR`).
    GlobalVariable,
}

const PS_SPECS: [(&str, &str); 6] = [
    (
        "PS-STRICT-HEADER",
        "Add `Set-StrictMode -Version Latest` and `$ErrorActionPreference = 'Stop'` near the top of executable `.ps1` scripts.",
    ),
    (
        "PS-NULL-LEFT",
        "Place `$null` on the left-hand side (`$null -eq $value`, `$null -ne $value`) so collection operands are compared rather than filtered.",
    ),
    (
        "PS-NO-IEX",
        "Never call `Invoke-Expression` or `iex`; invoke cmdlets or executables directly via `& $cmd @args`.",
    ),
    (
        "PS-PLAIN-SECRET-PARAM",
        "Type credential and secret parameters as `[SecureString]` or `[PSCredential]` instead of plain-text `[string]`.",
    ),
    (
        "PS-EMPTY-CATCH",
        "Handle, log (`Write-Verbose` / `Write-Error`), or rethrow exceptions in `catch` blocks instead of swallowing them silently.",
    ),
    (
        "PS-GLOBAL-VAR",
        "Avoid `$global:...` mutable state; pass explicit parameters and return structured values from functions.",
    ),
];

impl PowerShellViolation {
    /// All [`PowerShellViolation`] variants in deterministic order.
    pub const ALL: [Self; 6] = [
        Self::MissingStrictHeader,
        Self::NullOnRight,
        Self::AvoidInvokeExpression,
        Self::PlainTextSecretParam,
        Self::EmptyCatchBlock,
        Self::GlobalVariable,
    ];

    /// Canonical ecosystem rule identifier (`"PS-..."`).
    #[must_use]
    pub const fn code(self) -> &'static str {
        PS_SPECS[self as usize].0
    }

    /// Actionable root-cause remediation snippet.
    #[must_use]
    pub const fn remediation(self) -> &'static str {
        PS_SPECS[self as usize].1
    }
}
