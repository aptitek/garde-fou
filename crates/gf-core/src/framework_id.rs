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
    /// Astro content/SSR & islands framework (`src/pages/**`, `src/layouts/**`).
    Astro,
    /// Next.js Pages/App Router framework (`src/pages/**`, `src/app/**`).
    NextJs,
    /// `FastAPI` Python ASGI service.
    FastApi,
    /// Django Python web framework.
    Django,
    /// React Router v7 Data & Framework mode (`app/root.tsx`, `app/routes.ts`, `**/routes/**`).
    ReactRouter,
    /// Cloudflare Workers edge runtime (`wrangler.*`, `D1`, `R2`, `cf`).
    CloudflareWorkers,
    /// Drizzle ORM typed SQL schema & query builder (`drizzle.config.*`).
    Drizzle,
}

const FRAMEWORK_NAMES: [&str; 9] = [
    "cargo-workspace",
    "react",
    "astro",
    "nextjs",
    "fastapi",
    "django",
    "react-router",
    "cloudflare-workers",
    "drizzle",
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
    ("react-router", FrameworkId::ReactRouter),
    ("reactrouter", FrameworkId::ReactRouter),
    ("remix", FrameworkId::ReactRouter),
    ("cloudflare-workers", FrameworkId::CloudflareWorkers),
    ("cloudflare", FrameworkId::CloudflareWorkers),
    ("workers", FrameworkId::CloudflareWorkers),
    ("cf-workers", FrameworkId::CloudflareWorkers),
    ("drizzle", FrameworkId::Drizzle),
    ("drizzle-orm", FrameworkId::Drizzle),
];

const CARGO_ENTRIES: &[&str] = &[
    "src/lib.rs",
    "src/main.rs",
    "crates/*/src/lib.rs",
    "crates/*/src/main.rs",
    "crates/*/tests/**",
    "tests/**",
];

const ASTRO_ENTRIES: &[&str] = &[
    "src/pages/**",
    "src/layouts/**",
    "src/middleware.*",
    "src/content.config.*",
    "src/content/config.*",
    "astro.config.*",
];

const NEXT_ENTRIES: &[&str] = &[
    "pages/**",
    "src/pages/**",
    "app/**",
    "src/app/**",
    "middleware.*",
    "src/middleware.*",
];

const REACT_ROUTER_ENTRIES: &[&str] = &[
    "app/root.*",
    "src/root.*",
    "root.*",
    "app/routes.*",
    "src/routes.*",
    "app/routes/**",
    "src/routes/**",
    "**/routes/**",
    "react-router.config.*",
];

const CLOUDFLARE_ENTRIES: &[&str] = &[
    "src/index.*",
    "src/worker.*",
    "worker.*",
    "workers/**",
    "functions/**",
];

const DRIZZLE_ENTRIES: &[&str] = &[
    "drizzle.config.*",
    "src/db/schema.*",
    "src/db/schema/**",
    "db/schema.*",
    "**/schema.ts",
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
            Self::Astro => ASTRO_ENTRIES,
            Self::NextJs => NEXT_ENTRIES,
            Self::FastApi => &["main.py", "app/main.py", "src/main.py", "**/routers/**"],
            Self::Django => &["manage.py", "**/views.py", "**/urls.py", "**/admin.py"],
            Self::ReactRouter => REACT_ROUTER_ENTRIES,
            Self::CloudflareWorkers => CLOUDFLARE_ENTRIES,
            Self::Drizzle => DRIZZLE_ENTRIES,
        }
    }
}

impl fmt::Display for FrameworkId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}
