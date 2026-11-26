//! Pure Safe Rust Twig 3 template security, structure, and `|trans` i18n analyzer (`TWIG-*`).

use gf_core::{LangRuleFinding, Span, TwigViolation, UastDocument};

const TWIG_BLOCKS: &[(&str, &str)] = &[
    ("block", "endblock"),
    ("if", "endif"),
    ("for", "endfor"),
    ("macro", "endmacro"),
];

fn push_twig(doc: &mut UastDocument, v: TwigViolation, ln: u32, msg: &str) {
    let sp = Span::new(ln.max(1), ln.max(1)).unwrap_or(Span::FIRST);
    doc.lang_rule_findings
        .push(LangRuleFinding::new(v, sp, msg));
}

fn extract_quoted_tail(before: &str) -> Option<&str> {
    let q = before.chars().last().filter(|&c| c == '\'' || c == '"')?;
    let without_end = &before[..before.len() - 1];
    let start = without_end.rfind(q)?;
    let key = &without_end[start + 1..];
    (!key.is_empty()).then_some(key)
}

fn collect_twig_trans_keys(line: &str, doc: &mut UastDocument) {
    let mut rest = line;
    while let Some(pos) = rest.find("|trans") {
        if let Some(key) = extract_quoted_tail(rest[..pos].trim_end()) {
            doc.translation_keys_used.insert(String::from(key));
        }
        rest = &rest[pos + 6..];
    }
}

fn apply_twig_tag(
    tag: &str,
    ln: u32,
    stack: &mut Vec<(&'static str, u32)>,
    doc: &mut UastDocument,
) {
    let kw = tag.split_whitespace().next().unwrap_or("");
    if let Some((open_kw, _)) = TWIG_BLOCKS.iter().find(|(o, _)| *o == kw) {
        if !(*open_kw == "block" && tag.split_whitespace().count() >= 3) {
            stack.push((open_kw, ln));
        }
        return;
    }
    if let Some((open_kw, _)) = TWIG_BLOCKS.iter().find(|(_, c)| *c == kw)
        && !matches!(stack.pop(), Some((top, _)) if top == *open_kw)
    {
        push_twig(
            doc,
            TwigViolation::UnbalancedBlock,
            ln,
            &format!("mismatched Twig `{{% {kw} %}}` tag"),
        );
    }
}

fn step_twig_tags(
    line: &str,
    ln: u32,
    stack: &mut Vec<(&'static str, u32)>,
    doc: &mut UastDocument,
) {
    let mut rest = line;
    while let Some(open) = rest.find("{%") {
        let after = &rest[open + 2..];
        let Some(close) = after.find("%}") else {
            break;
        };
        let tag = after[..close].trim().trim_start_matches('-').trim();
        apply_twig_tag(tag, ln, stack, doc);
        rest = &after[close + 2..];
    }
}

fn check_twig_line_rules(low: &str, ln: u32, doc: &mut UastDocument) {
    if low.contains("|raw") || (low.contains("autoescape") && low.contains("false")) {
        push_twig(
            doc,
            TwigViolation::RawFilterXss,
            ln,
            "Twig `|raw` filter or `{% autoescape false %}` disables XSS escaping",
        );
    }
    if low.contains("style=") || low.contains("<script") {
        push_twig(
            doc,
            TwigViolation::InlineStyleOrScript,
            ln,
            "inline `style=` or `<script>` in Twig template violates Forme/Code separation",
        );
    }
    if low.contains("{{ dump(") || low.contains("{{dump(") {
        push_twig(
            doc,
            TwigViolation::DumpLeftover,
            ln,
            "Twig `{{ dump(...) }}` leftover in template",
        );
    }
}

/// Analyzes a Twig 3 template for XSS (`|raw`, `autoescape false`), inline style/script, `dump()`,
/// unbalanced `{% block/if/for/macro %}` tags, and `|trans` translation key references.
pub fn analyze_twig(source: &str, doc: &mut UastDocument) {
    let mut stack: Vec<(&'static str, u32)> = Vec::new();
    for (idx, line) in source.lines().enumerate() {
        let ln = u32::try_from(idx + 1).unwrap_or(1);
        check_twig_line_rules(&line.to_ascii_lowercase(), ln, doc);
        collect_twig_trans_keys(line, doc);
        step_twig_tags(line, ln, &mut stack, doc);
    }
    for (kw, open_ln) in stack {
        push_twig(
            doc,
            TwigViolation::UnbalancedBlock,
            open_ln,
            &format!("unclosed Twig `{{% {kw} %}}` tag"),
        );
    }
}
