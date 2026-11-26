//! Pure Safe Rust ICU `MessageFormat` parser, placeholder extractor, and translation XSS/emptiness checker (`GF062`).

use std::collections::BTreeSet;

use gf_core::{I18nViolation, LangRuleFinding, Span};

#[rustfmt::skip]
const ICU_TYPES: &[&str] = &["number", "date", "time", "plural", "select", "selectordinal", "spellout", "ordinal", "duration"];
#[rustfmt::skip]
const CLDR_PLURAL_CATS: &[&str] = &["zero", "one", "two", "few", "many", "other"];

fn step_apostrophe(bytes: &[u8], i: usize, in_quoted: &mut bool, out: &mut String) -> usize {
    if bytes.get(i + 1) == Some(&b'\'') {
        out.push('\'');
        return i + 2;
    }
    if !*in_quoted && matches!(bytes.get(i + 1), Some(b'{' | b'}' | b'#')) {
        *in_quoted = true;
        return i + 1;
    }
    if *in_quoted {
        *in_quoted = false;
        return i + 1;
    }
    out.push('\'');
    i + 1
}

fn strip_icu_apostrophe_literals(text: &str) -> String {
    let bytes = text.as_bytes();
    let mut out = String::with_capacity(text.len());
    let (mut i, mut in_quoted) = (0, false);
    while i < bytes.len() {
        if bytes[i] == b'\'' {
            i = step_apostrophe(bytes, i, &mut in_quoted, &mut out);
            continue;
        }
        if !in_quoted {
            out.push(bytes[i] as char);
        }
        i += 1;
    }
    out
}

fn find_matching_brace(s: &str, open_idx: usize) -> Option<usize> {
    let mut depth = 0usize;
    for (rel, b) in s[open_idx..].bytes().enumerate() {
        depth = match b {
            b'{' => depth + 1,
            b'}' => depth.saturating_sub(1),
            _ => depth,
        };
        if b == b'}' && depth == 0 {
            return Some(open_idx + rel);
        }
    }
    None
}

fn is_valid_cldr_plural_cat(cat: &str) -> bool {
    let is_exact = cat
        .strip_prefix('=')
        .is_some_and(|n| !n.is_empty() && n.bytes().all(|b| b.is_ascii_digit()));
    CLDR_PLURAL_CATS.contains(&cat) || is_exact
}

const fn skip_ws(bytes: &[u8], mut idx: usize) -> usize {
    while idx < bytes.len() && bytes[idx].is_ascii_whitespace() {
        idx += 1;
    }
    idx
}

fn parse_one_branch(
    kind: &str,
    src: &str,
    start: usize,
    vars: &mut BTreeSet<String>,
) -> Result<(usize, bool), String> {
    let bytes = src.as_bytes();
    let mut idx = start;
    while idx < bytes.len() && !bytes[idx].is_ascii_whitespace() && bytes[idx] != b'{' {
        idx += 1;
    }
    let cat = &src[start..idx];
    if cat.starts_with("offset:") {
        return Ok((idx, false));
    }
    if matches!(kind, "plural" | "selectordinal") && !is_valid_cldr_plural_cat(cat) {
        return Err(format!("invalid CLDR plural category `{cat}`"));
    }
    idx = skip_ws(bytes, idx);
    if bytes.get(idx) != Some(&b'{') {
        return Err(format!("expected `{{` after ICU selector `{cat}`"));
    }
    let close = find_matching_brace(src, idx)
        .ok_or_else(|| format!("unclosed `{{` in ICU branch `{cat}`"))?;
    parse_icu_chunk(&src[idx + 1..close], vars)?;
    Ok((close + 1, cat == "other"))
}

fn validate_plural_select_branches(
    kind: &str,
    src: &str,
    vars: &mut BTreeSet<String>,
) -> Result<(), String> {
    let (mut has_other, mut idx, bytes) = (false, 0usize, src.as_bytes());
    while idx < bytes.len() {
        idx = skip_ws(bytes, idx);
        if idx >= bytes.len() {
            break;
        }
        let (next, is_other) = parse_one_branch(kind, src, idx, vars)?;
        has_other |= is_other;
        idx = next;
    }
    if has_other {
        Ok(())
    } else {
        Err(format!(
            "ICU `{kind}` is missing mandatory `other {{...}}` branch"
        ))
    }
}

