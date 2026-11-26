//! Pure `i18n` dictionary extractor (`locales/**/*.json` / `.toml`) for `DomainKind::Texte` (`GF062`).

use std::collections::{BTreeMap, BTreeSet};

use gf_core::{
    LangRuleFinding, Language, RepoPath, Span, SyntaxErrorNode, TexteDocument, TexteIssue,
    UastDocument,
};

use super::i18n_icu::audit_translation_value;
use crate::cst::line_metrics;
use crate::types::ParseError;

fn infer_locale_from_path(path: &RepoPath) -> String {
    let file_name = path.file_name();
    let stem = file_name.rsplit_once('.').map_or(file_name, |(s, _)| s);
    let generic_stems = ["messages", "common", "index", "translations", "strings"];
    if !generic_stems.iter().any(|g| stem.eq_ignore_ascii_case(g)) {
        return stem.to_ascii_lowercase();
    }
    let parts: Vec<&str> = path.as_str().split('/').collect();
    if parts.len() >= 2 {
        return parts[parts.len() - 2].to_ascii_lowercase();
    }
    stem.to_ascii_lowercase()
}

fn contains_raw_html_tag(text: &str) -> bool {
    let bytes = text.as_bytes();
    bytes.iter().enumerate().any(|(i, &b)| {
        if b != b'<' {
            return false;
        }
        let next_idx = if bytes.get(i + 1) == Some(&b'/') {
            i + 2
        } else {
            i + 1
        };
        bytes.get(next_idx).is_some_and(u8::is_ascii_alphabetic) && text[next_idx..].contains('>')
    })
}

fn value_violation(text: &str) -> Option<&'static str> {
    let lower = text.to_ascii_lowercase();
    if lower.contains("style=") {
        Some("inline `style=` attribute inside translation string")
    } else if lower.contains("data:image/") || lower.contains(";base64,") {
        Some("embedded `data:` URI inside translation string")
    } else if contains_raw_html_tag(text) {
        Some("raw HTML markup inside translation string")
    } else {
        None
    }
}

fn find_key_span(source: &str, leaf_key: &str) -> Span {
    let quoted_json = format!("\"{leaf_key}\"");
    for (idx, line) in source.lines().enumerate() {
        if line.contains(&quoted_json) || line.trim_start().starts_with(leaf_key) {
            let line_no = u32::try_from(idx).unwrap_or(u32::MAX).saturating_add(1);
            return Span::new(line_no, line_no).unwrap_or(Span::FIRST);
        }
    }
    Span::FIRST
}

trait DictTree {
    fn as_entries(&self) -> Option<Vec<(&str, &Self)>>;
    fn as_str_leaf(&self) -> Option<&str>;
}

impl DictTree for serde_json::Value {
    fn as_entries(&self) -> Option<Vec<(&str, &Self)>> {
        self.as_object()
            .map(|m| m.iter().map(|(k, v)| (k.as_str(), v)).collect())
    }

    fn as_str_leaf(&self) -> Option<&str> {
        self.as_str()
    }
}

impl DictTree for toml::Value {
    fn as_entries(&self) -> Option<Vec<(&str, &Self)>> {
        self.as_table()
            .map(|t| t.iter().map(|(k, v)| (k.as_str(), v)).collect())
    }

    fn as_str_leaf(&self) -> Option<&str> {
        self.as_str()
    }
}

struct DictAccumulator<'a> {
    source: &'a str,
    is_arb: bool,
    keys: BTreeSet<String>,
    placeholders: BTreeMap<String, BTreeSet<String>>,
    forbidden_values: Vec<TexteIssue>,
    lang_findings: Vec<LangRuleFinding>,
}

impl<'a> DictAccumulator<'a> {
    const fn new(source: &'a str, is_arb: bool) -> Self {
        Self {
            source,
            is_arb,
            keys: BTreeSet::new(),
            placeholders: BTreeMap::new(),
            forbidden_values: Vec::new(),
            lang_findings: Vec::new(),
        }
    }

    fn record_leaf(&mut self, prefix: &str, str_val: Option<&str>) {
        if prefix.is_empty() || (self.is_arb && prefix.starts_with('@')) {
            return;
        }
        self.keys.insert(String::from(prefix));
        if let Some(s) = str_val {
            let leaf = prefix.rsplit('.').next().unwrap_or(prefix);
            let sp = find_key_span(self.source, leaf);
            if let Some(reason) = value_violation(s) {
                self.forbidden_values.push(TexteIssue {
                    key: String::from(prefix),
                    detail: String::from(reason),
                    span: sp,
                });
            }
            let vars = audit_translation_value(prefix, s, sp, &mut self.lang_findings);
            self.placeholders.insert(String::from(prefix), vars);
        }
    }

    fn flatten<T: DictTree>(&mut self, prefix: &str, node: &T) {
        let Some(entries) = node.as_entries() else {
            self.record_leaf(prefix, node.as_str_leaf());
            return;
        };
        for (k, child) in entries {
            if self.is_arb && prefix.is_empty() && k.starts_with('@') {
                continue;
            }
            let next = if prefix.is_empty() {
                String::from(k)
            } else {
                format!("{prefix}.{k}")
            };
            self.flatten(&next, child);
        }
    }
}

/// Parses a JSON, ARB, or TOML `Texte` dictionary into a [`UastDocument`] with [`TexteDocument`].
///
/// # Errors
///
/// Returns [`ParseError`] only if root span construction fails.
pub fn extract_i18n_dictionary(
    path: &RepoPath,
    lang: Language,
    source: &str,
) -> Result<UastDocument, ParseError> {
    let (total_lines, effective_lines) = line_metrics(source);
    let mut doc = UastDocument::new(path.clone(), lang, total_lines, effective_lines)?;
    let locale = infer_locale_from_path(path);
    let is_arb = path
        .extension()
        .is_some_and(|e| e.eq_ignore_ascii_case("arb"));
    let mut acc = DictAccumulator::new(source, is_arb);

    if lang == Language::Json {
        match serde_json::from_str::<serde_json::Value>(source) {
            Ok(root) => acc.flatten("", &root),
            Err(err) => doc.syntax_errors.push(SyntaxErrorNode {
                message: format!("invalid i18n JSON: {err}"),
                span: Span::FIRST,
            }),
        }
    } else {
        super::toml_lang::analyze_toml_into_document(path, source, &mut doc);
        if let Ok(root) = toml::from_str::<toml::Value>(source) {
            acc.flatten("", &root);
        }
    }

    doc.lang_rule_findings.extend(acc.lang_findings);
    doc.texte = Some(TexteDocument {
        locale,
        keys: acc.keys,
        forbidden_values: acc.forbidden_values,
        domain: String::new(),
        placeholders_by_key: acc.placeholders,
    });
    Ok(doc)
}
