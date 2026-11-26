//! Language-to-extractor dispatch helpers for [`crate::parse_source_with_context`].

use gf_core::{
    DomainKind, DomainLayoutConfig, Language, RepoPath, SqlDialectKind, UastDocument,
    classify_path, is_audited_tool_config_file,
};

use crate::cst;
use crate::langs::{
    self,
    asset::extract_svg_asset,
    astro::AstroExtractor,
    csharp::CSharpExtractor,
    css::CssExtractor,
    dockerfile::DockerfileExtractor,
    go::GoExtractor,
    html_w3c::HtmlExtractor,
    i18n_dict::extract_i18n_dictionary,
    kotlin::KotlinExtractor,
    markdown::obsidian::JsonCanvasExtractor,
    notebook::NotebookExtractor,
    powershell::PowerShellExtractor,
    python::PythonExtractor,
    rust::RustExtractor,
    shell::ShellExtractor,
    sql::{extract_plsql, extract_sql_with_dialect_hint},
    toml_lang::TomlExtractor,
    tool_config::extract_tool_config,
    ts::{TsDialect, TypeScriptExtractor},
    xml::XmlExtractor,
    yaml::YamlExtractor,
};
use crate::types::{LanguageExtractor, ParseError};

#[derive(Clone, Copy)]
pub struct DispatchCtx<'a> {
    pub domains: &'a DomainLayoutConfig,
    pub sql_dialect_hint: Option<SqlDialectKind>,
}

fn dispatch_academic_or_text(
    lang: Language,
    path: &RepoPath,
    source: &str,
    ctx: DispatchCtx<'_>,
) -> Result<UastDocument, ParseError> {
    match lang {
        Language::Notebook => {
            NotebookExtractor::extract_with_sql_dialect(path, source, ctx.sql_dialect_hint)
        }
        Language::Quarto => langs::diagrams::fenced_bridge::extract_quarto_with_diagrams(
            path,
            source,
            ctx.sql_dialect_hint,
        ),
        Language::JsonCanvas => JsonCanvasExtractor.extract(path, source),
        Language::Sql => extract_sql_with_dialect_hint(path, source, ctx.sql_dialect_hint),
        Language::Mermaid => langs::diagrams::extract_mermaid(path, source),
        Language::PlantUml => langs::diagrams::extract_plantuml(path, source),
        Language::GettextPo => langs::i18n_formats::extract_gettext_po(path, source),
        Language::FluentFtl => langs::i18n_formats::extract_fluent_ftl(path, source),
        other => {
            let (total_lines, effective_lines) = cst::line_metrics(source);
            Ok(UastDocument::new(
                path.clone(),
                other,
                total_lines,
                effective_lines,
            )?)
        }
    }
}

fn dispatch_non_code_domain(
    lang: Language,
    path: &RepoPath,
    source: &str,
    ctx: DispatchCtx<'_>,
) -> Result<UastDocument, ParseError> {
    let domain = classify_path(path, ctx.domains);
    match lang {
        Language::Css => CssExtractor.extract(path, source),
        Language::Html => HtmlExtractor.extract(path, source),
        Language::Markdown => {
            langs::diagrams::fenced_bridge::extract_markdown_with_diagrams(path, source)
        }
        Language::Svg => extract_svg_asset(path, source),
        Language::Json | Language::Toml if domain == DomainKind::Texte => {
            extract_i18n_dictionary(path, lang, source)
        }
        Language::Toml => TomlExtractor.extract(path, source),
        Language::Xml => XmlExtractor.extract(path, source),
        other => dispatch_academic_or_text(other, path, source, ctx),
    }
}

fn dispatch_script_or_compiled(
    lang: Language,
    path: &RepoPath,
    source: &str,
    ctx: DispatchCtx<'_>,
) -> Result<UastDocument, ParseError> {
    match lang {
        Language::Go => GoExtractor.extract(path, source),
        Language::Kotlin => KotlinExtractor.extract(path, source),
        Language::CSharp => CSharpExtractor.extract(path, source),
        Language::Shell => ShellExtractor.extract(path, source),
        Language::PowerShell => PowerShellExtractor.extract(path, source),
        Language::Dockerfile => DockerfileExtractor.extract(path, source),
        Language::Yaml => YamlExtractor.extract(path, source),
        Language::Astro => AstroExtractor.extract(path, source),
        Language::PlSql => extract_plsql(path, source),
        other => dispatch_non_code_domain(other, path, source, ctx),
    }
}

pub fn dispatch_language_extractor(
    lang: Language,
    path: &RepoPath,
    source: &str,
    ctx: DispatchCtx<'_>,
) -> Result<UastDocument, ParseError> {
    if is_audited_tool_config_file(path) {
        if lang == Language::Xml {
            let mut doc = XmlExtractor.extract(path, source)?;
            langs::tool_config_compiled::audit_compiled_tool_config(path, source, &mut doc);
            return Ok(doc);
        }
        return extract_tool_config(path, lang, source);
    }
    let domain = classify_path(path, ctx.domains);
    if let Some(dialect) = TsDialect::from_path_and_lang(path, lang) {
        return TypeScriptExtractor::new(dialect)
            .with_sql_dialect(ctx.sql_dialect_hint)
            .extract_in_domain(path, source, domain);
    }
    match lang {
        Language::Rust => RustExtractor.extract(path, source),
        Language::Python => PythonExtractor::extract_in_domain(path, source, domain),
        other => dispatch_script_or_compiled(other, path, source, ctx),
    }
}
