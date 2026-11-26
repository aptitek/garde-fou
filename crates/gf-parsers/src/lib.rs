//! Context detectors, 6-domain extractors, and safe `Tree-sitter` -> [`UastDocument`] bridges.
//!
//! This crate performs pure syntactic and metric extraction on in-memory UTF-8 strings
//! without touching the filesystem, processes, or environment.

mod anti_cheat;
mod complexity;
mod cst;
mod fingerprint;
mod kiss_contracts;
mod kiss_yagni;
mod lang_detect;
pub mod langs;
mod secrets;
mod test_inspector;
mod test_inspector_py;
mod test_inspector_script;
pub mod tool_output;
pub mod tool_output_compiled;
pub mod tool_output_formats;
mod tool_output_gha;
mod tool_output_pssa;
pub mod tool_output_shell;
pub mod tool_output_sql;
mod tool_output_taplo_xml;
pub mod tool_output_ts;
pub mod tool_output_web;
pub mod tool_output_yaml;
pub mod tool_output_zpa;
pub mod types;

use gf_core::{
    AssetDocument, DomainKind, DomainLayoutConfig, Language, RepoPath, SqlDialectKind,
    UastDocument, classify_path, is_audited_tool_config_file,
};
pub use lang_detect::{detect_language, detect_language_or_shebang, detect_project_marker};
pub use langs::shell::dialect::detect_shebang_language;
pub use langs::ts::is_hook_identifier;
use langs::{
    asset::{collect_script_mentions, extract_svg_asset},
    astro::AstroExtractor,
    csharp::CSharpExtractor,
    css::CssExtractor,
    dockerfile::DockerfileExtractor,
    go::GoExtractor,
    html_w3c::HtmlExtractor,
    i18n_dict::extract_i18n_dictionary,
    kotlin::KotlinExtractor,
    markdown::{MarkdownExtractor, obsidian::JsonCanvasExtractor},
    notebook::NotebookExtractor,
    powershell::PowerShellExtractor,
    python::PythonExtractor,
    quarto::QuartoExtractor,
    rust::RustExtractor,
    shell::ShellExtractor,
    sql::{extract_plsql, extract_sql_with_dialect_hint},
    toml_lang::TomlExtractor,
    tool_config::extract_tool_config,
    ts::{TsDialect, TypeScriptExtractor},
    xml::XmlExtractor,
    yaml::YamlExtractor,
};
pub use {
    tool_output::{ToolParseError, parse_mypy_ndjson, parse_pyright_json, parse_ruff_json},
    tool_output_compiled::{
        parse_go_vet_json, parse_golangci_lint_json, parse_ktlint_json, parse_msbuild_diagnostics,
    },
    tool_output_formats::{
        parse_rumdl_or_markdownlint_json, parse_stylelint_json, parse_taplo_output,
        parse_xmllint_output,
    },
    tool_output_shell::{parse_hadolint_json, parse_psscriptanalyzer_json, parse_shellcheck_json},
    tool_output_sql::{parse_sqlfluff_json, parse_sqruff_json, parse_squawk_json},
    tool_output_ts::{parse_biome_json, parse_oxlint_json, parse_tsc_output},
    tool_output_web::{parse_astro_check_output, parse_html_validate_json},
    tool_output_yaml::{
        parse_actionlint_json, parse_dclint_json, parse_yamllint_parsable, parse_zizmor_sarif,
    },
    tool_output_zpa::parse_zpa_generic_issues_json,
    types::{LanguageExtractor, ParseError, ProjectMarker},
};

/// Builds a lightweight [`UastDocument`] for a binary asset without decoding file bytes.
///
/// # Errors
///
/// Returns [`ParseError`] if constructing the 1-line span fails.
pub fn build_binary_asset_document(
    path: &RepoPath,
    byte_size: u64,
) -> Result<UastDocument, ParseError> {
    let mut doc = UastDocument::new(path.clone(), Language::BinaryAsset, 1, 1)?;
    let extension = path.extension().unwrap_or("").to_ascii_lowercase();
    doc.asset = Some(AssetDocument {
        byte_size,
        extension,
        hazards: Vec::new(),
    });
    Ok(doc)
}

#[derive(Clone, Copy)]
struct DispatchCtx<'a> {
    domains: &'a DomainLayoutConfig,
    sql_dialect_hint: Option<SqlDialectKind>,
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
        Language::Quarto => {
            QuartoExtractor::extract_with_sql_dialect(path, source, ctx.sql_dialect_hint)
        }
        Language::JsonCanvas => JsonCanvasExtractor.extract(path, source),
        Language::Sql => extract_sql_with_dialect_hint(path, source, ctx.sql_dialect_hint),
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
        Language::Markdown => MarkdownExtractor.extract(path, source),
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

fn dispatch_language_extractor(
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

/// Parses `source` with explicit [`DomainLayoutConfig`] and optional workspace [`SqlDialectKind`] hint.
///
/// # Errors
///
/// Returns [`ParseError`] if grammar setup or span construction fails.
pub fn parse_source_with_context(
    path: &RepoPath,
    source: &str,
    domains: &DomainLayoutConfig,
    sql_dialect_hint: Option<SqlDialectKind>,
) -> Result<UastDocument, ParseError> {
    let lang = detect_language_or_shebang(path, source);
    let ctx = DispatchCtx {
        domains,
        sql_dialect_hint,
    };
    let mut doc = dispatch_language_extractor(lang, path, source, ctx)?;
    langs::toolchain_commands::maybe_audit_legacy_cli_commands(lang, path, source, &mut doc);
    doc.secret_findings = secrets::extract_secret_findings(path, source);
    if classify_path(path, domains) == DomainKind::Tests
        && (TsDialect::from_path_and_lang(path, lang).is_some()
            || matches!(
                lang,
                Language::Rust | Language::Python | Language::Shell | Language::PowerShell
            ))
    {
        doc.script_refs = collect_script_mentions(source);
    }
    Ok(doc)
}

/// Parses `source` with explicit [`DomainLayoutConfig`] (enabling `Texte` `.json`/`.toml` parsing).
/// # Errors
/// Returns [`ParseError`] if grammar setup or span construction fails.
pub fn parse_source_with_domains(
    path: &RepoPath,
    source: &str,
    domains: &DomainLayoutConfig,
) -> Result<UastDocument, ParseError> {
    parse_source_with_context(path, source, domains, None)
}

/// Parses `source` using strict default domain boundaries.
/// # Errors
/// Returns [`ParseError`] if grammar setup or span construction fails.
pub fn parse_source(path: &RepoPath, source: &str) -> Result<UastDocument, ParseError> {
    parse_source_with_domains(path, source, &DomainLayoutConfig::strict_defaults())
}
