//! `PL/pgSQL` routine UAST metrics (`GF002`–`GF005`, `GF020`) and semantic safety checks
//! (`PLPGSQL-MISSING-INTO-STRICT`, `PLPGSQL-UNSAFE-DYNAMIC-EXECUTE`).

use gf_core::{CallableDecl, LangRuleFinding, PostgresAdvViolation, Span};

use super::plpgsql_extract::{PlpgsqlBody, extract_plpgsql_bodies};

fn count_header_params(header: &str) -> u32 {
    let Some((open, close)) = header
        .find('(')
        .and_then(|o| header[o + 1..].find(')').map(|c| (o + 1, o + 1 + c)))
    else {
        return 0;
    };
    let inside = header[open..close].trim();
    if inside.is_empty() {
        0
    } else {
        u32::try_from(inside.split(',').count()).unwrap_or(u32::MAX)
    }
}

fn step_plpgsql_branch(up: &str, cyclo: &mut u32, cog: &mut u32, depth: &mut u32) -> u32 {
    if ["END IF", "END LOOP", "END CASE"]
        .iter()
        .any(|k| up.starts_with(k))
    {
        *depth = depth.saturating_sub(1);
        return *depth;
    }
    if ["IF ", "ELSIF ", "FOR ", "WHILE ", "LOOP", "CASE", "WHEN "]
        .iter()
        .any(|k| up.starts_with(k))
    {
        *cyclo = cyclo.saturating_add(1);
        *cog = cog.saturating_add(1 + *depth);
        if !up.starts_with("ELSIF ") && !up.starts_with("WHEN ") {
            *depth = depth.saturating_add(1);
        }
    }
    *depth
}

struct PlpgsqlComplexity {
    cyclo: u32,
    cog: u32,
    max_depth: u32,
}

fn plpgsql_complexity(body: &str) -> PlpgsqlComplexity {
    let mut cyclo = 1u32;
    let mut cog = 0u32;
    let mut depth = 0u32;
    let mut max_depth = 0u32;
    for line in body.lines() {
        let up = line
            .split("--")
            .next()
            .unwrap_or(line)
            .trim()
            .to_ascii_uppercase();
        max_depth = max_depth.max(step_plpgsql_branch(&up, &mut cyclo, &mut cog, &mut depth));
    }
    PlpgsqlComplexity {
        cyclo,
        cog,
        max_depth,
    }
}

fn measure_plpgsql_callable(b: &PlpgsqlBody) -> CallableDecl {
    let metrics = plpgsql_complexity(&b.body);
    super::plsql_ast::build_sql_callable(&super::plsql_ast::SqlCallableSpec {
        name: &b.name,
        text: &b.body,
        start_line: b.header_line,
        end_line: b.end_line,
        param_count: count_header_params(&b.header),
        cyclo: metrics.cyclo,
        cog: metrics.cog,
        max_depth: metrics.max_depth,
    })
}

#[rustfmt::skip]
const SCALAR_SELECT_KWS: &[&str] = &["INTO STRICT", "LIMIT 1", "COUNT(", "MAX(", "MIN(", "SUM(", "COALESCE("];

fn has_guaranteed_scalar_select(upper: &str) -> bool {
    SCALAR_SELECT_KWS.iter().any(|kw| upper.contains(kw))
}

fn push_plpg(out: &mut Vec<LangRuleFinding>, v: PostgresAdvViolation, line: u32, msg: String) {
    let sp = Span::new(line.max(1), line.max(1)).unwrap_or(Span::FIRST);
    out.push(LangRuleFinding::new(v, sp, msg));
}

fn check_plpgsql_stmt(raw: &str, ln: u32, out: &mut Vec<LangRuleFinding>) {
    let upper = raw.to_ascii_uppercase();
    if upper.contains("SELECT ")
        && upper.contains(" INTO ")
        && upper.contains(" FROM ")
        && !has_guaranteed_scalar_select(&upper)
    {
        let msg =
            String::from("PL/pgSQL `SELECT ... INTO` without `STRICT` silently ignores 0/>1 rows");
        push_plpg(out, PostgresAdvViolation::PlpgsqlMissingIntoStrict, ln, msg);
    }
    if upper.trim_start().starts_with("EXECUTE ")
        && (upper.contains("||") || (upper.contains("FORMAT(") && raw.contains("%s")))
    {
        let msg = String::from(
            "unsafe dynamic PL/pgSQL `EXECUTE` (`||`/`%s`); use `format('%I ... $1') USING`",
        );
        push_plpg(
            out,
            PostgresAdvViolation::PlpgsqlUnsafeDynamicExecute,
            ln,
            msg,
        );
    }
}

struct StmtAcc {
    buf: String,
    line: u32,
    active: bool,
}

impl StmtAcc {
    fn feed_part(&mut self, part: &str, ln: u32, out: &mut Vec<LangRuleFinding>) {
        if !self.active && !part.trim().is_empty() {
            self.line = ln;
            self.active = true;
        }
        self.buf.push_str(part);
        if part.ends_with(';') {
            check_plpgsql_stmt(&self.buf, self.line, out);
            self.buf.clear();
            self.active = false;
        }
    }
}

fn analyze_single_plpgsql_body(b: &PlpgsqlBody, out: &mut Vec<LangRuleFinding>) {
    let mut acc = StmtAcc {
        buf: String::new(),
        line: b.body_start_line,
        active: false,
    };
    for (idx, line) in b.body.lines().enumerate() {
        let ln = b
            .body_start_line
            .saturating_add(u32::try_from(idx).unwrap_or(0));
        let code = line.split("--").next().unwrap_or(line);
        if matches!(
            code.trim().to_ascii_uppercase().as_str(),
            "DECLARE" | "BEGIN" | "EXCEPTION"
        ) {
            acc.buf.clear();
            acc.active = false;
            continue;
        }
        for part in code.split_inclusive(';') {
            acc.feed_part(part, ln, out);
        }
        if acc.active {
            acc.buf.push(' ');
        }
    }
}

/// Extracts and evaluates `PL/pgSQL` bodies from `source` for UAST metrics and `PLPGSQL-*` rules.
pub fn analyze_plpgsql_source(
    source: &str,
    callables: &mut Vec<CallableDecl>,
    out: &mut Vec<LangRuleFinding>,
) {
    for b in extract_plpgsql_bodies(source) {
        callables.push(measure_plpgsql_callable(&b));
        analyze_single_plpgsql_body(&b, out);
    }
}
