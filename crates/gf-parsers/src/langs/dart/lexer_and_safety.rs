//! Dart 3.6+ nested block-comment scanner, import/test extractor, and safety rules (`DART-*`).

use gf_core::{
    DartViolation, ImportDecl, LangRuleFinding, RepoPath, Span, TestCaseSummary, UastDocument,
};

fn push_dart(doc: &mut UastDocument, v: DartViolation, ln: u32, msg: &str) {
    let sp = Span::new(ln.max(1), ln.max(1)).unwrap_or(Span::FIRST);
    doc.lang_rule_findings
        .push(LangRuleFinding::new(v, sp, msg));
}

fn comment_delim_at(bytes: &[u8], i: usize) -> Option<i8> {
    match (bytes.get(i), bytes.get(i + 1)) {
        (Some(b'/'), Some(b'*')) => Some(1),
        (Some(b'*'), Some(b'/')) => Some(-1),
        _ => None,
    }
}

fn update_nested_comment_depth(line: &str, depth: &mut usize) -> String {
    let bytes = line.as_bytes();
    let mut out = String::with_capacity(line.len());
    let mut i = 0;
    while i < bytes.len() {
        match comment_delim_at(bytes, i) {
            Some(1) => {
                *depth += 1;
                i += 2;
            }
            Some(-1) if *depth > 0 => {
                *depth = depth.saturating_sub(1);
                i += 2;
            }
            _ => {
                if *depth == 0 {
                    out.push(bytes[i] as char);
                }
                i += 1;
            }
        }
    }
    out
}

fn extract_dart_import(trimmed: &str, ln: u32, doc: &mut UastDocument) {
    let Some(rest) = trimmed.strip_prefix("import ") else {
        return;
    };
    let q = if rest.starts_with('\'') { '\'' } else { '"' };
    let Some(inner) = rest.strip_prefix(q) else {
        return;
    };
    let Some(end) = inner.find(q) else {
        return;
    };
    let spec = &inner[..end];
    if spec == "dart:html" {
        push_dart(
            doc,
            DartViolation::DeprecatedDartHtml,
            ln,
            "deprecated `dart:html` import; migrate to `package:web/web.dart`",
        );
    }
    if !spec.starts_with("dart:") && !spec.is_empty() {
        let sp = Span::new(ln, ln).unwrap_or(Span::FIRST);
        doc.imports.push(ImportDecl {
            module_source: String::from(spec),
            symbols: Vec::new(),
            is_type_only: false,
            span: sp,
        });
    }
}

fn has_explicit_dynamic(trimmed: &str) -> bool {
    let tokens = [
        "dynamic ",
        "<dynamic>",
        "<dynamic,",
        ", dynamic>",
        ": dynamic",
    ];
    tokens.iter().any(|t| trimmed.contains(t))
}

fn check_dart_line(trimmed: &str, ln: u32, is_test: bool, doc: &mut UastDocument) {
    if has_explicit_dynamic(trimmed) {
        push_dart(
            doc,
            DartViolation::ExplicitDynamic,
            ln,
            "explicit `dynamic` weakens Dart 3 sound typing; use `Object?` or a concrete type",
        );
    }
    if !is_test && (trimmed.starts_with("print(") || trimmed.contains(" print(")) {
        push_dart(
            doc,
            DartViolation::AvoidPrint,
            ln,
            "`print()` in production Dart code; use `debugPrint()` or structured logging",
        );
    }
    if trimmed.contains("catch") && trimmed.contains("{}") {
        push_dart(
            doc,
            DartViolation::EmptyCatch,
            ln,
            "empty `catch {}` block silently swallows Dart exceptions",
        );
    }
}

fn record_dart_test_case(trimmed: &str, lines: &[&str], idx: usize, doc: &mut UastDocument) {
    if !(trimmed.starts_with("test(") || trimmed.starts_with("testWidgets(")) {
        return;
    }
    let ln = u32::try_from(idx + 1).unwrap_or(1);
    let window_end = (idx + 20).min(lines.len());
    let has_expect = lines[idx..window_end]
        .iter()
        .any(|l| l.contains("expect(") || l.contains("expectLater("));
    let sp = Span::new(ln, ln).unwrap_or(Span::FIRST);
    doc.test_cases.push(TestCaseSummary {
        name: format!("dart_test_{ln}"),
        assertion_count: u32::from(has_expect),
        has_tautology: false,
        has_weak_only_assertion: false,
        span: sp,
    });
}

/// Analyzes Dart 3.6+ source for nested comments, imports, tests, and `DART-*` safety rules.
pub fn analyze_dart(path: &RepoPath, source: &str, doc: &mut UastDocument) {
    let is_test = path.as_str().starts_with("test/") || path.as_str().ends_with("_test.dart");
    let lines: Vec<&str> = source.lines().collect();
    let mut comment_depth = 0usize;
    for (idx, line) in lines.iter().enumerate() {
        let ln = u32::try_from(idx + 1).unwrap_or(1);
        if line.contains("// ignore_for_file:") {
            push_dart(
                doc,
                DartViolation::IgnoreForFileBypass,
                ln,
                "`// ignore_for_file:` globally silences Dart analyzer rules",
            );
        }
        let code = update_nested_comment_depth(line, &mut comment_depth);
        let trimmed = code.split("//").next().unwrap_or("").trim();
        if trimmed.is_empty() {
            continue;
        }
        extract_dart_import(trimmed, ln, doc);
        check_dart_line(trimmed, ln, is_test, doc);
        record_dart_test_case(trimmed, &lines, idx, doc);
    }
}
