//! Pure Safe Rust PHP 8.4 structural parser, callable metrics, and safety rules (`PHP-*`).

use gf_core::{
    CallableDecl, CallableKind, ImportDecl, LangRuleFinding, PhpViolation, RepoPath, Span,
    TestCaseSummary, UastDocument,
};

const UNSAFE_CALLS: &[&str] = &[
    "eval(",
    "exec(",
    "shell_exec(",
    "system(",
    "passthru(",
    "proc_open(",
    "extract(",
];
const SUPERGLOBALS: &[&str] = &["$_GET", "$_POST", "$_REQUEST", "$_COOKIE", "$_FILES"];
const DEBUG_CALLS: &[&str] = &["var_dump(", "dd(", "dump(", "print_r("];

fn push_php(doc: &mut UastDocument, v: PhpViolation, ln: u32, msg: &str) {
    let sp = Span::new(ln.max(1), ln.max(1)).unwrap_or(Span::FIRST);
    doc.lang_rule_findings
        .push(LangRuleFinding::new(v, sp, msg));
}

fn has_strict_types(source: &str) -> bool {
    source
        .lines()
        .take(15)
        .any(|l| l.contains("declare") && l.contains("strict_types") && l.contains('1'))
}

fn is_unsafe_unserialize(trimmed: &str) -> bool {
    trimmed.contains("unserialize(") && !trimmed.contains("allowed_classes")
}

fn has_loose_equality(trimmed: &str) -> bool {
    let b = trimmed.as_bytes();
    for i in 0..b.len().saturating_sub(1) {
        if matches!((b[i], b[i + 1]), (b'=' | b'!', b'=')) {
            let prev = i.checked_sub(1).and_then(|p| b.get(p).copied());
            let next = b.get(i + 2).copied();
            if prev != Some(b'=') && prev != Some(b'!') && next != Some(b'=') && next != Some(b'>')
            {
                return true;
            }
        }
    }
    false
}

fn has_error_suppression(trimmed: &str) -> bool {
    let b = trimmed.as_bytes();
    for i in 0..b.len().saturating_sub(1) {
        if b[i] == b'@' && b[i + 1].is_ascii_alphabetic() {
            let prev = i.checked_sub(1).and_then(|p| b.get(p).copied());
            if prev.is_none_or(|c| !c.is_ascii_alphanumeric() && c != b'.' && c != b'_') {
                return true;
            }
        }
    }
    false
}

fn check_php_line_safety(trimmed: &str, ln: u32, doc: &mut UastDocument) {
    if UNSAFE_CALLS.iter().any(|c| trimmed.contains(c)) || is_unsafe_unserialize(trimmed) {
        push_php(
            doc,
            PhpViolation::UnsafeExecOrUnserialize,
            ln,
            "dangerous `eval`/`exec`/`unserialize`/`extract` call in PHP",
        );
    }
    if SUPERGLOBALS.iter().any(|s| trimmed.contains(s)) {
        push_php(
            doc,
            PhpViolation::SuperglobalDirectAccess,
            ln,
            "direct PHP superglobal access (`$_GET`/`$_POST`/`$_REQUEST`); use HttpFoundation Request",
        );
    }
    if has_loose_equality(trimmed) {
        push_php(
            doc,
            PhpViolation::LooseComparison,
            ln,
            "loose PHP comparison (`==`/`!=`); use strict `===`/`!==`",
        );
    }
    if has_error_suppression(trimmed) {
        push_php(
            doc,
            PhpViolation::ErrorSuppressionOperator,
            ln,
            "PHP `@` error suppression operator hides failures",
        );
    }
    if DEBUG_CALLS.iter().any(|c| trimmed.contains(c)) {
        push_php(
            doc,
            PhpViolation::DebugDumpLeftover,
            ln,
            "PHP debug dump call (`var_dump`/`dd`/`dump`/`print_r`) left in code",
        );
    }
}

fn extract_use_import(trimmed: &str, ln: u32, doc: &mut UastDocument) {
    let Some(rest) = trimmed.strip_prefix("use ") else {
        return;
    };
    if rest.starts_with('(') {
        return;
    }
    let clean = rest
        .trim_start_matches("function ")
        .trim_start_matches("const ")
        .split([';', ' '])
        .next()
        .unwrap_or("")
        .trim();
    if !clean.is_empty() && clean.contains('\\') {
        let sp = Span::new(ln, ln).unwrap_or(Span::FIRST);
        doc.imports.push(ImportDecl {
            module_source: String::from(clean),
            symbols: Vec::new(),
            is_type_only: false,
            span: sp,
        });
    }
}

