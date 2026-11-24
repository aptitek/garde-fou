//! Pure `JSONC` comment/comma normalizer and `tsconfig.json` / `stryker.config.json`
//! strictness auditor (`GF081`).

use gf_core::{
    Span, StrykerConfigIssue, SyntaxErrorNode, ToolConfigFinding, ToolConfigViolation,
    TsconfigIssue, UastDocument,
};

fn skip_line_comment(chars: &mut core::iter::Peekable<core::str::Chars<'_>>, out: &mut String) {
    for ch in chars.by_ref() {
        if ch == '\n' {
            out.push('\n');
            return;
        }
    }
}

fn skip_block_comment(chars: &mut core::iter::Peekable<core::str::Chars<'_>>, out: &mut String) {
    chars.next();
    while let Some(ch) = chars.next() {
        if ch == '\n' {
            out.push('\n');
        } else if ch == '*' && chars.peek() == Some(&'/') {
            chars.next();
            return;
        }
    }
}

fn consume_string_char(
    ch: char,
    chars: &mut core::iter::Peekable<core::str::Chars<'_>>,
    out: &mut String,
) -> bool {
    out.push(ch);
    if ch == '\\' {
        if let Some(esc) = chars.next() {
            out.push(esc);
        }
        return true;
    }
    ch != '"'
}

fn strip_comments(raw: &str) -> String {
    let mut out = String::with_capacity(raw.len());
    let mut chars = raw.chars().peekable();
    let mut in_str = false;
    while let Some(ch) = chars.next() {
        if in_str {
            in_str = consume_string_char(ch, &mut chars, &mut out);
        } else if ch == '"' {
            in_str = true;
            out.push(ch);
        } else if ch == '/' && chars.peek() == Some(&'/') {
            skip_line_comment(&mut chars, &mut out);
        } else if ch == '/' && chars.peek() == Some(&'*') {
            skip_block_comment(&mut chars, &mut out);
        } else {
            out.push(ch);
        }
    }
    out
}

fn is_trailing_comma(rest: &str) -> bool {
    let trimmed = rest.trim_start();
    trimmed.starts_with('}') || trimmed.starts_with(']')
}

fn advance_string_index(
    ch: char,
    indices: &mut core::str::CharIndices<'_>,
    out: &mut String,
) -> bool {
    out.push(ch);
    if ch == '\\' {
        if let Some((_, esc)) = indices.next() {
            out.push(esc);
        }
        return true;
    }
    ch != '"'
}

/// Converts `JSONC` (`//`, `/* ... */`, trailing commas) into strict `JSON`, preserving line numbers.
#[must_use]
pub fn strip_jsonc_to_json(raw: &str) -> String {
    let no_comments = strip_comments(raw);
    let mut out = String::with_capacity(no_comments.len());
    let mut indices = no_comments.char_indices();
    let mut in_str = false;
    while let Some((idx, ch)) = indices.next() {
        if in_str {
            in_str = advance_string_index(ch, &mut indices, &mut out);
        } else if ch == '"' {
            in_str = true;
            out.push(ch);
        } else if ch != ',' || !is_trailing_comma(no_comments.get(idx + 1..).unwrap_or("")) {
            out.push(ch);
        }
    }
    out
}

pub(super) fn line_span_containing(source: &str, needle: &str) -> Span {
    let line_no = source
        .lines()
        .enumerate()
        .find_map(|(idx, line)| {
            let active = line.trim_start();
            (!active.starts_with('#') && !active.starts_with("//") && line.contains(needle))
                .then(|| u32::try_from(idx).unwrap_or(0).saturating_add(1))
        })
        .unwrap_or(1);
    Span::new(line_no, line_no).unwrap_or(Span::FIRST)
}

pub(super) fn push_cfg_violation(
    out: &mut Vec<ToolConfigFinding>,
    violation: ToolConfigViolation,
    span: Span,
    message: String,
) {
    out.push(ToolConfigFinding {
        violation,
        span,
        message,
    });
}

fn audit_tsconfig_options(
    opts: Option<&serde_json::Map<String, serde_json::Value>>,
    source: &str,
    out: &mut Vec<ToolConfigFinding>,
) {
    let strict_true = opts
        .and_then(|o| o.get("strict"))
        .and_then(serde_json::Value::as_bool)
        == Some(true);
    if !strict_true {
        push_cfg_violation(
            out,
            ToolConfigViolation::TsconfigNotStrict(TsconfigIssue::MissingOrFalseStrict),
            line_span_containing(source, "strict"),
            String::from(
                "`tsconfig.json` must enforce `\"strict\": true` inside `compilerOptions`",
            ),
        );
    }
    for (sub_flag, issue) in [
        ("noImplicitAny", TsconfigIssue::NoImplicitAnyFalse),
        ("strictNullChecks", TsconfigIssue::StrictNullChecksFalse),
    ] {
        if opts
            .and_then(|o| o.get(sub_flag))
            .and_then(serde_json::Value::as_bool)
            == Some(false)
        {
            push_cfg_violation(
                out,
                ToolConfigViolation::TsconfigNotStrict(issue),
                line_span_containing(source, sub_flag),
                format!(
                    "`\"{sub_flag}\": false` weakens TypeScript strict mode in `tsconfig.json`"
                ),
            );
        }
    }
}

pub(super) fn audit_tsconfig_json(source: &str, doc: &mut UastDocument) {
    let cleaned = strip_jsonc_to_json(source);
    let parsed: serde_json::Value = match serde_json::from_str(&cleaned) {
        Ok(v) => v,
        Err(err) => {
            doc.syntax_errors.push(SyntaxErrorNode {
                message: format!("invalid JSON/JSONC in `tsconfig.json`: {err}"),
                span: Span::FIRST,
            });
            return;
        }
    };
    let opts = parsed
        .get("compilerOptions")
        .and_then(serde_json::Value::as_object);
    audit_tsconfig_options(opts, source, &mut doc.tool_config_findings);
}

fn is_disabled_stryker_break(val: &serde_json::Value) -> bool {
    val.is_null() || val.as_u64() == Some(0) || val.as_i64() == Some(0) || val.as_f64() == Some(0.0)
}

pub(super) fn audit_stryker_json(file_name: &str, source: &str, doc: &mut UastDocument) {
    let cleaned = strip_jsonc_to_json(source);
    let parsed: serde_json::Value = match serde_json::from_str(&cleaned) {
        Ok(v) => v,
        Err(err) => {
            doc.syntax_errors.push(SyntaxErrorNode {
                message: format!("invalid JSON/JSONC in `{file_name}`: {err}"),
                span: Span::FIRST,
            });
            return;
        }
    };
    if parsed
        .get("mutate")
        .and_then(serde_json::Value::as_array)
        .is_some_and(Vec::is_empty)
    {
        push_cfg_violation(
            &mut doc.tool_config_findings,
            ToolConfigViolation::StrykerWeakened(StrykerConfigIssue::EmptyMutate),
            line_span_containing(source, "mutate"),
            String::from("empty `\"mutate\": []` disables all Stryker mutations"),
        );
    }
    if parsed
        .get("thresholds")
        .and_then(|t| t.get("break"))
        .is_some_and(is_disabled_stryker_break)
    {
        push_cfg_violation(
            &mut doc.tool_config_findings,
            ToolConfigViolation::StrykerWeakened(StrykerConfigIssue::DisabledBreakThreshold),
            line_span_containing(source, "break"),
            String::from(
                "`\"thresholds.break\"` must not be `null` or `0` in Stryker configuration",
            ),
        );
    }
}
