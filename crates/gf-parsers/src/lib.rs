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
pub mod langs;
mod secrets;
mod test_inspector;
mod test_inspector_py;
mod test_inspector_script;
pub mod tool_output;
pub mod tool_output_ts;
pub mod types;

use gf_core::{
    AssetDocument, DomainKind, DomainLayoutConfig, Language, RepoPath, UastDocument, classify_path,
    is_audited_tool_config_file, is_binary_asset_extension,
};
pub use langs::ts::is_hook_identifier;
use langs::{
    asset::{collect_script_mentions, extract_svg_asset},
    css::CssExtractor,
    dockerfile::DockerfileExtractor,
    i18n_dict::extract_i18n_dictionary,
    markdown::MarkdownExtractor,
    python::PythonExtractor,
    rust::RustExtractor,
    tool_config::extract_tool_config,
    ts::{TsDialect, TypeScriptExtractor},
};
pub use tool_output::{ToolParseError, parse_mypy_ndjson, parse_pyright_json, parse_ruff_json};
pub use tool_output_ts::{parse_biome_json, parse_oxlint_json, parse_tsc_output};
pub use types::{LanguageExtractor, ParseError, ProjectMarker};

const EXT_LANGUAGES: &[(&str, Language)] = &[
    ("rs", Language::Rust),
    ("py", Language::Python),
    ("ts", Language::TypeScript),
    ("mts", Language::TypeScript),
    ("cts", Language::TypeScript),
    ("tsx", Language::Tsx),
    ("js", Language::JavaScript),
    ("mjs", Language::JavaScript),
    ("cjs", Language::JavaScript),
    ("jsx", Language::Jsx),
    ("go", Language::Go),
    ("dockerfile", Language::Dockerfile),
    ("containerfile", Language::Dockerfile),
    ("css", Language::Css),
    ("scss", Language::Css),
    ("md", Language::Markdown),
    ("mdx", Language::Markdown),
    ("json", Language::Json),
    ("jsonc", Language::Json),
    ("toml", Language::Toml),
    ("sh", Language::Shell),
    ("bash", Language::Shell),
    ("ps1", Language::Shell),
    ("svg", Language::Svg),
];

fn is_dockerfile_name(file_name: &str) -> bool {
    let lower = file_name.to_ascii_lowercase();
    lower == "dockerfile"
        || lower == "containerfile"
        || lower.starts_with("dockerfile.")
        || lower.starts_with("containerfile.")
}

/// Infers the [`Language`] of a repository file from its name or extension.
#[must_use]
pub fn detect_language(path: &RepoPath) -> Language {
    let file_name = path.file_name();
    if file_name.eq_ignore_ascii_case("dockerfile")
        || file_name.eq_ignore_ascii_case("containerfile")
    {
        return Language::Dockerfile;
    }
    if let Some(ext) = path.extension() {
        if let Some((_, lang)) = EXT_LANGUAGES
            .iter()
            .find(|(candidate, _)| candidate.eq_ignore_ascii_case(ext))
        {
            return *lang;
        }
        if is_binary_asset_extension(ext) {
            return Language::BinaryAsset;
        }
    }
    if is_dockerfile_name(file_name) {
        return Language::Dockerfile;
    }
    Language::Unknown
}

/// Detects well-known project ecosystem manifest files from a [`RepoPath`].
#[must_use]
pub fn detect_project_marker(path: &RepoPath) -> Option<ProjectMarker> {
    if detect_language(path) == Language::Dockerfile {
        return Some(ProjectMarker::Containerfile);
    }
    match path.file_name() {
        "Cargo.toml" => Some(ProjectMarker::CargoManifest),
        "package.json" => Some(ProjectMarker::NodeManifest),
        "go.mod" => Some(ProjectMarker::GoMod),
        _ => None,
    }
}

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
        Language::Dockerfile => DockerfileExtractor.extract(path, source),
        Language::Css => CssExtractor.extract(path, source),
        Language::Markdown => MarkdownExtractor.extract(path, source),
        Language::Svg => extract_svg_asset(path, source),
        Language::Json | Language::Toml if domain == DomainKind::Texte => {
            extract_i18n_dictionary(path, lang, source)
        }
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
    let lang = detect_language(path);
    let mut doc = dispatch_language_extractor(lang, path, source, domains)?;
    if lang == Language::Shell {
        anti_cheat::scan_hash_line_comments(source, &mut doc);
    }
    doc.secret_findings = secrets::extract_secret_findings(path, source);
    if classify_path(path, domains) == DomainKind::Tests
        && (TsDialect::from_path_and_lang(path, lang).is_some()
            || matches!(lang, Language::Rust | Language::Python))
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
