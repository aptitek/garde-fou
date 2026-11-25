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
pub mod tool_output_formats;
mod tool_output_gha;
mod tool_output_pssa;
pub mod tool_output_shell;
mod tool_output_taplo_xml;
pub mod tool_output_ts;
pub mod tool_output_yaml;
pub mod types;

use gf_core::{
    AssetDocument, DomainKind, DomainLayoutConfig, Language, RepoPath, UastDocument, classify_path,
    is_audited_tool_config_file,
};
pub use lang_detect::{detect_language, detect_language_or_shebang, detect_project_marker};
pub use langs::shell::dialect::detect_shebang_language;
pub use langs::ts::is_hook_identifier;
use langs::{
    asset::{collect_script_mentions, extract_svg_asset},
    css::CssExtractor,
    dockerfile::DockerfileExtractor,
    i18n_dict::extract_i18n_dictionary,
    markdown::MarkdownExtractor,
    powershell::PowerShellExtractor,
    python::PythonExtractor,
    rust::RustExtractor,
    shell::ShellExtractor,
    toml_lang::TomlExtractor,
    tool_config::extract_tool_config,
    ts::{TsDialect, TypeScriptExtractor},
    xml::XmlExtractor,
    yaml::YamlExtractor,
};
pub use tool_output::{ToolParseError, parse_mypy_ndjson, parse_pyright_json, parse_ruff_json};
pub use tool_output_formats::{
    parse_rumdl_or_markdownlint_json, parse_stylelint_json, parse_taplo_output,
    parse_xmllint_output,
};
pub use tool_output_shell::{
    parse_hadolint_json, parse_psscriptanalyzer_json, parse_shellcheck_json,
};
pub use tool_output_ts::{parse_biome_json, parse_oxlint_json, parse_tsc_output};
pub use tool_output_yaml::{
    parse_actionlint_json, parse_dclint_json, parse_yamllint_parsable, parse_zizmor_sarif,
};
pub use types::{LanguageExtractor, ParseError, ProjectMarker};

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

fn dispatch_non_code_domain(
    lang: Language,
    path: &RepoPath,
    source: &str,
    domain: DomainKind,
) -> Result<UastDocument, ParseError> {
    match lang {
        Language::Css => CssExtractor.extract(path, source),
        Language::Markdown => MarkdownExtractor.extract(path, source),
        Language::Svg => extract_svg_asset(path, source),
        Language::Json | Language::Toml if domain == DomainKind::Texte => {
            extract_i18n_dictionary(path, lang, source)
        }
        Language::Toml => TomlExtractor.extract(path, source),
        Language::Xml => XmlExtractor.extract(path, source),
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

fn dispatch_language_extractor(
    lang: Language,
    path: &RepoPath,
    source: &str,
    domains: &DomainLayoutConfig,
) -> Result<UastDocument, ParseError> {
    if is_audited_tool_config_file(path) {
        return extract_tool_config(path, lang, source);
    }
    let domain = classify_path(path, domains);
    if let Some(dialect) = TsDialect::from_path_and_lang(path, lang) {
        return TypeScriptExtractor::new(dialect).extract_in_domain(path, source, domain);
    }
    match lang {
        Language::Rust => RustExtractor.extract(path, source),
        Language::Python => PythonExtractor::extract_in_domain(path, source, domain),
        Language::Shell => ShellExtractor.extract(path, source),
        Language::PowerShell => PowerShellExtractor.extract(path, source),
        Language::Dockerfile => DockerfileExtractor.extract(path, source),
        Language::Yaml => YamlExtractor.extract(path, source),
        other => dispatch_non_code_domain(other, path, source, domain),
    }
}

/// Parses `source` with explicit [`DomainLayoutConfig`] (enabling `Texte` `.json`/`.toml` parsing).
///
/// # Errors
///
/// Returns [`ParseError`] if grammar setup or span construction fails.
pub fn parse_source_with_domains(
    path: &RepoPath,
    source: &str,
    domains: &DomainLayoutConfig,
) -> Result<UastDocument, ParseError> {
    let lang = detect_language_or_shebang(path, source);
    let mut doc = dispatch_language_extractor(lang, path, source, domains)?;
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

/// Parses `source` using strict default domain boundaries.
///
/// # Errors
///
/// Returns [`ParseError`] if grammar setup or span construction fails.
pub fn parse_source(path: &RepoPath, source: &str) -> Result<UastDocument, ParseError> {
    parse_source_with_domains(path, source, &DomainLayoutConfig::strict_defaults())
}
