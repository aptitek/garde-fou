//! Exemplary 6-domain starter templates (`Forme` tokens/CSS, `Texte` en/fr,
//! `Contenus` pure Markdown, and `Asset` sanitized SVG).

use gf_core::{FeatureId, FrameworkId, LanguageId, ProjectContext};

use super::{templates_infra, templates_python, templates_rust, templates_web};
use crate::types::DEFAULT_CONFIG_FILENAME;

const TOKENS_CSS: &str = "/* Design Tokens — single source of primitive visual values */\n\
:root {\n\
  --color-bg: #0f172a;\n\
  --color-fg: #f8fafc;\n\
  --color-accent: #38bdf8;\n\
  --space-4: 16px;\n\
  --radius-md: 8px;\n\
}\n";

const APP_CSS: &str = "/* Semantic component styles consuming exclusively Design Tokens */\n\
.app-shell {\n\
  background-color: var(--color-bg);\n\
  color: var(--color-fg);\n\
  padding: var(--space-4);\n\
  border-radius: var(--radius-md);\n\
}\n";

const LOGO_SVG: &str = "<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"0 0 24 24\" width=\"24\" height=\"24\">\n\
  <circle cx=\"12\" cy=\"12\" r=\"10\" fill=\"currentColor\" />\n\
</svg>\n";

const LOCALE_EN_JSON: &str = "{\n\
  \"app\": {\n\
    \"title\": \"Garde-fou Application\",\n\
    \"cta\": \"Explore documentation\",\n\
    \"logo_alt\": \"Garde-fou emblem\"\n\
  }\n\
}\n";

const LOCALE_FR_JSON: &str = "{\n\
  \"app\": {\n\
    \"title\": \"Application Garde-fou\",\n\
    \"cta\": \"Explorer la documentation\",\n\
    \"logo_alt\": \"Emblème Garde-fou\"\n\
  }\n\
}\n";

fn render_content_index_md(project_kebab: &str, with_logo: bool) -> String {
    let logo_line = if with_logo {
        "\n![Garde-fou emblem](../assets/logo.svg)\n"
    } else {
        ""
    };
    format!(
        "# {project_kebab}\n\
         \n\
         ## Architecture Overview\n\
         \n\
         This workspace enforces strict segregation across Code, Scripts, Forme, Texte, Contenus, and Asset domains.\n\
         {logo_line}"
    )
}

/// Collects all starter files `(relative_path, contents)` for `garde init`.
pub fn collect_all_init_files(
    project_name: &str,
    pkg_snake: &str,
    context: &ProjectContext,
) -> Vec<(String, String)> {
    let mut files = vec![
        (
            String::from(".gitattributes"),
            String::from(templates_infra::GITATTRIBUTES_TEMPLATE),
        ),
        (
            String::from(DEFAULT_CONFIG_FILENAME),
            templates_infra::render_garde_fou_toml(project_name, context),
        ),
    ];
    if context.languages.contains(&LanguageId::Rust) {
        files.extend(templates_rust::rust_files(project_name, pkg_snake));
    }
    if context.languages.contains(&LanguageId::TypeScript) {
        files.extend(templates_web::web_files(project_name, context));
    }
    if context.languages.contains(&LanguageId::Python) {
        files.extend(templates_python::python_files(
            project_name,
            pkg_snake,
            context,
        ));
    }
    files.extend(six_domain_feature_files(project_name, context));
    files.extend(templates_infra::infra_feature_files(project_name, context));
    files
}

fn six_domain_feature_files(project_kebab: &str, ctx: &ProjectContext) -> Vec<(String, String)> {
    let has_ui = ctx.features.contains(&FeatureId::Ui);
    let has_i18n = ctx.features.contains(&FeatureId::I18n);
    let has_content = ctx.features.contains(&FeatureId::Content);

    let mut files = Vec::new();
    if has_ui {
        files.push((String::from("styles/tokens.css"), String::from(TOKENS_CSS)));
        files.push((String::from("styles/app.css"), String::from(APP_CSS)));
    }
    if has_i18n {
        files.push((
            String::from("locales/en.json"),
            String::from(LOCALE_EN_JSON),
        ));
        files.push((
            String::from("locales/fr.json"),
            String::from(LOCALE_FR_JSON),
        ));
    }
    if has_ui || has_content {
        files.push((String::from("assets/logo.svg"), String::from(LOGO_SVG)));
    }
    if has_content {
        files.push((
            String::from("content/index.md"),
            render_content_index_md(project_kebab, has_ui || has_content),
        ));
    }
    files
}

/// Applies framework-to-language inference and default Rust fallback on `ctx`.
pub fn finalize_init_context(ctx: &mut ProjectContext) {
    for fw in &ctx.frameworks {
        let lang = match fw {
            FrameworkId::CargoWorkspace => LanguageId::Rust,
            FrameworkId::React | FrameworkId::Astro | FrameworkId::NextJs => LanguageId::TypeScript,
            FrameworkId::FastApi | FrameworkId::Django => LanguageId::Python,
        };
        ctx.languages.insert(lang);
    }
    if ctx.languages.is_empty() {
        ctx.languages.insert(LanguageId::Rust);
        ctx.features.insert(FeatureId::GitHooks);
    }
    if ctx.languages.contains(&LanguageId::Rust) {
        ctx.frameworks.insert(FrameworkId::CargoWorkspace);
    }
}
