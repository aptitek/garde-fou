//! Strict TypeScript / Web (`React`, `Astro`, `Next.js`, `Storybook`) scaffolding templates,
//! wired cleanly to `Forme`, `Texte` (`t(...)`), `Asset`, `Vitest`, and Atomic Design (`GF072`/`GF076`).

use gf_core::{FeatureId, FrameworkId, LanguageId, ProjectContext};

const PANDA_CONFIG_TS: &str = "export default {\n  preflight: true,\n  strictTokens: true,\n  strictPropertyValues: true,\n  include: [\"./src/**/*.{ts,tsx}\"],\n  outdir: \"styled-system\",\n};\n";
const DRIZZLE_CONFIG_TS: &str = "export default {\n  dialect: \"sqlite\",\n  schema: \"./src/index.ts\",\n  out: \"./migrations\",\n};\n";
const MIGRATION_INIT_SQL: &str = "CREATE TABLE IF NOT EXISTS app_items (\n  id INTEGER PRIMARY KEY NOT NULL,\n  title TEXT NOT NULL\n);\n";

fn render_wrangler_toml(project_kebab: &str) -> String {
    format!(
        "name = \"{project_kebab}\"\nmain = \"src/index.ts\"\ncompatibility_date = \"2026-03-01\"\ncompatibility_flags = [\"nodejs_compat\"]\n"
    )
}

fn render_astro_page(project_kebab: &str, ctx: &ProjectContext) -> String {
    let has_logo =
        ctx.features.contains(&FeatureId::Ui) || ctx.features.contains(&FeatureId::Content);
    let logo = if has_logo {
        "\n  <img src=\"/assets/logo.svg\" alt=\"Emblem\" />"
    } else {
        ""
    };
    format!(
        "---\nimport {{ formatTitle }} from \"../index\";\nconst heading = formatTitle(\"{project_kebab}\");\n---\n<main class=\"app-shell\">{logo}\n  <h1>{{heading}}</h1>\n</main>\n"
    )
}

#[rustfmt::skip]
fn dependencies_block(ctx: &ProjectContext) -> String {
    let mut deps = Vec::new();
    for (fw, pkg) in [
        (FrameworkId::Astro, "\"astro\": \"^5.0.0\""),
        (FrameworkId::NextJs, "\"next\": \"^15.0.0\""),
        (FrameworkId::ReactRouter, "\"react-router\": \"^7.2.0\""),
        (FrameworkId::CloudflareWorkers, "\"@cloudflare/workers-types\": \"^4.20260301.0\""),
        (FrameworkId::Drizzle, "\"drizzle-orm\": \"^0.40.0\""),
        (FrameworkId::Express, "\"express\": \"^5.0.1\""),
        (FrameworkId::Mongoose, "\"mongoose\": \"^8.10.0\""),
    ] {
        if ctx.frameworks.contains(&fw) { deps.push(pkg); }
    }
    if ctx.features.contains(&FeatureId::PandaCss) { deps.push("\"@pandacss/dev\": \"^0.51.0\""); }
    if [FrameworkId::React, FrameworkId::Astro, FrameworkId::NextJs, FrameworkId::ReactRouter].iter().any(|fw| ctx.frameworks.contains(fw)) {
        deps.push("\"react\": \"^19.0.0\"");
    }
    if deps.is_empty() { String::from("{}") } else { format!("{{\n    {}\n  }}", deps.join(",\n    ")) }
}

fn package_json(project_kebab: &str, ctx: &ProjectContext) -> String {
    let deps = dependencies_block(ctx);
    format!(
        "{{\n  \"name\": \"{project_kebab}\",\n  \"version\": \"0.1.0\",\n  \"private\": true,\n  \"type\": \"module\",\n  \"packageManager\": \"pnpm@10.6.5\",\n  \"dependencies\": {deps},\n  \"devDependencies\": {{\n    \"typescript\": \"^5.7.0\",\n    \"vitest\": \"^3.0.0\"\n  }}\n}}\n"
    )
}

#[rustfmt::skip]
fn tsconfig_json(uses_jsx: bool) -> String {
    let jsx_line = if uses_jsx { ",\n    \"jsx\": \"preserve\"" } else { "" };
    format!(
        "{{\n  \"compilerOptions\": {{\n    \"target\": \"ES2023\",\n    \"module\": \"ESNext\",\n    \"moduleResolution\": \"Bundler\",\n    \"strict\": true,\n    \"noUncheckedIndexedAccess\": true,\n    \"exactOptionalPropertyTypes\": true,\n    \"noImplicitOverride\": true,\n    \"isolatedModules\": true,\n    \"noEmit\": true{jsx_line}\n  }},\n  \"include\": [\"src\", \"tests\"]\n}}\n"
    )
}