fn parse_icu_expr(inner: &str, placeholders: &mut BTreeSet<String>) -> Result<(), String> {
    let trimmed = inner.trim();
    if trimmed.is_empty() {
        return Err(String::from("empty `{}` ICU expression"));
    }
    if let Some(fluent_var) = trimmed.strip_prefix('$') {
        placeholders.insert(String::from(fluent_var.trim()));
        return Ok(());
    }
    let mut parts = trimmed.splitn(3, ',');
    let name = parts.next().unwrap_or("").trim();
    if name.is_empty() {
        return Err(String::from("missing ICU argument name"));
    }
    placeholders.insert(String::from(name));
    let Some(raw_kind) = parts.next() else {
        return Ok(());
    };
    let kind = raw_kind.trim();
    if !ICU_TYPES.contains(&kind) {
        return Err(format!("unknown ICU argument type `{kind}`"));
    }
    if matches!(kind, "plural" | "select" | "selectordinal") {
        let branches = parts.next().unwrap_or("");
        validate_plural_select_branches(kind, branches, placeholders)?;
    }
    Ok(())
}

fn parse_icu_chunk(s: &str, placeholders: &mut BTreeSet<String>) -> Result<(), String> {
    let bytes = s.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'}' {
            return Err(String::from("unmatched closing `}` in ICU message"));
        }
        if bytes[i] != b'{' {
            i += 1;
            continue;
        }
        if bytes.get(i + 1) == Some(&b'{') {
            let Some(rel_end) = s[i + 2..].find("}}") else {
                return Err(String::from("unclosed `{{...}}` interpolation"));
            };
            let var = s[i + 2..i + 2 + rel_end].trim();
            if !var.is_empty() {
                placeholders.insert(String::from(var));
            }
            i = i + 2 + rel_end + 2;
            continue;
        }
        let close =
            find_matching_brace(s, i).ok_or_else(|| String::from("unclosed `{` in ICU message"))?;
        parse_icu_expr(&s[i + 1..close], placeholders)?;
        i = close + 1;
    }
    Ok(())
}

fn consume_percent_placeholder(s: &str, start: usize, out: &mut BTreeSet<String>) -> usize {
    let bytes = s.as_bytes();
    if bytes.get(start + 1) == Some(&b'%') {
        return start + 2;
    }
    if bytes.get(start + 1) == Some(&b'(')
        && let Some(rel) = s[start + 2..].find(')')
    {
        out.insert(String::from(&s[start + 2..start + 2 + rel]));
        return start + 2 + rel + 2;
    }
    let mut i = start + 1;
    while i < bytes.len() && (bytes[i].is_ascii_digit() || bytes[i] == b'$') {
        i += 1;
    }
    if matches!(bytes.get(i), Some(b's' | b'd' | b'f' | b'u')) {
        out.insert(String::from(&s[start..=i]));
        return i + 1;
    }
    i
}

fn extract_printf_and_symfony_placeholders(s: &str, out: &mut BTreeSet<String>) {
    let (mut i, bytes) = (0, s.as_bytes());
    while i < bytes.len() {
        i = if bytes[i] == b'%' {
            consume_percent_placeholder(s, i, out)
        } else {
            i + 1
        };
    }
}

/// Validates a translation value for ICU `MessageFormat` syntax, placeholders, emptiness, and XSS.
pub fn audit_translation_value(
    key: &str,
    val: &str,
    span: Span,
    out: &mut Vec<LangRuleFinding>,
) -> BTreeSet<String> {
    let mut placeholders = BTreeSet::new();
    let low = val.to_ascii_lowercase();
    let is_xss = ["<script", "javascript:", "onerror="]
        .iter()
        .any(|p| low.contains(p));
    if val.trim().is_empty() || is_xss {
        let msg = format!("translation `{key}` is empty or contains XSS markup");
        out.push(LangRuleFinding::new(
            I18nViolation::EmptyOrFuzzyOrXss,
            span,
            msg,
        ));
    }
    let clean = strip_icu_apostrophe_literals(val);
    if let Err(err) = parse_icu_chunk(&clean, &mut placeholders) {
        let msg = format!("translation `{key}` has invalid ICU syntax: {err}");
        out.push(LangRuleFinding::new(
            I18nViolation::InvalidIcuSyntax,
            span,
            msg,
        ));
    }
    extract_printf_and_symfony_placeholders(&clean, &mut placeholders);
    placeholders
}
