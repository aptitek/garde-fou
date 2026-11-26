//! Pure `#![no_std]` violations for PHP 8.4, Symfony 7, Doctrine ORM 3, and Twig 3 (`GF080`).

use core::fmt;
use serde::{Deserialize, Serialize};

/// Native PHP 8.4 safety and idiom violations (`PHP-*`).
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
pub enum PhpViolation {
    /// Missing `declare(strict_types=1);` at the top of a `.php` file.
    MissingStrictTypes,
    /// Dangerous `eval()`, `exec()`, `shell_exec()`, `system()`, `passthru()`, `unserialize()`, or `extract()`.
    UnsafeExecOrUnserialize,
    /// Direct superglobal access (`$_GET`, `$_POST`, `$_REQUEST`, `$_COOKIE`, `$_FILES`).
    SuperglobalDirectAccess,
    /// Loose `==` / `!=` comparison instead of strict `===` / `!==`.
    LooseComparison,
    /// Error-suppression operator `@` masking runtime failures.
    ErrorSuppressionOperator,
    /// Debug dump leftover (`var_dump()`, `dd()`, `dump()`, `print_r()`).
    DebugDumpLeftover,
}

impl PhpViolation {
    /// Canonical PHP rule code (`PHP-*`).
    #[must_use]
    pub const fn code(self) -> &'static str {
        const CODES: [&str; 6] = [
            "PHP-STRICT-TYPES",
            "PHP-UNSAFE-EXEC-OR-UNSERIALIZE",
            "PHP-SUPERGLOBAL-ACCESS",
            "PHP-LOOSE-COMPARISON",
            "PHP-ERROR-SUPPRESSION",
            "PHP-DEBUG-LEFTOVER",
        ];
        CODES[self as usize]
    }

    /// Actionable root-cause remediation.
    #[must_use]
    pub const fn remediation(self) -> &'static str {
        const REMS: [&str; 6] = [
            "Add `declare(strict_types=1);` immediately after `<?php` in every PHP source file.",
            "Remove `eval()`/`exec()`/`unserialize()`/`extract()`; use `Symfony\\Component\\Process\\Process` with array args and `json_decode(..., JSON_THROW_ON_ERROR)`.",
            "Inject `Symfony\\Component\\HttpFoundation\\Request` instead of reading `$_GET`/`$_POST`/`$_REQUEST` directly.",
            "Use strict identity comparisons (`===` / `!==`) instead of loose `==` / `!=`.",
            "Remove the `@` error-suppression operator and handle failures explicitly with exceptions or `Result` checks.",
            "Remove debug calls (`var_dump`, `dd`, `dump`, `print_r`) before committing.",
        ];
        REMS[self as usize]
    }
}

/// Native Symfony 7.x and Doctrine ORM 3 / DBAL 4 violations (`SYMFONY-*`).
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
pub enum SymfonyViolation {
    /// Service locator anti-pattern (`$this->container->get(...)` / `ContainerInterface::get(...)`).
    ServiceLocatorAntiPattern,
    /// Dynamic SQL or DQL string concatenation / interpolation in `createQuery` / `executeQuery` / `where`.
    RawSqlOrDqlInjection,
    /// Calling `EntityManagerInterface::flush()` inside a `for`/`foreach`/`while` loop (N+1 flush storm).
    FlushInsideLoop,
    /// Mutating `#[Route(..., methods: ['POST'|'PUT'|'DELETE'|'PATCH'])]` without `#[IsGranted]` / `denyAccessUnlessGranted`.
    MissingAccessControl,
    /// `final class` annotated with `#[ORM\Entity]` breaks Doctrine lazy proxy generation.
    FinalDoctrineEntity,
    /// Unbounded `->findAll()` call loading an entire table into memory.
    UnboundedFindAll,
    /// Legacy `@Route` / `@ORM\` docblock annotations instead of PHP 8 attributes.
    LegacyAnnotation,
}

