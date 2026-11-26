//! Starter `pnpm-workspace.yaml` monorepo templates for `garde init --framework mern`
//! (`apps/api` Express 5 + Mongoose, `apps/web` React TS, `packages/contracts`).

use gf_core::{FrameworkId, ProjectContext};

const PNPM_WORKSPACE: &str = "packages:\n  - \"apps/*\"\n  - \"packages/*\"\n";

const CONTRACTS_PKG: &str = "{\n  \"name\": \"@app/contracts\",\n  \"version\": \"0.1.0\",\n  \"private\": true,\n  \"type\": \"module\"\n}\n";

const CONTRACTS_INDEX: &str =
    "export interface HealthResponse {\n  status: \"ok\";\n  service: string;\n}\n";

const API_PKG: &str = "{\n  \"name\": \"@app/api\",\n  \"version\": \"0.1.0\",\n  \"private\": true,\n  \"type\": \"module\",\n  \"dependencies\": {\n    \"express\": \"^5.0.1\",\n    \"mongoose\": \"^8.10.0\"\n  }\n}\n";

const API_APP_TS: &str = "import type { HealthResponse } from \"../../../packages/contracts/src/index\";\n\nexport function createHealthPayload(service: string): HealthResponse {\n  return { status: \"ok\", service };\n}\n";

const API_SERVER_TS: &str = "import { createHealthPayload } from \"./app\";\n\nexport function startServer(): string {\n  const payload = createHealthPayload(\"api\");\n  return `${payload.service}:${payload.status}`;\n}\n";

const API_APP_TEST: &str = "import { describe, expect, it } from \"vitest\";\nimport { createHealthPayload } from \"../src/app\";\n\ndescribe(\"createHealthPayload\", () => {\n  it(\"returns ok status\", () => {\n    expect(createHealthPayload(\"api\").status).toBe(\"ok\");\n  });\n});\n";

const WEB_PKG: &str = "{\n  \"name\": \"@app/web\",\n  \"version\": \"0.1.0\",\n  \"private\": true,\n  \"type\": \"module\",\n  \"dependencies\": {\n    \"react\": \"^19.0.0\"\n  }\n}\n";

const WEB_APP_TSX: &str = "import type { HealthResponse } from \"../../../packages/contracts/src/index\";\n\nexport function App({ health }: { health: HealthResponse }) {\n  return (\n    <main>\n      <h1>{health.service}</h1>\n      <p>{health.status}</p>\n    </main>\n  );\n}\n";

const TSCONFIG_MERN: &str = "{\n  \"compilerOptions\": {\n    \"target\": \"ES2023\",\n    \"module\": \"ESNext\",\n    \"moduleResolution\": \"Bundler\",\n    \"strict\": true,\n    \"noUncheckedIndexedAccess\": true,\n    \"exactOptionalPropertyTypes\": true,\n    \"noImplicitOverride\": true,\n    \"isolatedModules\": true,\n    \"noEmit\": true,\n    \"jsx\": \"preserve\"\n  },\n  \"include\": [\"apps\", \"packages\"]\n}\n";

/// Returns `true` when `ctx` requests a fullstack MERN workspace (`React` + `Express`/`Mongoose`).
#[must_use]
pub fn is_mern_context(ctx: &ProjectContext) -> bool {
    ctx.frameworks.contains(&FrameworkId::React)
        && (ctx.frameworks.contains(&FrameworkId::Express)
            || ctx.frameworks.contains(&FrameworkId::Mongoose))
}

/// Appends a clean, zero-violation MERN `pnpm-workspace.yaml` monorepo scaffold.
pub fn push_mern_templates(project_kebab: &str, files: &mut Vec<(String, String)>) {
    let root_pkg = format!(
        "{{\n  \"name\": \"{project_kebab}\",\n  \"version\": \"0.1.0\",\n  \"private\": true,\n  \"type\": \"module\",\n  \"packageManager\": \"pnpm@10.6.5\",\n  \"dependencies\": {{\n    \"express\": \"^5.0.1\",\n    \"mongoose\": \"^8.10.0\",\n    \"react\": \"^19.0.0\"\n  }},\n  \"devDependencies\": {{\n    \"@types/node\": \"^22.0.0\",\n    \"typescript\": \"^5.7.0\",\n    \"vitest\": \"^3.0.0\"\n  }}\n}}\n"
    );
    files.extend([
        (
            String::from("pnpm-workspace.yaml"),
            String::from(PNPM_WORKSPACE),
        ),
        (String::from("package.json"), root_pkg),
        (String::from("tsconfig.json"), String::from(TSCONFIG_MERN)),
        (
            String::from("packages/contracts/package.json"),
            String::from(CONTRACTS_PKG),
        ),
        (
            String::from("packages/contracts/src/index.ts"),
            String::from(CONTRACTS_INDEX),
        ),
        (String::from("apps/api/package.json"), String::from(API_PKG)),
        (
            String::from("apps/api/src/app.ts"),
            String::from(API_APP_TS),
        ),
        (
            String::from("apps/api/src/server.ts"),
            String::from(API_SERVER_TS),
        ),
        (
            String::from("apps/api/tests/app.test.ts"),
            String::from(API_APP_TEST),
        ),
        (String::from("apps/web/package.json"), String::from(WEB_PKG)),
        (
            String::from("apps/web/src/App.tsx"),
            String::from(WEB_APP_TSX),
        ),
    ]);
}