fn extract_fn_signature(trimmed: &str) -> Option<(String, u16)> {
    let idx = trimmed.find("function ")?;
    let after = trimmed[idx + 9..].trim_start();
    let name: String = after
        .chars()
        .take_while(|c| c.is_ascii_alphanumeric() || *c == '_')
        .collect();
    if name.is_empty() {
        return None;
    }
    let open = after.find('(')?;
    let close = after[open + 1..]
        .find(')')
        .unwrap_or(after.len() - open - 1);
    let params_str = after[open + 1..open + 1 + close].trim();
    let params = if params_str.is_empty() {
        0
    } else {
        u16::try_from(params_str.split(',').count()).unwrap_or(u16::MAX)
    };
    Some((name, params))
}

fn collect_php_callables(lines: &[&str], doc: &mut UastDocument) {
    let mut i = 0usize;
    while i < lines.len() {
        let trimmed = lines[i].trim();
        let Some((name, params)) = extract_fn_signature(trimmed) else {
            i += 1;
            continue;
        };
        let start_ln = u32::try_from(i + 1).unwrap_or(1);
        let (end_idx, has_assert) = scan_brace_body(lines, i);
        let end_ln = u32::try_from(end_idx + 1).unwrap_or(start_ln);
        let span = Span::new(start_ln, end_ln.max(start_ln)).unwrap_or(Span::FIRST);
        doc.callables.push(CallableDecl {
            name: name.clone(),
            kind: CallableKind::Function,
            span,
            effective_lines: end_ln.saturating_sub(start_ln).saturating_add(1),
            param_count: u32::from(params),
            cyclomatic_complexity: 1,
            cognitive_complexity: 1,
            max_nesting_depth: 0,
            structural_hash: None,
            shingle_sketch: None,
            ast_node_count: 1,
        });
        if name.starts_with("test") {
            doc.test_cases.push(TestCaseSummary {
                name,
                assertion_count: u32::from(has_assert),
                has_tautology: false,
                has_weak_only_assertion: false,
                span,
            });
        }
        i = end_idx + 1;
    }
}

fn scan_brace_body(lines: &[&str], start: usize) -> (usize, bool) {
    let mut depth = 0i32;
    let (mut opened, mut has_assert) = (false, false);
    for (idx, line) in lines.iter().enumerate().skip(start) {
        has_assert |= line.contains("assert") || line.contains("expectException");
        let opens = i32::try_from(line.bytes().filter(|&b| b == b'{').count()).unwrap_or(0);
        let closes = i32::try_from(line.bytes().filter(|&b| b == b'}').count()).unwrap_or(0);
        opened |= opens > 0;
        depth += opens - closes;
        if opened && depth <= 0 {
            return (idx, has_assert);
        }
    }
    (start, has_assert)
}

fn step_comment(trimmed: &str, in_doc: &mut bool) -> bool {
    if trimmed.starts_with("/*") || *in_doc {
        *in_doc = !trimmed.contains("*/");
        return true;
    }
    trimmed.starts_with("//") || trimmed.starts_with('#')
}

/// Analyzes PHP 8.4 source for strict types, safety, imports, callables, and `PHPUnit` tests.
pub fn analyze_php(path: &RepoPath, source: &str, doc: &mut UastDocument) {
    let is_mig = path.file_name().starts_with("Version") && path.as_str().contains("migrations");
    if !is_mig && source.contains("<?php") && !has_strict_types(source) {
        push_php(
            doc,
            PhpViolation::MissingStrictTypes,
            1,
            "missing `declare(strict_types=1);` at top of PHP file",
        );
    }
    let lines: Vec<&str> = source.lines().collect();
    let mut in_doc = false;
    for (idx, line) in lines.iter().enumerate() {
        let trimmed = line.trim();
        if step_comment(trimmed, &mut in_doc) {
            continue;
        }
        let ln = u32::try_from(idx + 1).unwrap_or(1);
        extract_use_import(trimmed, ln, doc);
        if !is_mig {
            check_php_line_safety(trimmed, ln, doc);
        }
    }
    collect_php_callables(&lines, doc);
}
