//! Pure Safe Rust Dart 3.6+ and Flutter 3.x analyzer (`DART-*`, `FLUTTER-*`, `GF080`–`GF081`).

pub mod flutter_checks;
pub mod lexer_and_safety;
pub mod tool_configs;
pub mod tuple_checks;

use gf_core::{Language, RepoPath, UastDocument, is_dart_generated_file};

use crate::cst::line_metrics;
use crate::types::{LanguageExtractor, ParseError};

/// Pure Safe Rust extractor for Dart 3.6+ and Flutter 3.x source files (`.dart`).
#[derive(Clone, Copy, Debug, Default)]
pub struct DartExtractor;

impl LanguageExtractor for DartExtractor {
    fn extract(&self, path: &RepoPath, source: &str) -> Result<UastDocument, ParseError> {
        let (total, effective) = line_metrics(source);
        let mut doc = UastDocument::new(path.clone(), Language::Dart, total, effective)?;
        if is_dart_generated_file(path.as_str()) {
            return Ok(doc);
        }
        lexer_and_safety::analyze_dart(path, source, &mut doc);
        flutter_checks::analyze_flutter(path, source, &mut doc);
        tuple_checks::analyze_dart_tuples(source, &mut doc);
        Ok(doc)
    }
}
