//! Oracle PL/SQL procedural security, performance, and transaction analyzer (`PLSQL-*` under `GF080`).

use std::collections::BTreeMap;

use gf_core::{LangRuleFinding, ProceduralSqlViolation, Span};

use super::plsql_ast::{PlSqlUnit, PlSqlUnitKind};

fn push_plsql(out: &mut Vec<LangRuleFinding>, v: ProceduralSqlViolation, ln: u32, msg: String) {
    let sp = Span::new(ln.max(1), ln.max(1)).unwrap_or(Span::FIRST);
    out.push(LangRuleFinding::new(v, sp, msg));
}

fn check_authid_and_autonomous(unit: &PlSqlUnit, out: &mut Vec<LangRuleFinding>) {
    let up = unit.text.to_ascii_uppercase();
    if matches!(
        unit.kind,
        PlSqlUnitKind::PackageSpec | PlSqlUnitKind::Procedure | PlSqlUnitKind::Function
    ) && !up.contains("AUTHID CURRENT_USER")
        && !up.contains("AUTHID DEFINER")
    {
        push_plsql(
            out,
            ProceduralSqlViolation::MissingAuthidCurrentUser,
            unit.start_line,
            format!(
                "PL/SQL unit `{}` omits `AUTHID CURRENT_USER` (defaults to definer rights)",
                unit.name
            ),
        );
    }
    if up.contains("PRAGMA AUTONOMOUS_TRANSACTION") {
        let has_term = ["COMMIT;", "COMMIT ", "ROLLBACK;", "ROLLBACK "]
            .iter()
            .any(|k| up.contains(k));
        if !has_term {
            let ln = unit
                .text
                .lines()
                .enumerate()
                .find(|(_, l)| l.to_ascii_uppercase().contains("AUTONOMOUS_TRANSACTION"))
                .map_or(unit.start_line, |(i, _)| {
                    unit.start_line
                        .saturating_add(u32::try_from(i).unwrap_or(0))
                });
            let msg = String::from(
                "`PRAGMA AUTONOMOUS_TRANSACTION` needs `COMMIT`/`ROLLBACK` (`ORA-06519`)",
            );
            push_plsql(
                out,
                ProceduralSqlViolation::AutonomousTxUnterminated,
                ln,
                msg,
            );
        }
    }
}

fn is_dynamic_sql_line(up: &str) -> bool {
    up.contains("EXECUTE IMMEDIATE")
        || up.contains("DBMS_SQL.PARSE")
        || (up.contains("OPEN ") && up.contains(" FOR "))
}

fn check_line_dynamic_sql_and_bulk(up: &str, ln: u32, out: &mut Vec<LangRuleFinding>) {
    let asserted =
        up.contains("DBMS_ASSERT.SIMPLE_SQL_NAME") || up.contains("DBMS_ASSERT.ENQUOTE_NAME");
    if is_dynamic_sql_line(up) && up.contains("||") && !asserted {
        push_plsql(
            out,
            ProceduralSqlViolation::ExecuteImmediateInjection,
            ln,
            String::from(
                "dynamic PL/SQL concatenation (`||`) risks SQL injection; use `:1` + `USING` or `DBMS_ASSERT`",
            ),
        );
    }
    if up.contains("FETCH ") && up.contains("BULK COLLECT") && !up.contains("LIMIT ") {
        push_plsql(
            out,
            ProceduralSqlViolation::BulkCollectNoLimit,
            ln,
            String::from("`FETCH ... BULK COLLECT INTO` without `LIMIT <n>` risks PGA exhaustion"),
        );
    }
}

fn is_row_dml(up: &str) -> bool {
    let t = up.trim_start();
    (t.starts_with("INSERT ")
        || t.starts_with("UPDATE ")
        || t.starts_with("DELETE ")
        || t.starts_with("MERGE "))
        && !t.starts_with("FORALL ")
}

fn collect_when_others_body(lines: &[&str], idx: usize, first_up: String) -> String {
    let mut handler = first_up;
    for next in lines.iter().skip(idx + 1) {
        let nup = next
            .split("--")
            .next()
            .unwrap_or(next)
            .trim()
            .to_ascii_uppercase();
        if nup.starts_with("WHEN ") || nup.starts_with("END;") || nup.starts_with("END ") {
            break;
        }
        handler.push(' ');
        handler.push_str(&nup);
    }
    handler
}

