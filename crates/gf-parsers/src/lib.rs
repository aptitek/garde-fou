//! Context detectors, 6-domain extractors, and safe `Tree-sitter` -> [`UastDocument`] bridges.
//!
//! This crate performs pure syntactic and metric extraction on in-memory UTF-8 strings
//! without touching the filesystem, processes, or environment.

mod anti_cheat;
mod complexity;
mod cst;
mod dispatch;
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
pub mod tool_output_taplo_xml;
pub mod tool_output_ts;
pub mod tool_output_web;
pub mod tool_output_yaml;
pub mod tool_output_zpa;
pub mod types;

use dispatch::{DispatchCtx, dispatch_language_extractor};
use gf_core::{
    AssetDocument, DomainKind, DomainLayoutConfig, Language, RepoPath, SqlDialectKind,
    UastDocument, classify_path,
};
pub use lang_detect::{detect_language, detect_language_or_shebang, detect_project_marker};
pub use langs::shell::dialect::detect_shebang_language;
pub use langs::ts::is_hook_identifier;
use langs::{asset::collect_script_mentions, ts::TsDialect};
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
