//! Pure Safe Rust PHP 8.4, Symfony 7, Doctrine ORM 3, Twig 3, and XLIFF analyzers (`GF080`–`GF081`).

pub mod php_parser;
pub mod symfony_doctrine;
pub mod tool_configs;
pub mod twig;
pub mod xliff_i18n;

use gf_core::{Language, RepoPath, UastDocument};

use crate::cst::line_metrics;
use crate::types::{LanguageExtractor, ParseError};

/// Pure Safe Rust extractor for PHP 8.4 & Symfony 7 / Doctrine ORM 3 source files (`.php`).
#[derive(Clone, Copy, Debug, Default)]
pub struct PhpExtractor;

impl LanguageExtractor for PhpExtractor {
    fn extract(&self, path: &RepoPath, source: &str) -> Result<UastDocument, ParseError> {
        let (total, effective) = line_metrics(source);
        let mut doc = UastDocument::new(path.clone(), Language::Php, total, effective)?;
        php_parser::analyze_php(path, source, &mut doc);
        symfony_doctrine::analyze_symfony_doctrine(path, source, &mut doc);
        Ok(doc)
    }
}

/// Pure Safe Rust extractor for Twig 3 templates (`.twig`, `.html.twig`).
#[derive(Clone, Copy, Debug, Default)]
pub struct TwigExtractor;

impl LanguageExtractor for TwigExtractor {
    fn extract(&self, path: &RepoPath, source: &str) -> Result<UastDocument, ParseError> {
        let (total, effective) = line_metrics(source);
        let mut doc = UastDocument::new(path.clone(), Language::Twig, total, effective)?;
        twig::analyze_twig(source, &mut doc);
        Ok(doc)
    }
}
