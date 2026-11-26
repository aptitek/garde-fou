//! Closed algebraic sum types for modern exclusive toolchains (`pnpm`, `uv`, Gradle `.kts`),
//! modern Node.js (`NODE-*`), and MERN TypeScript (`EXPRESS-*`, `MONGO-*`).

use alloc::collections::BTreeSet;
use alloc::string::String;
use alloc::vec::Vec;
use core::fmt;
use serde::{Deserialize, Serialize};

use crate::diagnostic::Span;
use crate::external_tool_kind::ExternalToolKind;

/// Closed sum type of modern toolchain exclusivity & manifest violations (`GF081`).
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
pub enum ToolchainViolation {
    /// Legacy JS/TS package manager (`npm`, `npx`, `yarn`, `package-lock.json`, `yarn.lock`) or missing pinned `packageManager: pnpm@x.y.z`.
    PnpmOnly,
    /// Legacy Python package manager (`pip`, `poetry`, `pipenv`, `requirements.txt`, `setup.py`, `Pipfile`, `environment.yml`).
    UvOnly,
    /// Legacy Groovy Gradle (`build.gradle` / `settings.gradle`), dynamic `+`/`SNAPSHOT` version, or insecure `http://` wrapper URL.
    GradleKtsOnly,
    /// Hardcoded `"group:artifact:version"` string in `build.gradle.kts` when `gradle/libs.versions.toml` is present.
    GradleUseVersionCatalog,
    /// Missing `"type": "module"` in a modern Node.js / TypeScript `package.json`.
    NodeEsmModuleType,
}

impl ToolchainViolation {
    /// Canonical diagnostic code (`TOOLCHAIN-*` / `GRADLE-*` / `NODE-*`).
    #[must_use]
    pub const fn code(self) -> &'static str {
        const CODES: [&str; 5] = [
            "TOOLCHAIN-PNPM-ONLY",
            "TOOLCHAIN-UV-ONLY",
            "TOOLCHAIN-GRADLE-KTS-ONLY",
            "GRADLE-USE-VERSION-CATALOG",
            "NODE-MODERN-ESM-HYGIENE",
        ];
        CODES[self as usize]
    }

    /// Associated ecosystem tool for `GF081`.
    #[must_use]
    pub const fn tool(self) -> ExternalToolKind {
        match self {
            Self::PnpmOnly | Self::NodeEsmModuleType => ExternalToolKind::Pnpm,
            Self::UvOnly => ExternalToolKind::Uv,
            Self::GradleKtsOnly | Self::GradleUseVersionCatalog => ExternalToolKind::Gradle,
        }
    }

    /// Actionable root-cause remediation snippet.
    #[must_use]
    pub const fn remediation(self) -> &'static str {
        const REM: [&str; 5] = [
            "Use `pnpm` exclusively: delete npm/yarn lockfiles, pin `\"packageManager\": \"pnpm@<version>\"` in `package.json`, and run `pnpm install --frozen-lockfile` / `pnpm exec` / `pnpm dlx`.",
            "Use `uv` exclusively: replace `pip`/`poetry`/`pipenv`/`requirements.txt`/`setup.py` with `pyproject.toml` + `uv.lock` (`uv sync --locked` / `uv run`).",
            "Use Gradle Kotlin DSL (`build.gradle.kts` / `settings.gradle.kts`) with pinned versions and HTTPS `gradle-wrapper.properties`.",
            "Declare the dependency in `gradle/libs.versions.toml` and reference it via typed `libs.*` accessors in `build.gradle.kts`.",
            "Declare `\"type\": \"module\"` in `package.json` to enforce native ESM hygiene in modern Node.js projects.",
        ];
        REM[self as usize]
    }
}

/// Closed sum type of modern Node.js runtime CST violations (`GF080`).
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
pub enum NodeViolation {
    /// Node.js builtin imported without the explicit `node:` protocol prefix.
    ProtocolImport,
    /// Synchronous blocking I/O (`*Sync`) inside an `async` function, Express handler, or HTTP server.
    NoSyncIoInHandler,
    /// Legacy `new Buffer(...)`, `CommonJS` `require(...)` in ESM, or `process.exit(...)` in library code.
    ModernEsmHygiene,
}

impl NodeViolation {
    /// Canonical `NODE-*` diagnostic code.
    #[must_use]
    pub const fn code(self) -> &'static str {
        const CODES: [&str; 3] = [
            "NODE-PROTOCOL-IMPORT",
            "NODE-NO-SYNC-IO-IN-HANDLER",
            "NODE-MODERN-ESM-HYGIENE",
        ];
        CODES[self as usize]
    }

    /// Actionable root-cause remediation snippet.
    #[must_use]
    pub const fn remediation(self) -> &'static str {
        const REM: [&str; 3] = [
            "Prefix Node.js built-in module imports with `node:` (e.g. `import { readFile } from \"node:fs/promises\"`).",
            "Replace blocking `*Sync` calls with async `node:fs/promises` / `node:crypto` / `node:child_process` APIs inside async handlers.",
            "Use `Buffer.from(...)`, static ESM `import`, and structured error returns instead of `new Buffer`, `require()`, or `process.exit()` in library modules.",
        ];
        REM[self as usize]
    }
}

