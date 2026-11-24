//! Application and workspace [`FrameworkId`] definitions and implicit entrypoint globs.

use core::fmt;
use serde::{Deserialize, Serialize};

/// Application or workspace frameworks informing implicit entrypoint routing.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
pub enum FrameworkId {
    /// Multi-crate or single-crate Cargo workspace.
    CargoWorkspace,
    /// React component library or SPA.
    React,
    /// Astro content/SSR framework (`src/pages/**`).
    Astro,
    /// Next.js Pages/App Router framework (`src/pages/**`, `src/app/**`).
    NextJs,
    /// `FastAPI` Python ASGI service.
    FastApi,
    /// Django Python web framework.
    Django,
}

const FRAMEWORK_NAMES: [&str; 6] = [
    "cargo-workspace",
    "react",
    "astro",
    "nextjs",
    "fastapi",
    "django",
];

const FRAMEWORK_ALIASES: &[(&str, FrameworkId)] = &[
    ("cargo-workspace", FrameworkId::CargoWorkspace),
    ("cargo", FrameworkId::CargoWorkspace),
    ("react", FrameworkId::React),
    ("astro", FrameworkId::Astro),
    ("nextjs", FrameworkId::NextJs),
    ("next", FrameworkId::NextJs),
    ("fastapi", FrameworkId::FastApi),
    ("django", FrameworkId::Django),
];

const CARGO_ENTRIES: &[&str] = &[
    "src/lib.rs",
    "src/main.rs",
    "crates/*/src/lib.rs",
    "crates/*/src/main.rs",
    "crates/*/tests/**",
    "tests/**",
];

const NEXT_ENTRIES: &[&str] = &[
    "pages/**",
    "src/pages/**",
    "app/**",
    "src/app/**",
    "middleware.*",
    "src/middleware.*",
];

impl FrameworkId {
    /// Parses a case-insensitive framework identifier from configuration.
    #[must_use]
    pub fn from_config_str(raw: &str) -> Option<Self> {
        FRAMEWORK_ALIASES
            .iter()
            .find(|(alias, _)| raw.eq_ignore_ascii_case(alias))
            .map(|(_, id)| *id)
    }

    /// Canonical kebab-case identifier.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        FRAMEWORK_NAMES[self as usize]
    }

    /// Returns implicit entrypoint path globs associated with this framework.
    #[must_use]
    pub const fn implicit_entrypoints(self) -> &'static [&'static str] {
        match self {
            Self::CargoWorkspace => CARGO_ENTRIES,
            Self::React => &["src/index.*", "src/main.*", "src/App.*"],
            Self::Astro => &["src/pages/**", "src/middleware.*", "astro.config.*"],
            Self::NextJs => NEXT_ENTRIES,
            Self::FastApi => &["main.py", "app/main.py", "src/main.py", "**/routers/**"],
            Self::Django => &["manage.py", "**/views.py", "**/urls.py", "**/admin.py"],
        }
    }
}

impl fmt::Display for FrameworkId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}
