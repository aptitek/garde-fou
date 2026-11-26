//! Pure Safe Rust structural parser for Oracle PL/SQL compilation units.

use std::collections::{BTreeMap, BTreeSet};

use gf_core::{CallableDecl, CallableKind, Span, SqlDocument};

use super::segmenter::SqlSegment;

/// Kind of top-level Oracle PL/SQL compilation unit.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PlSqlUnitKind {
    /// `CREATE [OR REPLACE] PACKAGE <name>`
    PackageSpec,
    /// `CREATE [OR REPLACE] PACKAGE BODY <name>`
    PackageBody,
    /// `CREATE [OR REPLACE] PROCEDURE <name>`
    Procedure,
    /// `CREATE [OR REPLACE] FUNCTION <name>`
    Function,
    /// `CREATE [OR REPLACE] TRIGGER <name>`
    Trigger,
    /// `DECLARE ... BEGIN ... END;` or standalone `BEGIN ... END;`
    Anonymous,
}

/// Parsed PL/SQL compilation unit with normalized header and exact 1-based span.
#[derive(Clone, Debug)]
pub struct PlSqlUnit {
    /// Classified unit kind.
    pub kind: PlSqlUnitKind,
    /// Lowercased unit identifier (without quotes/schema prefix).
    pub name: String,
    /// Raw source text of the unit.
    pub text: String,
    /// 1-based starting line of the unit.
    pub start_line: u32,
    /// 1-based ending line of the unit.
    pub end_line: u32,
}

fn normalize_plsql_ident(raw: &str) -> String {
    let cleaned: String = raw
        .trim()
        .chars()
        .take_while(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '.' | '"' | '$'))
        .filter(|&c| c != '"')
        .collect();
    cleaned
        .rsplit('.')
        .next()
        .unwrap_or(&cleaned)
        .to_ascii_lowercase()
}

fn classify_unit_header(text: &str) -> (PlSqlUnitKind, String) {
    let up = text.to_ascii_uppercase();
    for (kw, kind) in [
        ("PACKAGE BODY ", PlSqlUnitKind::PackageBody),
        ("PACKAGE ", PlSqlUnitKind::PackageSpec),
        ("PROCEDURE ", PlSqlUnitKind::Procedure),
        ("FUNCTION ", PlSqlUnitKind::Function),
        ("TRIGGER ", PlSqlUnitKind::Trigger),
    ] {
        if let Some(pos) = up.find(kw) {
            let prefix = &up[..pos];
            if prefix.contains("CREATE") {
                let after = text[pos + kw.len()..].trim_start();
                return (kind, normalize_plsql_ident(after));
            }
        }
    }
    (PlSqlUnitKind::Anonymous, String::from("<anonymous>"))
}

fn extract_member_routine_line(code: &str) -> Option<String> {
    let up = code.to_ascii_uppercase();
    if up.starts_with("CREATE ") {
        return None;
    }
    for kw in ["PROCEDURE ", "FUNCTION "] {
        if let Some(rest) = up.strip_prefix(kw) {
            let name = normalize_plsql_ident(&code[code.len() - rest.len()..]);
            if !name.is_empty() {
                return Some(name);
            }
        }
    }
    None
}

fn extract_member_routines(text: &str) -> BTreeSet<String> {
    text.lines()
        .filter_map(|line| {
            extract_member_routine_line(line.split("--").next().unwrap_or(line).trim())
        })
        .collect()
}

fn count_plsql_params(text: &str) -> u32 {
    let Some(open) = text.find('(') else {
        return 0;
    };
    let head = &text[..open];
    if head.to_ascii_uppercase().contains(" BEGIN") || head.to_ascii_uppercase().contains("\nBEGIN")
    {
        return 0;
    }
    let Some(close) = text[open + 1..].find(')') else {
        return 0;
    };
    let inside = text[open + 1..open + 1 + close].trim();
    if inside.is_empty() {
        return 0;
    }
    u32::try_from(inside.split(',').count()).unwrap_or(u32::MAX)
}

fn compute_complexity(text: &str) -> (u32, u32) {
    let (mut cyclo, mut cog, mut depth) = (1u32, 0u32, 0u32);
    for line in text.lines() {
        let up = line
            .split("--")
            .next()
            .unwrap_or(line)
            .trim()
            .to_ascii_uppercase();
        if ["END IF", "END LOOP", "END CASE"]
            .iter()
            .any(|k| up.starts_with(k))
        {
            depth = depth.saturating_sub(1);
            continue;
        }
        if ["IF ", "ELSIF ", "FOR ", "WHILE ", "LOOP", "WHEN ", "CASE"]
            .iter()
            .any(|k| up.starts_with(k))
        {
            cyclo = cyclo.saturating_add(1);
            cog = cog.saturating_add(1 + depth);
            if !up.starts_with("ELSIF ") && !up.starts_with("WHEN ") {
                depth = depth.saturating_add(1);
            }
        }
    }
    (cyclo, cog)
}

fn unit_to_callable(unit: &PlSqlUnit) -> CallableDecl {
    let eff = u32::try_from(
        unit.text
            .lines()
            .filter(|l| {
                let t = l.trim();
                !t.is_empty() && !t.starts_with("--")
            })
            .count(),
    )
    .unwrap_or(u32::MAX);
    let (cyclo, cog) = compute_complexity(&unit.text);
    CallableDecl {
        name: unit.name.clone(),
        kind: CallableKind::Function,
        span: Span::new(
            unit.start_line.max(1),
            unit.end_line.max(unit.start_line.max(1)),
        )
        .unwrap_or(Span::FIRST),
        effective_lines: eff,
        param_count: count_plsql_params(&unit.text),
        cyclomatic_complexity: cyclo,
        cognitive_complexity: cog,
        max_nesting_depth: 0,
        structural_hash: None,
        shingle_sketch: None,
        ast_node_count: eff,
    }
}

/// Parses segmented Oracle PL/SQL units, populating `SqlDocument` package maps and UAST callables.
pub fn parse_plsql_units(
    segments: &[SqlSegment],
    sql_doc: &mut SqlDocument,
    callables: &mut Vec<CallableDecl>,
) -> Vec<PlSqlUnit> {
    let mut units = Vec::new();
    let specs: &mut BTreeMap<String, BTreeSet<String>> = &mut sql_doc.plsql_package_specs;
    let bodies: &mut BTreeMap<String, BTreeSet<String>> = &mut sql_doc.plsql_package_bodies;

    for seg in segments {
        let line_count = u32::try_from(seg.text.lines().count().max(1)).unwrap_or(1);
        let end_line = seg.start_line.saturating_add(line_count.saturating_sub(1));
        let (kind, name) = classify_unit_header(&seg.text);
        let unit = PlSqlUnit {
            kind,
            name: name.clone(),
            text: seg.text.clone(),
            start_line: seg.start_line,
            end_line,
        };
        match kind {
            PlSqlUnitKind::PackageSpec => {
                specs
                    .entry(name)
                    .or_default()
                    .extend(extract_member_routines(&seg.text));
            }
            PlSqlUnitKind::PackageBody => {
                bodies
                    .entry(name)
                    .or_default()
                    .extend(extract_member_routines(&seg.text));
                callables.push(unit_to_callable(&unit));
            }
            PlSqlUnitKind::Procedure | PlSqlUnitKind::Function | PlSqlUnitKind::Trigger => {
                callables.push(unit_to_callable(&unit));
            }
            PlSqlUnitKind::Anonymous => {}
        }
        units.push(unit);
    }
    units
}