/// Closed sum type of MERN TypeScript (Express 5 + MongoDB/Mongoose) CST violations (`GF080`).
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
pub enum MernViolation {
    /// `NoSQL` operator injection (`$where`/`$function`/`$accumulator` or unvalidated `req.body`/`query`/`params` in MongoDB filter).
    MongoNoSqlInjection,
    /// Unbounded `deleteMany({})`/`updateMany({})`, `find()` without `.limit()`, or awaited MongoDB call inside a loop (N+1).
    MongoUnboundedQueryOrMutation,
    /// `Schema.Types.Mixed` or `ref: "..."` field missing an index in a Mongoose schema.
    MongoSchemaStrictness,
    /// Unsafe `req.body as Dto` / `req.query as Dto` or direct destructuring without a runtime schema validator (`Zod`/`Valibot`/`ArkType`/`TypeBox`).
    ExpressUnvalidatedRequestCast,
    /// Permissive `cors({ origin: "*", credentials: true })` / `origin: true` + credentials, or leaking `err.stack` in HTTP response.
    ExpressInsecureCorsOrErrorLeak,
    /// Calling `app.listen(...)` in a module that defines routes/middlewares and exports `app`.
    ExpressListenInAppModule,
}

impl MernViolation {
    /// Canonical `MONGO-*` / `EXPRESS-*` diagnostic code.
    #[must_use]
    pub const fn code(self) -> &'static str {
        const CODES: [&str; 6] = [
            "MONGO-NOSQL-INJECTION",
            "MONGO-UNBOUNDED-QUERY-OR-MUTATION",
            "MONGO-SCHEMA-STRICTNESS",
            "EXPRESS-UNVALIDATED-REQUEST-CAST",
            "EXPRESS-INSECURE-CORS-OR-ERROR-LEAK",
            "EXPRESS-LISTEN-IN-APP-MODULE",
        ];
        CODES[self as usize]
    }

    /// Actionable root-cause remediation snippet.
    #[must_use]
    pub const fn remediation(self) -> &'static str {
        const REM: [&str; 6] = [
            "Validate request inputs with a runtime schema (`Zod`/`Valibot`) and wrap untrusted filter values in `{ $eq: value }`; never use `$where`/`$function`/`$accumulator`.",
            "Scope `deleteMany`/`updateMany` with explicit filters, chain `.limit(...)` on `find()`, and replace loop queries with `$in` or `bulkWrite()`.",
            "Replace `Schema.Types.Mixed` with typed sub-schemas and add `index: true` (or `.index(...)`) on every `ref:` foreign-key field.",
            "Parse `req.body` / `req.query` / `req.params` through a runtime validator (`schema.parse(...)` / `safeParse(...)`) instead of TypeScript `as` casts.",
            "Restrict `cors({ origin: allowlist, credentials: true })` to explicit origins and never send `err.stack` in HTTP responses.",
            "Separate the testable Express `app` factory (`app.ts`) from the runtime entrypoint (`server.ts`) that calls `app.listen(...)`.",
        ];
        REM[self as usize]
    }
}

/// Workspace-level toolchain & Gradle catalog metadata attached to [`crate::UastDocument`].
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct ToolchainSummary {
    /// Package names listed in `package.json` dependencies (to suppress browser `NODE-PROTOCOL-IMPORT` FPs).
    #[serde(default)]
    pub package_json_deps: BTreeSet<String>,
    /// `true` when this file is `gradle/libs.versions.toml`.
    #[serde(default)]
    pub is_gradle_version_catalog: bool,
    /// Hardcoded `"group:artifact:version"` dependencies found in `build.gradle.kts` (`Span`, raw coord).
    #[serde(default)]
    pub hardcoded_gradle_deps: Vec<(Span, String)>,
}

const LEGACY_TOOLCHAIN_FILENAMES: &[&str] = &[
    "package-lock.json",
    "npm-shrinkwrap.json",
    "yarn.lock",
    ".yarnrc",
    ".yarnrc.yml",
    ".pnp.cjs",
    "pipfile",
    "pipfile.lock",
    "poetry.lock",
    "pdm.lock",
    "setup.py",
    "setup.cfg",
    "environment.yml",
    "environment.yaml",
    "requirements.txt",
    "requirements-dev.txt",
    "build.gradle",
    "settings.gradle",
    "gradle-wrapper.properties",
];

/// Returns `true` when `path` is a manifest, lockfile, or build config audited by `TOOLCHAIN-*`.
#[must_use]
pub fn is_toolchain_audited_file(path: &str) -> bool {
    let base = path.rsplit('/').next().unwrap_or(path);
    let low = base.to_ascii_lowercase();
    if low == "package.json" || low == "libs.versions.toml" {
        return true;
    }
    if low.starts_with("requirements") && low.strip_suffix(".txt").is_some() {
        return true;
    }
    LEGACY_TOOLCHAIN_FILENAMES.contains(&low.as_str())
}

impl fmt::Display for ToolchainViolation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.code())
    }
}

impl fmt::Display for NodeViolation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.code())
    }
}

impl fmt::Display for MernViolation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.code())
    }
}
