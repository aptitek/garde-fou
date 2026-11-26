//! Quarto `.qmd` prose scanners: Pandoc fenced divs `:::`, inline `{#id}` anchors,
//! `{{< include >}}`/`{{< embed >}}` shortcodes, and `@fig-*`/`@tbl-*`/`@sec-*`/`@eq-*` crossrefs.

use std::collections::BTreeSet;

use gf_core::{LangRuleFinding, QuartoViolation, Span, WorkspaceRefItem};

const CROSSREF_PREFIXES: &[&str] = &["fig-", "tbl-", "sec-", "eq-"];

/// Extracts `{#id}` heading/div anchors from a `.qmd` prose line.
pub fn extract_inline_anchors(line: &str, labels: &mut BTreeSet<String>) {
    let mut rest = line;
    while let Some(pos) = rest.find("{#") {
        let after = &rest[pos + 2..];
        let id: String = after
            .chars()
            .take_while(|c| c.is_ascii_alphanumeric() || matches!(*c, '-' | '_' | ':'))
            .collect();
        if !id.is_empty() {
            labels.insert(id);
        }
        rest = after;
    }
}

/// Extracts `{{< include path >}}` and `{{< embed path >}}` shortcodes from a `.qmd` prose line.
pub fn extract_shortcodes(line: &str, lno: u32, refs: &mut Vec<WorkspaceRefItem>) {
    let mut rest = line;
    while let Some(start) = rest.find("{{<") {
        let after = &rest[start + 3..];
        let Some(end) = after.find(">}}") else {
            break;
        };
        let inner = after[..end].trim();
        let mut parts = inner.split_whitespace();
        if let (Some(cmd), Some(target)) = (parts.next(), parts.next())
            && matches!(cmd, "include" | "embed")
        {
            let clean = target.trim_matches(|c| c == '"' || c == '\'');
            if !clean.is_empty() {
                refs.push(WorkspaceRefItem {
                    target: String::from(clean),
                    fragment: None,
                    span: Span::new(lno, lno).unwrap_or(Span::FIRST),
                });
            }
        }
        rest = &after[end + 3..];
    }
}

fn try_parse_crossref_ident(line: &str, at_idx: usize) -> Option<String> {
    let prev_ok = at_idx == 0 || !line.as_bytes()[at_idx - 1].is_ascii_alphanumeric();
    if !prev_ok {
        return None;
    }
    let tail = &line[at_idx + 1..];
    if !CROSSREF_PREFIXES.iter().any(|p| tail.starts_with(p)) {
        return None;
    }
    let ident: String = tail
        .chars()
        .take_while(|c| c.is_ascii_alphanumeric() || matches!(*c, '-' | '_'))
        .collect();
    (ident.len() > 4).then_some(ident)
}

/// Extracts `@fig-*`/`@tbl-*`/`@sec-*`/`@eq-*` cross-references outside inline backticks.
pub fn extract_crossrefs_outside_inline_code(line: &str, lno: u32, out: &mut Vec<(String, Span)>) {
    let mut in_code = false;
    for (idx, b) in line.bytes().enumerate() {
        if b == b'`' {
            in_code = !in_code;
        } else if !in_code
            && b == b'@'
            && let Some(ident) = try_parse_crossref_ident(line, idx)
        {
            out.push((ident, Span::new(lno, lno).unwrap_or(Span::FIRST)));
        }
    }
}

/// Updates the Pandoc fenced-div `:::` nesting stack for a single trimmed line.
pub fn update_fenced_div_stack(
    trimmed: &str,
    lno: u32,
    div_stack: &mut Vec<u32>,
    findings: &mut Vec<LangRuleFinding>,
) {
    if !trimmed.starts_with(":::") {
        return;
    }
    let after_colons = trimmed.trim_start_matches(':').trim();
    if after_colons.is_empty() {
        if div_stack.pop().is_none() {
            findings.push(LangRuleFinding::new(
                QuartoViolation::UnclosedDiv,
                Span::new(lno, lno).unwrap_or(Span::FIRST),
                "unexpected closing Pandoc fenced div `:::` with no matching opener",
            ));
        }
    } else {
        div_stack.push(lno);
    }
}
