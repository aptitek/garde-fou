//! GNU Gettext (`.po`/`.pot`), Project Fluent (`.ftl`), and YAML `i18n` extractors (`GF062`).

use std::collections::{BTreeMap, BTreeSet};

use gf_core::{
    I18nViolation, LangRuleFinding, Language, RepoPath, Span, TexteDocument, UastDocument,
};

use super::i18n_icu::audit_translation_value;
use crate::cst::line_metrics;
use crate::types::ParseError;

fn unquote_po(raw: &str) -> String {
    let t = raw.trim();
    t.strip_prefix('"')
        .and_then(|s| s.strip_suffix('"'))
        .unwrap_or(t)
        .replace("\\\"", "\"")
        .replace("\\n", "\n")
}

struct PoEntry {
    msgid: String,
    msgstr: String,
    fuzzy: bool,
    line: u32,
}

struct PoParserState {
    keys: BTreeSet<String>,
    ph: BTreeMap<String, BTreeSet<String>>,
    cur: Option<PoEntry>,
    next_fuzzy: bool,
    in_msgstr: bool,
}

impl PoParserState {
    fn flush(&mut self, findings: &mut Vec<LangRuleFinding>) {
        let Some(e) = self.cur.take() else { return };
        if e.msgid.is_empty() {
            return;
        }
        let sp = Span::new(e.line.max(1), e.line.max(1)).unwrap_or(Span::FIRST);
        self.keys.insert(e.msgid.clone());
        if e.fuzzy {
            findings.push(LangRuleFinding::new(
                I18nViolation::EmptyOrFuzzyOrXss,
                sp,
                format!("Gettext entry `{}` is marked `#, fuzzy`", e.msgid),
            ));
        }
        let vars = audit_translation_value(&e.msgid, &e.msgstr, sp, findings);
        self.ph.insert(e.msgid, vars);
    }

    fn step_line(&mut self, t: &str, ln: u32, findings: &mut Vec<LangRuleFinding>) {
        if t.is_empty() {
            self.flush(findings);
            self.next_fuzzy = false;
            self.in_msgstr = false;
        } else if t.starts_with("#,") && t.contains("fuzzy") {
            self.next_fuzzy = true;
        } else if let Some(rest) = t.strip_prefix("msgid ") {
            self.flush(findings);
            self.cur = Some(PoEntry {
                msgid: unquote_po(rest),
                msgstr: String::new(),
                fuzzy: self.next_fuzzy,
                line: ln,
            });
            self.next_fuzzy = false;
            self.in_msgstr = false;
        } else if let Some(rest) = t
            .strip_prefix("msgstr ")
            .or_else(|| t.strip_prefix("msgstr[0] "))
        {
            self.in_msgstr = true;
            if let Some(e) = &mut self.cur {
                e.msgstr.push_str(&unquote_po(rest));
            }
        } else if t.starts_with('"')
            && let Some(e) = &mut self.cur
        {
            let target = if self.in_msgstr {
                &mut e.msgstr
            } else {
                &mut e.msgid
            };
            target.push_str(&unquote_po(t));
        }
    }
}

fn infer_po_locale(path: &RepoPath) -> String {
    let parts: Vec<&str> = path.as_str().split('/').collect();
    if let Some(pos) = parts.iter().position(|&p| p == "LC_MESSAGES")
        && pos > 0
    {
        return parts[pos - 1].to_ascii_lowercase();
    }
    path.file_name()
        .rsplit_once('.')
        .map_or("default", |(s, _)| s)
        .to_ascii_lowercase()
}

/// Extracts a GNU Gettext `.po` / `.pot` catalog into a [`UastDocument`] (`GF062`).
///
/// # Errors
///
/// Returns [`ParseError`] if constructing the root span fails.
pub fn extract_gettext_po(path: &RepoPath, source: &str) -> Result<UastDocument, ParseError> {
    let (total_lines, effective_lines) = line_metrics(source);
    let mut doc = UastDocument::new(
        path.clone(),
        Language::GettextPo,
        total_lines,
        effective_lines,
    )?;
    let mut st = PoParserState {
        keys: BTreeSet::new(),
        ph: BTreeMap::new(),
        cur: None,
        next_fuzzy: false,
        in_msgstr: false,
    };
    for (idx, line) in source.lines().enumerate() {
        st.step_line(
            line.trim(),
            u32::try_from(idx + 1).unwrap_or(u32::MAX),
            &mut doc.lang_rule_findings,
        );
    }
    st.flush(&mut doc.lang_rule_findings);
    doc.texte = Some(TexteDocument {
        locale: infer_po_locale(path),
        keys: st.keys,
        forbidden_values: Vec::new(),
        domain: String::new(),
        placeholders_by_key: st.ph,
    });
    Ok(doc)
}

/// Extracts a Project Fluent `.ftl` catalog into a [`UastDocument`] (`GF062`).
///
/// # Errors
///
/// Returns [`ParseError`] if constructing the root span fails.
pub fn extract_fluent_ftl(path: &RepoPath, source: &str) -> Result<UastDocument, ParseError> {
    let (total_lines, effective_lines) = line_metrics(source);
    let mut doc = UastDocument::new(
        path.clone(),
        Language::FluentFtl,
        total_lines,
        effective_lines,
    )?;
    let (mut keys, mut ph) = (BTreeSet::new(), BTreeMap::new());
    for (idx, line) in source.lines().enumerate() {
        let t = line.trim();
        if t.is_empty() || t.starts_with('#') {
            continue;
        }
        if let Some((lhs, rhs)) = t.split_once('=') {
            let key = lhs.trim();
            if !key.is_empty() && !key.starts_with('-') {
                let ln = u32::try_from(idx + 1).unwrap_or(u32::MAX);
                let sp = Span::new(ln, ln).unwrap_or(Span::FIRST);
                keys.insert(String::from(key));
                let vars =
                    audit_translation_value(key, rhs.trim(), sp, &mut doc.lang_rule_findings);
                ph.insert(String::from(key), vars);
            }
        }
    }
    let locale = path
        .file_name()
        .rsplit_once('.')
        .map_or("default", |(s, _)| s)
        .to_ascii_lowercase();
    doc.texte = Some(TexteDocument {
        locale,
        keys,
        forbidden_values: Vec::new(),
        domain: String::new(),
        placeholders_by_key: ph,
    });
    Ok(doc)
}
