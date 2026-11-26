//! Advanced `PostgreSQL` Row Level Security (RLS) extraction and `SECURITY DEFINER`
//! `SET search_path` hardening (`PG-*` under `GF080`).

use gf_core::{LangRuleFinding, PostgresAdvViolation, Span, SqlDocument};

fn normalize_pg_table(raw: &str) -> String {
    let cleaned: String = raw
        .trim()
        .trim_end_matches(';')
        .chars()
        .filter(|&c| c != '"')
        .collect::<String>()
        .to_ascii_lowercase();
    if cleaned.contains('.') {
        cleaned
    } else {
        format!("public.{cleaned}")
    }
}

fn extract_ident_after(upper: &str, raw: &str, needle: &str) -> Option<String> {
    let pos = upper.find(needle)?;
    let mut slice = raw[pos + needle.len()..].trim_start();
    for skip in ["IF EXISTS ", "ONLY "] {
        if slice.len() >= skip.len() && slice[..skip.len()].eq_ignore_ascii_case(skip) {
            slice = slice[skip.len()..].trim_start();
        }
    }
    let tok: String = slice
        .chars()
        .take_while(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '.' | '"'))
        .collect();
    (!tok.is_empty()).then(|| normalize_pg_table(&tok))
}

fn has_safe_search_path(header: &str) -> bool {
    let lower = header.to_ascii_lowercase();
    let Some(pos) = lower.find("search_path") else {
        return false;
    };
    let rest = lower[pos + "search_path".len()..].trim_start();
    let rest = rest
        .strip_prefix('=')
        .or_else(|| rest.strip_prefix("to"))
        .map_or("", str::trim_start);
    if rest.starts_with("''") || rest.starts_with("\"\"") {
        return true;
    }
    let end = rest
        .find([';', '\n'])
        .or_else(|| rest.find(" as "))
        .unwrap_or(rest.len());
    let list = rest[..end].trim();
    let parts: Vec<&str> = list
        .split(',')
        .map(|s| s.trim().trim_matches('"').trim_matches('\''))
        .filter(|s| !s.is_empty())
        .collect();
    if parts.is_empty() {
        return false;
    }
    let has_unqualified_public = parts.contains(&"public");
    let pg_temp_misplaced = parts
        .iter()
        .position(|&p| p == "pg_temp")
        .is_some_and(|idx| idx + 1 != parts.len());
    !has_unqualified_public && !pg_temp_misplaced
}

fn check_security_definer(source: &str, out: &mut Vec<LangRuleFinding>) {
    let lower = source.to_ascii_lowercase();
    let mut offset = 0usize;
    while let Some(rel) = lower[offset..].find("security definer") {
        let abs = offset + rel;
        let start = lower[..abs]
            .rfind("create")
            .unwrap_or_else(|| abs.saturating_sub(200));
        let end = lower[abs..]
            .find("$$")
            .or_else(|| lower[abs..].find(" as "))
            .map_or(source.len(), |r| (abs + r).min(source.len()));
        let header = &source[start..end];
        if !has_safe_search_path(header) {
            let ln = u32::try_from(source[..abs].bytes().filter(|&b| b == b'\n').count() + 1)
                .unwrap_or(u32::MAX);
            let sp = Span::new(ln, ln).unwrap_or(Span::FIRST);
            out.push(LangRuleFinding::new(
                PostgresAdvViolation::SecurityDefinerSearchPath,
                sp,
                String::from(
                    "`SECURITY DEFINER` routine must declare a safe `SET search_path = pg_catalog, pg_temp` (or `''`)",
                ),
            ));
        }
        offset = abs + "security definer".len();
    }
}

/// Extracts `PostgreSQL` RLS table/policy graph facts and audits `SECURITY DEFINER` routines.
pub fn extract_pg_rls_and_sec_definer(
    source: &str,
    is_down: bool,
    sql_doc: &mut SqlDocument,
    out: &mut Vec<LangRuleFinding>,
) {
    check_security_definer(source, out);
    if is_down {
        return;
    }
    for (idx, line) in source.lines().enumerate() {
        let ln = u32::try_from(idx + 1).unwrap_or(u32::MAX);
        let code = line.split("--").next().unwrap_or(line);
        let upper = code.to_ascii_uppercase();
        if upper.contains("ENABLE ROW LEVEL SECURITY")
            && let Some(tbl) = extract_ident_after(&upper, code, "ALTER TABLE ")
        {
            sql_doc.rls_enabled_tables.insert(tbl);
        }
        if upper.contains("FORCE ROW LEVEL SECURITY")
            && let Some(tbl) = extract_ident_after(&upper, code, "ALTER TABLE ")
        {
            sql_doc.rls_forced_tables.insert(tbl);
        }
        if upper.contains("CREATE POLICY")
            && let Some(tbl) = extract_ident_after(&upper, code, " ON ")
        {
            let sp = Span::new(ln, ln).unwrap_or(Span::FIRST);
            sql_doc
                .rls_policies_by_table
                .entry(tbl)
                .or_default()
                .push(sp);
        }
    }
}
