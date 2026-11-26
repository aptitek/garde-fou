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
    /// Docker Compose infrastructure stack (`compose.yaml`, `docker-compose.yml`).
    DockerCompose,
    /// Obsidian Markdown & JSON Canvas knowledge vault (`.obsidian/`, `.canvas`).
    Obsidian,
    /// Quarto scientific/academic publishing (`_quarto.yml`, `*.qmd`).
    Quarto,
    /// Jupyter Notebook ecosystem (`*.ipynb`).
    Jupyter,
    /// Modern Node.js ESM runtime (`node:*` protocol, `pnpm`).
    NodeJs,
    /// Express 5 TypeScript HTTP framework (`app.ts` / `server.ts`).
    Express,
    /// MongoDB & Mongoose typed ODM (`Schema`, `Model`).
    Mongoose,
    /// `NiceGUI` reactive Python web UI (`@ui.page`, `run.io_bound`, `app.storage`).
    NiceGui,
    /// `PyTorch` deep learning framework (`torch`, `safetensors`).
    PyTorch,
    /// `TensorFlow` / `Keras 3` deep learning framework (`keras`, `tf.function`).
    TensorFlow,
    /// `LangChain` 1.x & `LangGraph` 1.x LLM agent framework (`StateGraph`).
    LangChain,
    /// `PostgreSQL` database (RLS, `SECURITY DEFINER`, PL/pgSQL, `pgTAP`).
    PostgreSql,
    /// `MariaDB` 10.11 / 11.4 database (`InnoDB`, `utf8mb4`, Online DDL).
    MariaDb,
}

#[rustfmt::skip]
const FRAMEWORK_NAMES: [&str; 22] = [
    "cargo-workspace", "react", "astro", "nextjs", "fastapi", "django", "react-router",
    "cloudflare-workers", "drizzle", "docker-compose", "obsidian", "quarto", "jupyter",
    "nodejs", "express", "mongoose", "nicegui", "pytorch", "tensorflow", "langchain",
    "postgresql", "mariadb",
];

#[rustfmt::skip]
const FRAMEWORK_ALIASES: &[(&str, FrameworkId)] = &[
    ("cargo-workspace", FrameworkId::CargoWorkspace), ("cargo", FrameworkId::CargoWorkspace),
    ("react", FrameworkId::React), ("astro", FrameworkId::Astro),
    ("nextjs", FrameworkId::NextJs), ("next", FrameworkId::NextJs),
    ("fastapi", FrameworkId::FastApi), ("django", FrameworkId::Django),
    ("react-router", FrameworkId::ReactRouter), ("reactrouter", FrameworkId::ReactRouter),
    ("remix", FrameworkId::ReactRouter), ("cloudflare-workers", FrameworkId::CloudflareWorkers),
    ("cloudflare", FrameworkId::CloudflareWorkers), ("workers", FrameworkId::CloudflareWorkers),
    ("cf-workers", FrameworkId::CloudflareWorkers), ("drizzle", FrameworkId::Drizzle),
    ("drizzle-orm", FrameworkId::Drizzle), ("docker-compose", FrameworkId::DockerCompose),
    ("compose", FrameworkId::DockerCompose), ("obsidian", FrameworkId::Obsidian),
    ("quarto", FrameworkId::Quarto), ("jupyter", FrameworkId::Jupyter), ("ipynb", FrameworkId::Jupyter),
    ("nodejs", FrameworkId::NodeJs), ("node", FrameworkId::NodeJs),
    ("express", FrameworkId::Express), ("mongoose", FrameworkId::Mongoose), ("mongodb", FrameworkId::Mongoose),
    ("nicegui", FrameworkId::NiceGui), ("pytorch", FrameworkId::PyTorch), ("torch", FrameworkId::PyTorch),
    ("tensorflow", FrameworkId::TensorFlow), ("keras", FrameworkId::TensorFlow),
    ("langchain", FrameworkId::LangChain), ("langgraph", FrameworkId::LangChain),
    ("postgresql", FrameworkId::PostgreSql), ("postgres", FrameworkId::PostgreSql), ("pg", FrameworkId::PostgreSql),
    ("mariadb", FrameworkId::MariaDb),
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

const REACT_ENTRIES: &[&str] = &[
    "src/index.*",
    "src/main.*",
    "src/App.*",
    "apps/*/src/main.*",
    "apps/*/src/App.*",
];
const FASTAPI_ENTRIES: &[&str] = &["main.py", "app/main.py", "src/main.py", "**/routers/**"];
const DJANGO_ENTRIES: &[&str] = &[
    "manage.py",
    "**/views.py",
    "**/urls.py",
    "**/admin.py",
    "**/apps.py",
    "**/models.py",
    "**/wsgi.py",
    "**/asgi.py",
    "**/migrations/**",
    "**/management/commands/**",
];
const NICEGUI_ENTRIES: &[&str] = &["main.py", "src/main.py", "**/pages/**"];
const NODE_ENTRIES: &[&str] = &[
    "src/server.*",
    "src/main.*",
    "src/index.*",
    "apps/*/src/server.*",
    "apps/*/src/main.*",
    "packages/*/src/index.*",
];

#[rustfmt::skip]
const FRAMEWORK_ENTRIES: [&[&str]; 22] = [
    CARGO_ENTRIES, REACT_ENTRIES, ASTRO_ENTRIES, NEXT_ENTRIES, FASTAPI_ENTRIES,
    DJANGO_ENTRIES, REACT_ROUTER_ENTRIES, CLOUDFLARE_ENTRIES, DRIZZLE_ENTRIES,
    &[], &[], &[], &[], NODE_ENTRIES, NODE_ENTRIES, &[], NICEGUI_ENTRIES,
    &[], &[], &[], &[], &[],
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
        FRAMEWORK_ENTRIES[self as usize]
    }
}

impl fmt::Display for FrameworkId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}
