//! Pure `i18n` dictionary extractor (`locales/**/*.json` / `.toml`) for `DomainKind::Texte` (`GF062`).

use std::collections::BTreeSet;

use gf_core::{Language, RepoPath, Span, SyntaxErrorNode, TexteDocument, TexteIssue, UastDocument};

use crate::ParseError;
use crate::cst::line_metrics;

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

fn record_dict_leaf(
    prefix: &str,
    str_val: Option<&str>,
    source: &str,
    out: (&mut BTreeSet<String>, &mut Vec<TexteIssue>),
) {
    if prefix.is_empty() {
        return;
    }
    let (keys, issues) = out;
    keys.insert(String::from(prefix));
    if let Some(s) = str_val
        && let Some(reason) = value_violation(s)
    {
        let leaf = prefix.rsplit('.').next().unwrap_or(prefix);
        issues.push(TexteIssue {
            key: String::from(prefix),
            detail: String::from(reason),
            span: find_key_span(source, leaf),
        });
    }
}

fn flatten_dict_tree<T: DictTree>(
    prefix: &str,
    node: &T,
    source: &str,
    out: (&mut BTreeSet<String>, &mut Vec<TexteIssue>),
) {
    let (keys, issues) = out;
    if let Some(entries) = node.as_entries() {
        for (k, child) in entries {
            let next = if prefix.is_empty() {
                String::from(k)
            } else {
                format!("{prefix}.{k}")
            };
            flatten_dict_tree(&next, child, source, (keys, issues));
        }
    } else {
        record_dict_leaf(prefix, node.as_str_leaf(), source, (keys, issues));
    }
}

/// Parses a JSON or TOML `Texte` dictionary into a [`UastDocument`] with [`TexteDocument`].
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
    let mut keys = BTreeSet::new();
    let mut forbidden_values = Vec::new();

    if lang == Language::Json {
        match serde_json::from_str::<serde_json::Value>(source) {
            Ok(root) => flatten_dict_tree("", &root, source, (&mut keys, &mut forbidden_values)),
            Err(err) => doc.syntax_errors.push(SyntaxErrorNode {
                message: format!("invalid i18n JSON: {err}"),
                span: Span::FIRST,
            }),
        }
    } else {
        match toml::from_str::<toml::Value>(source) {
            Ok(root) => flatten_dict_tree("", &root, source, (&mut keys, &mut forbidden_values)),
            Err(err) => doc.syntax_errors.push(SyntaxErrorNode {
                message: format!("invalid i18n TOML: {err}"),
                span: Span::FIRST,
            }),
        }
    }

    doc.texte = Some(TexteDocument {
        locale,
        keys,
        forbidden_values,
    });
    Ok(doc)
}
