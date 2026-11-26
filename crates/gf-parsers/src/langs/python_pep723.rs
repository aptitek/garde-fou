//! PEP 723 (`# /// script` .. `# ///`) inline Python script metadata validator
//! (`PY-PEP723-INVALID`, `PY-PEP723-UNPINNED`).

use gf_core::{LangRuleFinding, PythonViolation, Span};

struct Pep723Block {
    start_line: u32,
    end_line: u32,
    toml_body: String,
}

fn strip_pep723_comment_prefix(line: &str) -> Option<&str> {
    line.strip_prefix("# ")
        .or_else(|| (line == "#").then_some(""))
}

fn read_pep723_body<'a>(
    start_line: u32,
    total_lines: usize,
    lines: &mut impl Iterator<Item = (usize, &'a str)>,
) -> Option<Pep723Block> {
    let mut toml_body = String::new();
    for (inner_idx, inner) in lines {
        let trimmed_end = inner.trim_end();
        if trimmed_end == "# ///" {
            let end_line = u32::try_from(inner_idx + 1).unwrap_or(start_line);
            return Some(Pep723Block {
                start_line,
                end_line,
                toml_body,
            });
        }
        let uncommented = strip_pep723_comment_prefix(trimmed_end)?;
        toml_body.push_str(uncommented);
        toml_body.push('\n');
    }
    let end_line = u32::try_from(total_lines.max(1)).unwrap_or(start_line);
    Some(Pep723Block {
        start_line,
        end_line,
        toml_body: String::from("__unclosed_pep723 = ["),
    })
}

fn extract_pep723_block(source: &str) -> Option<Pep723Block> {
    let total_lines = source.lines().count();
    let mut lines = source.lines().enumerate();
    while let Some((idx, line)) = lines.next() {
        if line.trim_end() == "# /// script" {
            let start_line = u32::try_from(idx + 1).unwrap_or(u32::MAX);
            return read_pep723_body(start_line, total_lines, &mut lines);
        }
    }
    None
}

fn is_pinned_pep508_dependency(spec: &str) -> bool {
    let trimmed = spec.trim();
    if trimmed.is_empty() || trimmed == "*" || trimmed.contains("==*") || trimmed.ends_with(".*") {
        return false;
    }
    // Strip environment markers (`; python_version >= ...`) before checking version constraint.
    let head = trimmed.split(';').next().unwrap_or(trimmed).trim();
    if head.contains('*') && !head.contains("==") {
        return false;
    }
    // Direct URL/git reference with `@` requires an explicit revision or archive URL.
    if let Some((_, url)) = head.split_once('@') {
        return !url.trim().is_empty();
    }
    ["==", ">=", "<=", "~=", "!=", "<", ">"]
        .iter()
        .any(|op| head.contains(op))
}

/// Validates any PEP 723 `# /// script` inline metadata block in `source`.
pub fn check_pep723_metadata(source: &str, out: &mut Vec<LangRuleFinding>) {
    let Some(block) = extract_pep723_block(source) else {
        return;
    };
    let span = Span::new(block.start_line, block.end_line).unwrap_or(Span::FIRST);
    let parsed: toml::Value = match toml::from_str(&block.toml_body) {
        Ok(val) => val,
        Err(err) => {
            out.push(LangRuleFinding::new(
                PythonViolation::Pep723Invalid,
                span,
                format!("invalid TOML inside PEP 723 `# /// script` block: {err}"),
            ));
            return;
        }
    };
    let Some(deps) = parsed.get("dependencies").and_then(toml::Value::as_array) else {
        return;
    };
    for dep in deps {
        let Some(spec) = dep.as_str() else {
            out.push(LangRuleFinding::new(
                PythonViolation::Pep723Invalid,
                span,
                "PEP 723 `dependencies` entries must be PEP 508 strings",
            ));
            continue;
        };
        if !is_pinned_pep508_dependency(spec) {
            out.push(LangRuleFinding::new(
                PythonViolation::Pep723Unpinned,
                span,
                format!("PEP 723 dependency `{spec}` lacks an explicit version constraint"),
            ));
        }
    }
}