fn render_i18n_ui_component(
    import_prefix: &str,
    atom_import: &str,
    ctx: &ProjectContext,
) -> String {
    let has_logo =
        ctx.features.contains(&FeatureId::Ui) || ctx.features.contains(&FeatureId::Content);
    let logo_jsx = if has_logo {
        "\n      <img src=\"/assets/logo.svg\" alt={t(\"app.logo_alt\")} />"
    } else {
        ""
    };
    let cta_jsx = if ctx.features.contains(&FeatureId::Storybook) {
        "<ActionButton label={t(\"app.cta\")} />"
    } else {
        "<button type=\"button\">{t(\"app.cta\")}</button>"
    };
    format!(
        "import {{ formatTitle, t }} from \"{import_prefix}\";\n\
         {atom_import}\n\
         export function HomePage() {{\n\
         \x20 return (\n\
         \x20   <main className=\"app-shell\">{logo_jsx}\n\
         \x20     <h1>{{formatTitle(t(\"app.title\"))}}</h1>\n\
         \x20     {cta_jsx}\n\
         \x20   </main>\n\
         \x20 );\n\
         }}\n"
    )
}

fn render_ui_component(
    import_prefix: &str,
    atom_prefix: &str,
    project_kebab: &str,
    ctx: &ProjectContext,
) -> String {
    let has_storybook = ctx.features.contains(&FeatureId::Storybook);
    let has_logo =
        ctx.features.contains(&FeatureId::Ui) || ctx.features.contains(&FeatureId::Content);
    let atom_import = if has_storybook {
        format!("import {{ ActionButton }} from \"{atom_prefix}/components/atoms/ActionButton\";\n")
    } else {
        String::new()
    };
    if ctx.features.contains(&FeatureId::I18n) {
        return render_i18n_ui_component(import_prefix, &atom_import, ctx);
    }
    let logo_jsx = if has_logo {
        "\n      <img src=\"/assets/logo.svg\" alt=\"Emblem\" />"
    } else {
        ""
    };
    let cta_jsx = if has_storybook {
        "\n      <ActionButton label=\"Explore\" />"
    } else {
        ""
    };
    format!(
        "import {{ formatTitle }} from \"{import_prefix}\";\n\
         {atom_import}\n\
         export function HomePage() {{\n\
         \x20 return (\n\
         \x20   <main className=\"app-shell\">{logo_jsx}\n\
         \x20     <h1>{{formatTitle(\"{project_kebab}\")}}</h1>{cta_jsx}\n\
         \x20   </main>\n\
         \x20 );\n\
         }}\n"
    )
}

fn render_index_ts(has_i18n: bool) -> String {
    let i18n_fn = if has_i18n {
        "\nexport function t(key: string): string {\n  return key;\n}\n"
    } else {
        ""
    };
    format!(
        "export function formatTitle(raw: string): string {{\n\
         \x20 const trimmed = raw.trim();\n\
         \x20 return trimmed.length > 0 ? trimmed : \"Garde-fou App\";\n\
         }}\n{i18n_fn}"
    )
}

fn render_test_ts(has_i18n: bool) -> String {
    if has_i18n {
        return String::from(
            "import { expect, test } from \"vitest\";\n\
             import { formatTitle, t } from \"../src/index\";\n\
             \n\
             test(\"trims surrounding whitespace and resolves i18n keys\", () => {\n\
             \x20 expect(formatTitle(\"   Astro \")).toBe(\"Astro\");\n\
             \x20 expect(formatTitle(\"   \")).toBe(\"Garde-fou App\");\n\
             \x20 expect(t(\"app.title\")).toBe(\"app.title\");\n\
             \x20 expect(t(\"app.cta\")).toBe(\"app.cta\");\n\
             \x20 expect(t(\"app.logo_alt\")).toBe(\"app.logo_alt\");\n\
             });\n",
        );
    }
    String::from(
        "import { expect, test } from \"vitest\";\n\
         import { formatTitle } from \"../src/index\";\n\
         \n\
         test(\"trims surrounding whitespace and falls back when blank\", () => {\n\
         \x20 expect(formatTitle(\"   Astro \")).toBe(\"Astro\");\n\
         \x20 expect(formatTitle(\"   \")).toBe(\"Garde-fou App\");\n\
         });\n",
    )
}