fn check_when_others(unit: &PlSqlUnit, out: &mut Vec<LangRuleFinding>) {
    let lines: Vec<&str> = unit.text.lines().collect();
    for (idx, line) in lines.iter().enumerate() {
        let up = line.split("--").next().unwrap_or(line).to_ascii_uppercase();
        if !up.contains("WHEN OTHERS") {
            continue;
        }
        let handler = collect_when_others_body(&lines, idx, up);
        let reraises = ["RAISE;", "RAISE ", "RAISE_APPLICATION_ERROR"]
            .iter()
            .any(|k| handler.contains(k));
        if !reraises {
            let ln = unit
                .start_line
                .saturating_add(u32::try_from(idx).unwrap_or(0));
            push_plsql(
                out,
                ProceduralSqlViolation::SwallowedWhenOthers,
                ln,
                String::from(
                    "`WHEN OTHERS THEN` swallows exceptions without `RAISE;` or `RAISE_APPLICATION_ERROR`",
                ),
            );
        }
    }
}

fn extract_cursor_name(up: &str, kw: &str) -> Option<String> {
    let pos = up.find(kw)?;
    let rest = up[pos + kw.len()..].trim_start();
    let name: String = rest
        .chars()
        .take_while(|c| c.is_ascii_alphanumeric() || *c == '_')
        .collect();
    (!name.is_empty()).then(|| name.to_ascii_lowercase())
}

struct BodyScanState {
    loop_depth: u32,
    prev_forall: bool,
    trigger_no_auto: bool,
    opened: BTreeMap<String, u32>,
    closed: std::collections::BTreeSet<String>,
}

fn track_cursor_line(up: &str, ln: u32, st: &mut BodyScanState) {
    if up.starts_with("OPEN ")
        && !up.contains(" FOR ")
        && let Some(cur) = extract_cursor_name(up, "OPEN ")
    {
        st.opened.entry(cur).or_insert(ln);
    }
    if up.starts_with("CLOSE ")
        && let Some(cur) = extract_cursor_name(up, "CLOSE ")
    {
        st.closed.insert(cur);
    }
}

fn check_loop_dml_or_tx(up: &str, ln: u32, st: &BodyScanState, out: &mut Vec<LangRuleFinding>) {
    if st.loop_depth > 0 && !st.prev_forall && is_row_dml(up) {
        let msg = String::from("row-by-row DML inside `LOOP`; use `BULK COLLECT` + `FORALL`");
        push_plsql(out, ProceduralSqlViolation::DmlInLoop, ln, msg);
    }
    let is_tx = up.starts_with("COMMIT;") || up == "COMMIT" || up.starts_with("ROLLBACK;");
    if (st.loop_depth > 0 || st.trigger_no_auto) && is_tx {
        let msg = String::from("avoid `COMMIT`/`ROLLBACK` in loops or non-autonomous triggers");
        push_plsql(out, ProceduralSqlViolation::CommitInLoop, ln, msg);
    }
}

fn step_plsql_body_line(up: &str, ln: u32, st: &mut BodyScanState, out: &mut Vec<LangRuleFinding>) {
    if up.starts_with("END LOOP") {
        st.loop_depth = st.loop_depth.saturating_sub(1);
        return;
    }
    check_line_dynamic_sql_and_bulk(up, ln, out);
    check_loop_dml_or_tx(up, ln, st, out);
    track_cursor_line(up, ln, st);
    st.prev_forall = up.starts_with("FORALL ");
    if up.ends_with(" LOOP") || up == "LOOP" || up.contains(" LOOP ") {
        st.loop_depth = st.loop_depth.saturating_add(1);
    }
}

fn check_unit_body_lines(unit: &PlSqlUnit, out: &mut Vec<LangRuleFinding>) {
    let trigger_no_auto = unit.kind == PlSqlUnitKind::Trigger
        && !unit
            .text
            .to_ascii_uppercase()
            .contains("PRAGMA AUTONOMOUS_TRANSACTION");
    let mut st = BodyScanState {
        loop_depth: 0,
        prev_forall: false,
        trigger_no_auto,
        opened: BTreeMap::new(),
        closed: std::collections::BTreeSet::new(),
    };
    for (idx, line) in unit.text.lines().enumerate() {
        let ln = unit
            .start_line
            .saturating_add(u32::try_from(idx).unwrap_or(0));
        let up = line
            .split("--")
            .next()
            .unwrap_or(line)
            .trim()
            .to_ascii_uppercase();
        step_plsql_body_line(&up, ln, &mut st, out);
    }
    for (cur, ln) in st.opened {
        if !st.closed.contains(&cur) {
            push_plsql(
                out,
                ProceduralSqlViolation::UnclosedCursor,
                ln,
                format!("explicit cursor `{cur}` opened without a matching `CLOSE {cur};`"),
            );
        }
    }
}

/// Evaluates all Oracle PL/SQL local procedural rules (`PLSQL-*`) on parsed units.
pub fn analyze_plsql_units(units: &[PlSqlUnit], out: &mut Vec<LangRuleFinding>) {
    for unit in units {
        check_authid_and_autonomous(unit, out);
        check_when_others(unit, out);
        check_unit_body_lines(unit, out);
    }
}
