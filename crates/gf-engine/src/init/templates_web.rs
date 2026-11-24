//! Strict TypeScript / Web (`React`, `Astro`, `Next.js`) scaffolding templates,
//! wired cleanly to `Forme`, `Texte` (`t(...)`), and `Asset` when 6-domain features are active.

use gf_core::{FeatureId, FrameworkId, ProjectContext};

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

fn render_ui_component(import_prefix: &str, project_kebab: &str, ctx: &ProjectContext) -> String {
    let has_i18n = ctx.features.contains(&FeatureId::I18n);
    let has_logo =
        ctx.features.contains(&FeatureId::Ui) || ctx.features.contains(&FeatureId::Content);
    if has_i18n {
        let logo_jsx = if has_logo {
            "\n      <img src=\"/assets/logo.svg\" alt={t(\"app.logo_alt\")} />"
        } else {
            ""
        };
        return format!(
            "import {{ formatTitle, t }} from \"{import_prefix}\";\n\
             \n\
             export function HomePage() {{\n\
             \x20 return (\n\
             \x20   <main className=\"app-shell\">{logo_jsx}\n\
             \x20     <h1>{{formatTitle(t(\"app.title\"))}}</h1>\n\
             \x20     <button type=\"button\">{{t(\"app.cta\")}}</button>\n\
             \x20   </main>\n\
             \x20 );\n\
             }}\n"
        );
    }
    let logo_jsx = if has_logo {
        "\n      <img src=\"/assets/logo.svg\" alt=\"Emblem\" />"
    } else {
        ""
    };
    format!(
        "import {{ formatTitle }} from \"{import_prefix}\";\n\
         \n\
         export function HomePage() {{\n\
         \x20 return (\n\
         \x20   <main className=\"app-shell\">{logo_jsx}\n\
         \x20     <h1>{{formatTitle(\"{project_kebab}\")}}</h1>\n\
         \x20   </main>\n\
         \x20 );\n\
         }}\n"
    )
}

fn render_index_and_test_ts(has_i18n: bool) -> (String, String) {
    let i18n_fn = if has_i18n {
        "\nexport function t(key: string): string {\n  return key;\n}\n"
    } else {
        ""
    };
    let index_ts = format!(
        "export function formatTitle(raw: string): string {{\n\
         \x20 const trimmed = raw.trim();\n\
         \x20 return trimmed.length > 0 ? trimmed : \"Garde-fou App\";\n\
         }}\n{i18n_fn}"
    );
    let i18n_test_import = if has_i18n { ", t" } else { "" };
    let i18n_test_call = if has_i18n {
        " && t(\"app.title\").length > 0 && t(\"app.cta\").length > 0 && t(\"app.logo_alt\").length > 0"
    } else {
        ""
    };
    let test_ts = format!(
        "import {{ formatTitle{i18n_test_import} }} from \"../src/index\";\n\
         \n\
         export function verifyFormatTitle(): boolean {{\n\
         \x20 return formatTitle(\"   Astro \") === \"Astro\"{i18n_test_call};\n\
         }}\n"
    );
    (index_ts, test_ts)
}

/// Generates non-destructive TypeScript starter files `(relative_path, contents)`.
pub fn web_files(project_kebab: &str, ctx: &ProjectContext) -> Vec<(String, String)> {
    let has_page_router = ctx.frameworks.contains(&FrameworkId::Astro)
        || ctx.frameworks.contains(&FrameworkId::NextJs);
    let has_react = ctx.frameworks.contains(&FrameworkId::React);
    let has_i18n = ctx.features.contains(&FeatureId::I18n);
    let uses_jsx =
        has_page_router || has_react || has_i18n || ctx.features.contains(&FeatureId::Ui);
    let (index_ts, test_ts) = render_index_and_test_ts(has_i18n);

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
        files.push((
            String::from("src/pages/index.tsx"),
            render_ui_component("../index", project_kebab, ctx),
        ));
    } else if uses_jsx {
        files.push((
            String::from("src/App.tsx"),
            render_ui_component("./index", project_kebab, ctx),
        ));
    }
    files
}
