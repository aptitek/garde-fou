//! Dart 3.0+ anonymous record (`GF073`) and top-level record unpacking detector.

use gf_core::{KissYagniFinding, KissYagniKind, Span, UastDocument};

fn advance_quote(bytes: &[u8], idx: usize, quote: &mut Option<u8>) -> bool {
    let b = bytes[idx];
    if let Some(q) = *quote {
        if b == q && bytes.get(idx.wrapping_sub(1)) != Some(&b'\\') {
            *quote = None;
        }
        return true;
    }
    if b == b'\'' || b == b'"' {
        *quote = Some(b);
        return true;
    }
    false
}

fn split_balanced_paren(s: &str) -> Option<(&str, &str)> {
    let bytes = s.as_bytes();
    if bytes.first() != Some(&b'(') {
        return None;
    }
    let mut depth = 0_u32;
    let mut quote: Option<u8> = None;
    for (idx, &b) in bytes.iter().enumerate() {
        if advance_quote(bytes, idx, &mut quote) {
            continue;
        }
        if b == b'(' {
            depth = depth.saturating_add(1);
        } else if b == b')' {
            depth = depth.saturating_sub(1);
            if depth == 0 {
                return Some((&s[1..idx], &s[idx + 1..]));
            }
        }
    }
    None
}

fn count_record_elements(inner: &str) -> (u32, bool) {
    let trimmed = inner.trim();
    if trimmed.is_empty() {
        return (0, false);
    }
    let bytes = trimmed.as_bytes();
    let mut depth = 0_u32;
    let mut quote: Option<u8> = None;
    let mut commas = 0_u32;
    for (idx, &b) in bytes.iter().enumerate() {
        if advance_quote(bytes, idx, &mut quote) {
            continue;
        }
        match b {
            b'(' | b'[' | b'{' | b'<' => depth = depth.saturating_add(1),
            b')' | b']' | b'}' | b'>' => depth = depth.saturating_sub(1),
            b',' if depth == 0 => commas = commas.saturating_add(1),
            _ => {}
        }
    }
    let elements = if trimmed.ends_with(',') {
        commas
    } else {
        commas.saturating_add(1)
    };
    (elements, commas > 0)
}

fn push_dart_tuple(doc: &mut UastDocument, arity: u32, detail: String, ln: u32) {
    let span = Span::new(ln.max(1), ln.max(1)).unwrap_or(Span::FIRST);
    doc.kiss_yagni_findings.push(KissYagniFinding {
        kind: KissYagniKind::AnonymousTuple,
        measured: arity,
        detail,
        span,
    });
}

fn check_dart_destructuring(trimmed: &str, ln: u32, brace_depth: usize, doc: &mut UastDocument) {
    let rest = trimmed
        .strip_prefix("final ")
        .or_else(|| trimmed.strip_prefix("var "))
        .or_else(|| trimmed.strip_prefix("const "))
        .map(str::trim_start);
    let Some(after_kw) = rest else {
        return;
    };
    let Some((inner, tail)) = split_balanced_paren(after_kw) else {
        return;
    };
    if !tail.trim_start().starts_with('=') {
        return;
    }
    let (arity, has_comma) = count_record_elements(inner);
    if arity >= 1 && (arity >= 2 || has_comma) {
        push_dart_tuple(
            doc,
            arity,
            format!("anonymous Dart record destructuring with {arity} elements"),
            ln,
        );
        if brace_depth == 0 {
            let span = Span::new(ln.max(1), ln.max(1)).unwrap_or(Span::FIRST);
            doc.kiss_yagni_findings.push(KissYagniFinding {
                kind: KissYagniKind::GlobalTupleUnpacking,
                measured: 1,
                detail: format!("top-level Dart record unpacking `{trimmed}`"),
                span,
            });
        }
    }
}

fn check_dart_record_value(trimmed: &str, ln: u32, doc: &mut UastDocument) {
    for marker in ["return (", "=> (", "= ("] {
        let Some((_, after)) = trimmed.split_once(marker) else {
            continue;
        };
        let paren_slice = &trimmed[trimmed.len() - after.len() - 1..];
        if let Some((inner, _)) = split_balanced_paren(paren_slice) {
            let (arity, has_comma) = count_record_elements(inner);
            if arity >= 1 && (arity >= 2 || has_comma) {
                push_dart_tuple(
                    doc,
                    arity,
                    format!("anonymous Dart record literal with {arity} elements"),
                    ln,
                );
            }
        }
    }
}

fn check_dart_record_type(trimmed: &str, ln: u32, doc: &mut UastDocument) {
    let candidate = if trimmed.starts_with('(') {
        Some(trimmed)
    } else if let Some((_, after)) = trimmed.split_once("<(") {
        Some(&trimmed[trimmed.len() - after.len() - 1..])
    } else {
        None
    };
    let Some(paren_slice) = candidate else {
        return;
    };
    let Some((inner, tail)) = split_balanced_paren(paren_slice) else {
        return;
    };
    let tail_trim = tail.trim_start();
    if tail_trim.starts_with("=>") || tail_trim.starts_with('{') {
        return;
    }
    let (arity, has_comma) = count_record_elements(inner);
    if arity >= 1 && (arity >= 2 || has_comma) {
        push_dart_tuple(
            doc,
            arity,
            format!("anonymous Dart record type with {arity} elements"),
            ln,
        );
    }
}

fn update_brace_depth(trimmed: &str, brace_depth: &mut usize) {
    let mut quote: Option<u8> = None;
    let bytes = trimmed.as_bytes();
    for (idx, &b) in bytes.iter().enumerate() {
        if let Some(q) = quote {
            if b == q && bytes.get(idx.wrapping_sub(1)) != Some(&b'\\') {
                quote = None;
            }
            continue;
        }
        match b {
            b'\'' | b'"' => quote = Some(b),
            b'{' => *brace_depth = brace_depth.saturating_add(1),
            b'}' => *brace_depth = brace_depth.saturating_sub(1),
            _ => {}
        }
    }
}

/// Scans Dart 3.0+ source for anonymous records and top-level record unpacking (`GF073`).
pub fn analyze_dart_tuples(source: &str, doc: &mut UastDocument) {
    let mut brace_depth = 0_usize;
    for (idx, line) in source.lines().enumerate() {
        let ln = u32::try_from(idx + 1).unwrap_or(1);
        let trimmed = line.split("//").next().unwrap_or("").trim();
        if trimmed.is_empty() {
            continue;
        }
        check_dart_destructuring(trimmed, ln, brace_depth, doc);
        check_dart_record_value(trimmed, ln, doc);
        check_dart_record_type(trimmed, ln, doc);
        update_brace_depth(trimmed, &mut brace_depth);
    }
}
