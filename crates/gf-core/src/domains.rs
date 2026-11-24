//! Six-domain architectural taxonomy (`Code`, `Scripts`, `Forme`, `Texte`, `Contenus`, `Asset`)
//! plus `Tests` and `Meta` classification.

use alloc::format;
use alloc::string::String;
use alloc::vec;
use alloc::vec::Vec;
use serde::{Deserialize, Serialize};

use crate::glob::glob_matches;
use crate::path::RepoPath;

/// Canonical 1 MiB default size ceiling for static assets (`GF064`).
pub const DEFAULT_MAX_ASSET_BYTES: u64 = 1_048_576;

const TEST_GLOBS: &[&str] = &[
    "tests/**",
    "**/tests/**",
    "*.test.*",
    "**/*.test.*",
    "*.spec.*",
    "**/*.spec.*",
    "*_test.*",
    "**/*_test.*",
    "test_*.py",
    "**/test_*.py",
];

const ASSET_EXTENSIONS: &[&str] = &[
    "png", "jpg", "jpeg", "gif", "webp", "avif", "ico", "svg", "woff", "woff2", "ttf", "otf",
    "mp3", "mp4", "webm", "pdf",
];

/// Architectural domain assigned to a repository file.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
pub enum DomainKind {
    /// Pure business logic and semantic UI components (`src/**`, `crates/**`, `app/**`).
    Code,
    /// Standalone automation and developer helpers (`scripts/**`, `tools/**`).
    Scripts,
    /// Styling sheets, themes, and Design Tokens (`styles/**`, `tokens/**`, `panda.config.*`).
    Forme,
    /// Internationalization dictionaries (`locales/**`, `i18n/**`, `messages/**`).
    Texte,
    /// Pure editorial Markdown/MDX content (`content/**`, `courses/**`, `docs/**`).
    Contenus,
    /// Confined static media, icons, and fonts (`assets/**`, `public/**`, `static/**`).
    Asset,
    /// Automated test suites (`tests/**`, `*.test.*`, `*.spec.*`).
    Tests,
    /// Root manifests, CI workflows, and governance documents (`Cargo.toml`, `README.md`, etc.).
    Meta,
}

impl DomainKind {
    /// Canonical lowercase label.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Code => "code",
            Self::Scripts => "scripts",
            Self::Forme => "forme",
            Self::Texte => "texte",
            Self::Contenus => "contenus",
            Self::Asset => "asset",
            Self::Tests => "tests",
            Self::Meta => "meta",
        }
    }
}

/// Configurable glob layout for the 6 domains (`[domains]` in `garde-fou.toml`).
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DomainLayoutConfig {
    /// Globs matching `DomainKind::Code`.
    pub code_globs: Vec<String>,
    /// Globs matching `DomainKind::Scripts`.
    pub scripts_globs: Vec<String>,
    /// Globs matching `DomainKind::Forme`.
    pub forme_globs: Vec<String>,
    /// Subset of `Forme` globs permitted to define raw design primitives (`#hex`, `px`, `oklch`).
    pub forme_token_globs: Vec<String>,
    /// Globs matching `DomainKind::Texte` (`i18n` dictionaries).
    pub texte_globs: Vec<String>,
    /// Globs matching `DomainKind::Contenus` (editorial Markdown/MDX).
    pub contenus_globs: Vec<String>,
    /// Globs matching `DomainKind::Asset` (static assets).
    pub asset_globs: Vec<String>,
    /// Maximum allowed size in bytes for a single static asset (`GF064`).
    pub max_asset_bytes: u64,
}

fn owned_strs(items: &[&str]) -> Vec<String> {
    items.iter().map(|&s| String::from(s)).collect()
}

