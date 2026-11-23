//! Strict TypeScript / Web (`React`, `Astro`, `Next.js`) scaffolding templates.

use gf_core::{FrameworkId, ProjectContext};

fn dependencies_block(ctx: &ProjectContext) -> String {
    let mut deps = Vec::new();
    if ctx.frameworks.contains(&FrameworkId::Astro) {
        deps.push("\"astro\": \"^5.0.0\"");
    }
    if ctx.frameworks.contains(&FrameworkId::NextJs) {
        deps.push("\"next\": \"^15.0.0\"");
    }
    if ctx.frameworks.contains(&FrameworkId::React)
        || ctx.frameworks.contains(&FrameworkId::Astro)
        || ctx.frameworks.contains(&FrameworkId::NextJs)
    {
        deps.push("\"react\": \"^19.0.0\"");
    }
    if deps.is_empty() {
        String::from("{}")
    } else {
        format!("{{\n    {}\n  }}", deps.join(",\n    "))
    }
}

fn package_json(project_kebab: &str, ctx: &ProjectContext) -> String {
    let deps = dependencies_block(ctx);
    format!(
        "{{\n\
         \x20 \"name\": \"{project_kebab}\",\n\
         \x20 \"version\": \"0.1.0\",\n\
         \x20 \"private\": true,\n\
         \x20 \"type\": \"module\",\n\
         \x20 \"dependencies\": {deps},\n\
         \x20 \"devDependencies\": {{\n\
         \x20   \"typescript\": \"^5.7.0\"\n\
         \x20 }}\n\
         }}\n"
    )
}

fn tsconfig_json(uses_jsx: bool) -> String {
    let jsx_line = if uses_jsx {
        ",\n    \"jsx\": \"preserve\""
    } else {
        ""
    };
    format!(
        "{{\n\
         \x20 \"compilerOptions\": {{\n\
         \x20   \"target\": \"ES2023\",\n\
         \x20   \"module\": \"ESNext\",\n\
         \x20   \"moduleResolution\": \"Bundler\",\n\
         \x20   \"strict\": true,\n\
         \x20   \"noUncheckedIndexedAccess\": true,\n\
         \x20   \"exactOptionalPropertyTypes\": true,\n\
         \x20   \"noImplicitOverride\": true,\n\
         \x20   \"isolatedModules\": true,\n\
         \x20   \"noEmit\": true{jsx_line}\n\
         \x20 }},\n\
         \x20 \"include\": [\"src\", \"tests\"]\n\
         }}\n"
    )
}

/// Generates non-destructive TypeScript starter files `(relative_path, contents)`.
pub fn web_files(project_kebab: &str, ctx: &ProjectContext) -> Vec<(String, String)> {
    let has_page_router = ctx.frameworks.contains(&FrameworkId::Astro)
        || ctx.frameworks.contains(&FrameworkId::NextJs);
    let uses_jsx = has_page_router || ctx.frameworks.contains(&FrameworkId::React);

    let index_ts = String::from(
        "export function formatTitle(raw: string): string {\n\
         \x20 const trimmed = raw.trim();\n\
         \x20 return trimmed.length > 0 ? trimmed : \"Garde-fou App\";\n\
         }\n",
    );
    let test_ts = String::from(
        "import { formatTitle } from \"../src/index\";\n\
         \n\
         export function verifyFormatTitle(): boolean {\n\
         \x20 return formatTitle(\"   Astro \") === \"Astro\" && formatTitle(\"\") === \"Garde-fou App\";\n\
         }\n",
    );

    let mut files = vec![
        (
            String::from("package.json"),
            package_json(project_kebab, ctx),
        ),
        (String::from("tsconfig.json"), tsconfig_json(uses_jsx)),
        (String::from("src/index.ts"), index_ts),
        (String::from("tests/index.test.ts"), test_ts),
    ];

    if has_page_router {
        let page_tsx = format!(
            "import {{ formatTitle }} from \"../index\";\n\
             \n\
             export function HomePage() {{\n\
             \x20 return <main><h1>{{formatTitle(\"{project_kebab}\")}}</h1></main>;\n\
             }}\n"
        );
        files.push((String::from("src/pages/index.tsx"), page_tsx));
    }
    files
}
