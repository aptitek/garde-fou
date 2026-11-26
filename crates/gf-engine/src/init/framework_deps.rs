//! Framework-to-language and feature implication table for `garde init`.

use gf_core::{FeatureId, FrameworkId, LanguageId, ProjectContext};

#[rustfmt::skip]
const fn implied_ext_lang_and_feat(fw: FrameworkId) -> Option<(LanguageId, Option<FeatureId>)> {
    match fw {
        FrameworkId::Symfony | FrameworkId::Doctrine => Some((LanguageId::Php, None)),
        FrameworkId::Flutter => Some((LanguageId::Dart, Some(FeatureId::Ui))),
        FrameworkId::MinecraftMod | FrameworkId::Kilt => Some((LanguageId::Kotlin, None)),
        FrameworkId::DataPack => Some((LanguageId::McFunction, None)),
        _ => None,
    }
}

#[rustfmt::skip]
const fn implied_lang_and_feat(fw: FrameworkId) -> (LanguageId, Option<FeatureId>) {
    if let Some(pair) = implied_ext_lang_and_feat(fw) { return pair; }
    match fw {
        FrameworkId::CargoWorkspace => (LanguageId::Rust, None),
        FrameworkId::FastApi | FrameworkId::Django | FrameworkId::Jupyter => (LanguageId::Python, None),
        FrameworkId::NiceGui => (LanguageId::Python, Some(FeatureId::Ui)),
        FrameworkId::PyTorch | FrameworkId::TensorFlow | FrameworkId::LangChain => (LanguageId::Python, Some(FeatureId::AiMl)),
        FrameworkId::DockerCompose => (LanguageId::Yaml, Some(FeatureId::Docker)),
        FrameworkId::Quarto | FrameworkId::Obsidian => (LanguageId::Markdown, None),
        FrameworkId::PostgreSql | FrameworkId::MariaDb => (LanguageId::Sql, None),
        _ => (LanguageId::TypeScript, None),
    }
}

/// Inserts languages and features implied by declared frameworks into `ctx`.
#[rustfmt::skip]
pub fn apply_framework_implied_deps(ctx: &mut ProjectContext) {
    for &fw in &ctx.frameworks {
        let (lang, feat) = implied_lang_and_feat(fw);
        ctx.languages.insert(lang);
        if fw == FrameworkId::Quarto { ctx.languages.insert(LanguageId::Yaml); }
        if let Some(f) = feat { ctx.features.insert(f); }
    }
}