impl SymfonyViolation {
    /// Canonical Symfony/Doctrine rule code (`SYMFONY-*`).
    #[must_use]
    pub const fn code(self) -> &'static str {
        const CODES: [&str; 7] = [
            "SYMFONY-SERVICE-LOCATOR",
            "SYMFONY-SQL-DQL-INJECTION",
            "SYMFONY-FLUSH-IN-LOOP",
            "SYMFONY-MISSING-AUTH",
            "SYMFONY-FINAL-ENTITY",
            "SYMFONY-UNBOUNDED-FINDALL",
            "SYMFONY-LEGACY-ANNOTATION",
        ];
        CODES[self as usize]
    }

    /// Actionable root-cause remediation.
    #[must_use]
    pub const fn remediation(self) -> &'static str {
        const REMS: [&str; 7] = [
            "Inject dependencies via constructor autowiring instead of pulling services from `$this->container->get()`.",
            "Bind query parameters via `->setParameter('name', $value)` instead of concatenating/interpolating variables into DQL or SQL.",
            "Persist entities inside the loop and call `$entityManager->flush()` once outside the loop (or in bounded batches).",
            "Protect mutating routes with `#[IsGranted('ROLE_...')]` or `$this->denyAccessUnlessGranted(...)`.",
            "Remove `final` from `#[ORM\\Entity]` classes (or use PHP 8.4 native lazy ghosts) so Doctrine proxies work safely.",
            "Replace `->findAll()` with a paginated `QueryBuilder` (`setFirstResult`/`setMaxResults`) or explicit limit.",
            "Migrate legacy `@Route`/`@ORM\\*` docblock annotations to native PHP 8 attributes (`#[Route]`, `#[ORM\\Entity]`).",
        ];
        REMS[self as usize]
    }
}

/// Native Twig 3 template security and structural violations (`TWIG-*`).
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
pub enum TwigViolation {
    /// Unescaped `|raw` filter or `{% autoescape false %}` enabling XSS.
    RawFilterXss,
    /// Inline `style="..."` or `<script>` tag inside a Twig template.
    InlineStyleOrScript,
    /// Leftover `{{ dump(...) }}` debug call in a Twig template.
    DumpLeftover,
    /// Unclosed or mismatched `{% block %}` / `{% if %}` / `{% for %}` / `{% macro %}` tag.
    UnbalancedBlock,
}

impl TwigViolation {
    /// Canonical Twig rule code (`TWIG-*`).
    #[must_use]
    pub const fn code(self) -> &'static str {
        const CODES: [&str; 4] = [
            "TWIG-RAW-XSS",
            "TWIG-INLINE-STYLE-OR-SCRIPT",
            "TWIG-DUMP-LEFTOVER",
            "TWIG-UNBALANCED-BLOCK",
        ];
        CODES[self as usize]
    }

    /// Actionable root-cause remediation.
    #[must_use]
    pub const fn remediation(self) -> &'static str {
        const REMS: [&str; 4] = [
            "Remove `|raw` and `{% autoescape false %}` so Twig auto-escapes untrusted HTML output.",
            "Move inline `style=` and `<script>` blocks out of Twig templates into `Forme` stylesheets or bundled Stimulus controllers.",
            "Remove `{{ dump(...) }}` calls from Twig templates before committing.",
            "Close every `{% block %}`, `{% if %}`, `{% for %}`, and `{% macro %}` with its matching `{% end... %}` tag.",
        ];
        REMS[self as usize]
    }
}

/// Unified closed sum type for PHP 8.4, Symfony 7, Doctrine ORM 3, and Twig 3 (`GF080`).
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
pub enum PhpSymfonyViolation {
    /// Native PHP 8.4 violation.
    Php(PhpViolation),
    /// Native Symfony 7 / Doctrine ORM 3 violation.
    Symfony(SymfonyViolation),
    /// Native Twig 3 template violation.
    Twig(TwigViolation),
}

impl PhpSymfonyViolation {
    /// Canonical rule code.
    #[must_use]
    pub const fn code(self) -> &'static str {
        match self {
            Self::Php(v) => v.code(),
            Self::Symfony(v) => v.code(),
            Self::Twig(v) => v.code(),
        }
    }

    /// Actionable remediation.
    #[must_use]
    pub const fn remediation(self) -> &'static str {
        match self {
            Self::Php(v) => v.remediation(),
            Self::Symfony(v) => v.remediation(),
            Self::Twig(v) => v.remediation(),
        }
    }
}

impl fmt::Display for PhpSymfonyViolation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.code())
    }
}
