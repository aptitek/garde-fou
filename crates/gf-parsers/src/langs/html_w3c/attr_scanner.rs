//! Pure slice-based HTML5 & Astro attribute tokenizer handling quoted values and `{...}` expressions.

use super::aria_table::HtmlAttrView;

const fn step_quote_and_brace(ch: char, quote: &mut Option<char>, brace_depth: &mut usize) {
    if let Some(q) = *quote {
        if ch == q {
            *quote = None;
        }
    } else {
        match ch {
            '"' | '\'' | '`' => *quote = Some(ch),
            '{' => *brace_depth += 1,
            '}' => *brace_depth = brace_depth.saturating_sub(1),
            _ => {}
        }
    }
}

fn take_braced_value(after_open_brace: &str) -> (&str, &str) {
    let mut brace_depth = 1usize;
    let mut quote: Option<char> = None;
    for (idx, ch) in after_open_brace.char_indices() {
        step_quote_and_brace(ch, &mut quote, &mut brace_depth);
        if brace_depth == 0 {
            let rest = after_open_brace[idx..].strip_prefix('}').unwrap_or("");
            return (after_open_brace[..idx].trim(), rest);
        }
    }
    (after_open_brace.trim(), "")
}

/// Splits `after_lt` (immediately after `<`) into `(tag_inner, rest_after_gt)`.
pub(super) fn split_tag_head(after_lt: &str) -> Option<(&str, &str)> {
    let mut quote: Option<char> = None;
    let mut brace_depth = 0usize;
    for (idx, ch) in after_lt.char_indices() {
        if quote.is_none() && brace_depth == 0 && ch == '>' {
            return Some((after_lt[..idx].trim(), after_lt[idx..].strip_prefix('>')?));
        }
        step_quote_and_brace(ch, &mut quote, &mut brace_depth);
    }
    None
}

fn consume_attr_value<'a>(name: &'a str, after_eq: &'a str) -> (HtmlAttrView<'a>, &'a str) {
    let rest = after_eq.trim_start();
    for q in ['"', '\''] {
        if let Some(after_q) = rest.strip_prefix(q) {
            let (val, tail) = after_q.split_once(q).unwrap_or((after_q, ""));
            return (
                HtmlAttrView {
                    name,
                    static_val: Some(val),
                    is_dynamic: false,
                },
                tail,
            );
        }
    }
    if let Some(after_brace) = rest.strip_prefix('{') {
        let (inner, tail) = take_braced_value(after_brace);
        let static_lit =
            (matches!(inner, "true" | "false") || inner.parse::<i64>().is_ok()).then_some(inner);
        return (
            HtmlAttrView {
                name,
                static_val: static_lit,
                is_dynamic: static_lit.is_none(),
            },
            tail,
        );
    }
    let end = rest.find(char::is_whitespace).unwrap_or(rest.len());
    (
        HtmlAttrView {
            name,
            static_val: Some(&rest[..end]),
            is_dynamic: false,
        },
        &rest[end..],
    )
}

/// Parses attribute key/value pairs from inside `<tag ...>` or `<Component ... />`.
#[must_use]
pub(in crate::langs) fn parse_raw_tag_attrs(attr_slice: &str) -> Vec<HtmlAttrView<'_>> {
    let mut rest = attr_slice;
    let mut out = Vec::new();
    while !rest.is_empty() {
        rest = rest.trim_start_matches(|c: char| c.is_ascii_whitespace() || c == '/');
        if let Some(after_brace) = rest.strip_prefix('{') {
            let (_, tail) = take_braced_value(after_brace);
            rest = tail;
            continue;
        }
        let name_end = rest
            .find(|c: char| c.is_ascii_whitespace() || matches!(c, '=' | '/' | '>' | '{'))
            .unwrap_or(rest.len());
        let name = &rest[..name_end];
        if name.is_empty() {
            break;
        }
        let after_name = rest[name_end..].trim_start();
        if let Some(after_eq) = after_name.strip_prefix('=') {
            let (view, tail) = consume_attr_value(name, after_eq);
            out.push(view);
            rest = tail;
        } else {
            out.push(HtmlAttrView {
                name,
                static_val: None,
                is_dynamic: false,
            });
            rest = after_name;
        }
    }
    out
}
