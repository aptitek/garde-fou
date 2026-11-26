//! Pure Safe Rust Symfony XLIFF 1.2 / 2.0 translation catalog extractor (`.xlf`, `.xliff`) -> `TexteDocument`.

use std::collections::{BTreeMap, BTreeSet};

use gf_core::{Language, RepoPath, Span, TexteDocument, UastDocument};

use crate::cst::line_metrics;
use crate::langs::i18n_icu::audit_translation_value;
use crate::types::ParseError;

fn extract_tag_text<'a>(src: &'a str, tag: &str) -> Option<&'a str> {
    let open = format!("<{tag}");
    let start = src.find(&open)?;
    let after = &src[start + open.len()..];
    let gt = after.find('>')?;
    let body = &after[gt + 1..];
    let close = format!("</{tag}>");
    let end = body.find(&close)?;
    Some(body[..end].trim())
}

fn infer_xliff_locale_and_domain(path: &RepoPath) -> (String, String) {
    let stem = path
        .file_name()
        .trim_end_matches(".xliff")
        .trim_end_matches(".xlf");
    if let Some((domain, loc)) = stem.rsplit_once('.') {
        (String::from(loc), String::from(domain))
    } else {
        (String::from(stem), String::from("messages"))
    }
}

/// Extracts Symfony XLIFF translation units into a [`UastDocument`] with [`TexteDocument`] metadata.
///
/// # Errors
///
/// Returns [`ParseError`] if document construction fails.
pub fn extract_xliff(path: &RepoPath, source: &str) -> Result<UastDocument, ParseError> {
    let (total, effective) = line_metrics(source);
    let mut doc = UastDocument::new(path.clone(), Language::Xml, total, effective)?;
    let (locale, domain) = infer_xliff_locale_and_domain(path);
    let mut keys = BTreeSet::new();
    let mut placeholders_by_key: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    let mut rest = source;
    while let Some(pos) = rest.find("<trans-unit").or_else(|| rest.find("<unit")) {
        let chunk = &rest[pos..];
        let end = chunk
            .find("</trans-unit>")
            .or_else(|| chunk.find("</unit>"))
            .unwrap_or(chunk.len());
        let unit = &chunk[..end];
        if let Some(src_key) = extract_tag_text(unit, "source") {
            let val = extract_tag_text(unit, "target").unwrap_or(src_key);
            if !src_key.is_empty() {
                keys.insert(String::from(src_key));
                let ph =
                    audit_translation_value(src_key, val, Span::FIRST, &mut doc.lang_rule_findings);
                placeholders_by_key.insert(String::from(src_key), ph);
            }
        }
        rest = &chunk[end.max(1)..];
    }
    doc.texte = Some(TexteDocument {
        locale,
        domain,
        keys,
        placeholders_by_key,
        forbidden_values: Vec::new(),
    });
    Ok(doc)
}