#[rustfmt::skip]
fn storybook_atom_files() -> [(String, String); 3] {
    let atom = "export interface ActionButtonProps {\n  readonly label: string;\n}\n\nexport function ActionButton({ label }: ActionButtonProps) {\n  return <button type=\"button\" className=\"app-shell\">{label}</button>;\n}\n";
    let story = "import { ActionButton, type ActionButtonProps } from \"./ActionButton\";\n\nexport default {\n  title: \"Atoms/ActionButton\",\n  component: ActionButton,\n};\n\nexport const Primary: { readonly args: ActionButtonProps } = {\n  args: { label: \"Primary\" },\n};\n";
    let test = "import { expect, test } from \"vitest\";\nimport { ActionButton } from \"./ActionButton\";\n\ntest(\"builds ActionButton element with explicit label\", () => {\n  const element = ActionButton({ label: \"Confirm\" });\n  expect(element.props.children).toBe(\"Confirm\");\n});\n";
    [
        (String::from("src/components/atoms/ActionButton.tsx"), String::from(atom)),
        (String::from("src/components/atoms/ActionButton.stories.tsx"), String::from(story)),
        (String::from("src/components/atoms/ActionButton.test.tsx"), String::from(test)),
    ]
}

#[rustfmt::skip]
fn append_edge_and_panda_files(project_kebab: &str, ctx: &ProjectContext, files: &mut Vec<(String, String)>) {
    if ctx.features.contains(&FeatureId::PandaCss) {
        files.push((String::from("panda.config.ts"), String::from(PANDA_CONFIG_TS)));
    }
    if ctx.frameworks.contains(&FrameworkId::CloudflareWorkers) {
        files.push((String::from("wrangler.toml"), render_wrangler_toml(project_kebab)));
    }
    if ctx.frameworks.contains(&FrameworkId::Drizzle) {
        files.push((String::from("drizzle.config.ts"), String::from(DRIZZLE_CONFIG_TS)));
    }
    if ctx.languages.contains(&LanguageId::Sql) {
        files.push((String::from("migrations/0001_init.sql"), String::from(MIGRATION_INIT_SQL)));
    }
}

#[rustfmt::skip]
fn ctx_uses_jsx(ctx: &ProjectContext) -> bool {
    [FrameworkId::Astro, FrameworkId::NextJs, FrameworkId::React, FrameworkId::ReactRouter]
        .iter()
        .any(|fw| ctx.frameworks.contains(fw))
        || [FeatureId::I18n, FeatureId::Storybook, FeatureId::Ui]
            .iter()
            .any(|f| ctx.features.contains(f))
}

/// Generates non-destructive TypeScript starter files `(relative_path, contents)`.
#[rustfmt::skip]
pub fn web_files(project_kebab: &str, ctx: &ProjectContext) -> Vec<(String, String)> {
    if super::templates_mern::is_mern_context(ctx) {
        let mut files = Vec::new();
        super::templates_mern::push_mern_templates(project_kebab, &mut files);
        return files;
    }
    let has_page_router = ctx.frameworks.contains(&FrameworkId::Astro)
        || ctx.frameworks.contains(&FrameworkId::NextJs);
    let has_i18n = ctx.features.contains(&FeatureId::I18n);
    let uses_jsx = ctx_uses_jsx(ctx);

    let mut files = vec![
        (String::from("package.json"), package_json(project_kebab, ctx)),
        (String::from("tsconfig.json"), tsconfig_json(uses_jsx)),
        (String::from("src/index.ts"), render_index_ts(has_i18n)),
        (String::from("tests/index.test.ts"), render_test_ts(has_i18n)),
    ];
    append_edge_and_panda_files(project_kebab, ctx, &mut files);
    if ctx.features.contains(&FeatureId::Storybook) {
        files.extend(storybook_atom_files());
    }
    if ctx.languages.contains(&LanguageId::Astro) {
        files.push((String::from("src/pages/index.astro"), render_astro_page(project_kebab, ctx)));
    } else if has_page_router {
        files.push((String::from("src/pages/index.tsx"), render_ui_component("../index", "..", project_kebab, ctx)));
    } else if uses_jsx {
        files.push((String::from("src/App.tsx"), render_ui_component("./index", ".", project_kebab, ctx)));
    }
    files
}