impl DomainLayoutConfig {
    /// Returns strict default 6-domain glob boundaries.
    #[must_use]
    pub fn strict_defaults() -> Self {
        Self {
            code_globs: owned_strs(&["src/**", "crates/**", "app/**", "lib/**", "packages/**"]),
            scripts_globs: owned_strs(&["scripts/**", "tools/**"]),
            forme_globs: owned_strs(&[
                "styles/**",
                "tokens/**",
                "theme/**",
                "design-system/**",
                "src/styles/**",
                "src/tokens/**",
                "panda.config.*",
            ]),
            forme_token_globs: owned_strs(&[
                "tokens/**",
                "styles/tokens/**",
                "src/tokens/**",
                "**/tokens.*",
                "**/variables.css",
                "panda.config.*",
            ]),
            texte_globs: owned_strs(&["locales/**", "i18n/**", "messages/**", "src/locales/**"]),
            contenus_globs: owned_strs(&["content/**", "courses/**", "docs/**", "src/content/**"]),
            asset_globs: owned_strs(&["assets/**", "public/**", "static/**", "src/assets/**"]),
            max_asset_bytes: DEFAULT_MAX_ASSET_BYTES,
        }
    }

    /// Deterministic canonical string fragment included in `BLAKE3` cache keys.
    #[must_use]
    pub fn canonical_fragment(&self) -> String {
        format!(
            "dc={}|ds={}|df={}|dft={}|dt={}|dco={}|da={}|mab={}",
            self.code_globs.join(","),
            self.scripts_globs.join(","),
            self.forme_globs.join(","),
            self.forme_token_globs.join(","),
            self.texte_globs.join(","),
            self.contenus_globs.join(","),
            self.asset_globs.join(","),
            self.max_asset_bytes,
        )
    }
}

impl Default for DomainLayoutConfig {
    fn default() -> Self {
        Self::strict_defaults()
    }
}

fn matches_any(globs: &[String], candidate: &str) -> bool {
    globs.iter().any(|pat| glob_matches(pat, candidate))
}

/// Returns `true` when `path` is a permitted root governance Markdown/license file.
#[must_use]
pub fn is_root_governance_doc(path: &RepoPath) -> bool {
    if path.as_str().contains('/') {
        return false;
    }
    let upper = path.file_name().to_ascii_uppercase();
    matches!(
        upper.as_str(),
        "README.MD" | "AGENTS.MD" | "CHANGELOG.MD" | "CONTRIBUTING.MD" | "SECURITY.MD"
    ) || upper.starts_with("LICENSE")
}

/// Returns `true` if `ext` (case-insensitive, no leading dot) is a static asset extension.
#[must_use]
pub fn is_asset_extension(ext: &str) -> bool {
    let lower = ext.to_ascii_lowercase();
    ASSET_EXTENSIONS.contains(&lower.as_str())
}

/// Returns `true` if `ext` is a binary static asset extension (all assets except UTF-8 `.svg`).
#[must_use]
pub fn is_binary_asset_extension(ext: &str) -> bool {
    !ext.eq_ignore_ascii_case("svg") && is_asset_extension(ext)
}

/// Returns `true` if `path` matches `layout.forme_token_globs` (allowed raw design tokens).
#[must_use]
pub fn is_forme_token_path(path: &RepoPath, layout: &DomainLayoutConfig) -> bool {
    matches_any(&layout.forme_token_globs, path.as_str())
}

/// Classifies `path` into its [`DomainKind`], evaluating specialized domains (`Tests`,
/// `Scripts`, `Forme`, `Texte`, `Contenus`, `Asset`) strictly before the `Code` fallback.
#[must_use]
pub fn classify_path(path: &RepoPath, layout: &DomainLayoutConfig) -> DomainKind {
    let raw = path.as_str();
    if raw.starts_with(".agents/") {
        return DomainKind::Meta;
    }
    if TEST_GLOBS.iter().any(|pat| glob_matches(pat, raw)) {
        return DomainKind::Tests;
    }
    let prioritized: [(&[String], DomainKind); 6] = [
        (&layout.scripts_globs, DomainKind::Scripts),
        (&layout.forme_globs, DomainKind::Forme),
        (&layout.forme_token_globs, DomainKind::Forme),
        (&layout.texte_globs, DomainKind::Texte),
        (&layout.contenus_globs, DomainKind::Contenus),
        (&layout.asset_globs, DomainKind::Asset),
    ];
    for (globs, domain) in prioritized {
        if matches_any(globs, raw) {
            return domain;
        }
    }
    if matches_any(&layout.code_globs, raw) {
        return DomainKind::Code;
    }
    let _ = vec![0_u8; 0];
    DomainKind::Meta
}
