//! Exemplary 6-domain starter templates (`Forme` tokens/CSS, `Texte` en/fr,
//! `Contenus` pure Markdown, and `Asset` sanitized SVG).

use gf_core::{FeatureId, FrameworkId, LanguageId, ProjectContext, WorkspaceArchetype};

use super::{
    templates_archetypes, templates_infra, templates_python, templates_rust, templates_web,
};
use crate::types::DEFAULT_CONFIG_FILENAME;

const TOKENS_CSS: &str = "/* Design Tokens — single source of primitive visual values */\n\
:root {\n\
  --color-bg: #0f172a;\n\
  --color-fg: #f8fafc;\n\
  --color-accent: #38bdf8;\n\
  --space-4: 16px;\n\
  --radius-md: 8px;\n\
}\n";

const TOKENS_M3E_CSS: &str = "/* Material Design 3 Expressive Tokens (md.sys.color.primaryContainer / onSurfaceVariant) */\n\
:root {\n\
  --color-bg: #0f172a;\n\
  --color-fg: #f8fafc;\n\
  --color-accent: #38bdf8;\n\
  --md-sys-color-primary-container: #d3e4ff;\n\
  --md-sys-color-on-surface-variant: #44474e;\n\
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
    if context.archetype == WorkspaceArchetype::Software {
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
        super::templates_compiled::push_compiled_lang_templates(
            project_name,
            pkg_snake,
            context,
            &mut files,
        );
        files.extend(six_domain_feature_files(project_name, context));
    } else {
        files.extend(templates_archetypes::archetype_starter_files(context));
    }
    files.extend(templates_infra::infra_feature_files(project_name, context));
    files
}

fn six_domain_feature_files(project_kebab: &str, ctx: &ProjectContext) -> Vec<(String, String)> {
    let has_ui = ctx.features.contains(&FeatureId::Ui)
        || ctx.features.contains(&FeatureId::Material3Expressive);
    let has_m3e = ctx.features.contains(&FeatureId::Material3Expressive);
    let has_i18n = ctx.features.contains(&FeatureId::I18n);
    let has_content = ctx.features.contains(&FeatureId::Content);

    let mut files = Vec::new();
    if has_ui {
        let tok_css = if has_m3e { TOKENS_M3E_CSS } else { TOKENS_CSS };
        files.push((String::from("styles/tokens.css"), String::from(tok_css)));
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

fn infer_feature_languages(ctx: &mut ProjectContext) {
    ctx.languages.insert(LanguageId::Toml);
    let has_yaml_feat = [
        FeatureId::GitHooks,
        FeatureId::TaskRunner,
        FeatureId::CiGithub,
    ]
    .iter()
    .any(|f| ctx.features.contains(f));
    if has_yaml_feat {
        ctx.languages.insert(LanguageId::Yaml);
    }
    if ctx.features.contains(&FeatureId::Ui)
        || ctx.features.contains(&FeatureId::Material3Expressive)
    {
        ctx.languages.insert(LanguageId::Css);
        ctx.languages.insert(LanguageId::Xml);
    }
    if ctx.features.contains(&FeatureId::Content) {
        ctx.languages.insert(LanguageId::Xml);
        ctx.languages.insert(LanguageId::Markdown);
    }
    if ctx.languages.contains(&LanguageId::Astro) {
        ctx.languages.insert(LanguageId::TypeScript);
        ctx.features.insert(FeatureId::W3c);
        ctx.frameworks.insert(FrameworkId::Astro);
    }
}

#[rustfmt::skip]
fn has_software_lang(ctx: &ProjectContext) -> bool {
    [LanguageId::Rust, LanguageId::TypeScript, LanguageId::Python, LanguageId::Astro, LanguageId::PlSql]
        .iter()
        .any(|l| ctx.languages.contains(l))
}

#[rustfmt::skip]
fn has_script_only_lang(ctx: &ProjectContext) -> bool {
    (ctx.languages.contains(&LanguageId::Shell) || ctx.languages.contains(&LanguageId::PowerShell))
        && !has_software_lang(ctx)
}

#[rustfmt::skip]
fn infer_init_archetype(ctx: &ProjectContext) -> WorkspaceArchetype {
    if ctx.frameworks.contains(&FrameworkId::Quarto) || ctx.frameworks.contains(&FrameworkId::Jupyter) {
        return WorkspaceArchetype::Academic;
    }
    if ctx.frameworks.contains(&FrameworkId::Obsidian) { return WorkspaceArchetype::Knowledge; }
    let has_sw = has_software_lang(ctx);
    if ctx.frameworks.contains(&FrameworkId::DockerCompose) || (ctx.features.contains(&FeatureId::Docker) && !has_sw) {
        return WorkspaceArchetype::Infra;
    }
    if has_script_only_lang(ctx) { return WorkspaceArchetype::Scripts; }
    if has_sw || !ctx.frameworks.is_empty() { WorkspaceArchetype::Software } else { WorkspaceArchetype::Minimal }
}

/// Applies proportional framework-to-language inference and archetype resolution on `ctx`.
#[rustfmt::skip]
pub fn finalize_init_context(ctx: &mut ProjectContext, explicit_archetype: Option<WorkspaceArchetype>) {
    super::framework_deps::apply_framework_implied_deps(ctx);
    ctx.features.insert(FeatureId::GitHooks);
    ctx.archetype = explicit_archetype.unwrap_or_else(|| infer_init_archetype(ctx));
    if ctx.archetype == WorkspaceArchetype::Software && ctx.languages.contains(&LanguageId::Rust) {
        ctx.frameworks.insert(FrameworkId::CargoWorkspace);
    }
    if ctx.archetype == WorkspaceArchetype::Infra {
        ctx.features.insert(FeatureId::Docker);
        ctx.frameworks.insert(FrameworkId::DockerCompose);
    }
    if ctx.archetype != WorkspaceArchetype::Minimal { infer_feature_languages(ctx); }
}
