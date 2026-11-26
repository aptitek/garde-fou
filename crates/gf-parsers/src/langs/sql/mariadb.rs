//! MariaDB 10.11 / 11.4 DDL, charset, storage engine, and Online DDL analyzer (`MARIADB-*`).

use std::collections::BTreeSet;

use gf_core::{LangRuleFinding, MariaDbViolation, Span};

use super::segmenter::SqlSegment;

/// Role of the analyzed MariaDB file (replaces multiple boolean flags).
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MariaDbMode {
    /// Standalone query or declarative schema.
    Query,
    /// Forward schema migration (online DDL enforced).
    UpMigration,
    /// Rollback migration.
    DownMigration,
}

fn push_mdb(out: &mut Vec<LangRuleFinding>, v: MariaDbViolation, line: u32, msg: String) {
    let sp = Span::new(line.max(1), line.max(1)).unwrap_or(Span::FIRST);
    out.push(LangRuleFinding::new(v, sp, msg));
}

fn first_word_upper(stmt: &str) -> String {
    stmt.split_whitespace()
        .next()
        .unwrap_or("")
        .to_ascii_uppercase()
}

fn extract_table_after(upper: &str, prefix: &str) -> Option<String> {
    let pos = upper.find(prefix)?;
    let mut rest = upper[pos + prefix.len()..].trim_start();
    for skip in ["IF NOT EXISTS ", "IF EXISTS "] {
        if let Some(s) = rest.strip_prefix(skip) {
            rest = s.trim_start();
        }
    }
    let tok: String = rest
        .chars()
        .take_while(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '.' | '`'))
        .filter(|&c| c != '`')
        .collect();
    (!tok.is_empty()).then_some(tok)
}

struct SemicolonBuf {
    cur: String,
    start: Option<u32>,
    out: Vec<(String, u32)>,
}

impl SemicolonBuf {
    fn push_part(&mut self, part: &str, ln: u32) {
        if self.start.is_none() && !part.trim().is_empty() {
            self.start = Some(ln);
        }
        self.cur.push_str(part);
        if part.ends_with(';') {
            if let Some(s) = self.start.take()
                && !self.cur.trim().is_empty()
            {
                self.out.push((core::mem::take(&mut self.cur), s));
            }
            self.cur.clear();
        }
    }
}

fn split_simple_semicolons(seg: &SqlSegment) -> Vec<(String, u32)> {
    let mut buf = SemicolonBuf {
        cur: String::new(),
        start: None,
        out: Vec::new(),
    };
    for (i, line) in seg.text.lines().enumerate() {
        let ln = seg.start_line.saturating_add(u32::try_from(i).unwrap_or(0));
        let clean = line.split("--").next().unwrap_or(line);
        for part in clean.split_inclusive(';') {
            buf.push_part(part, ln);
        }
        if buf.start.is_some() {
            buf.cur.push('\n');
        }
    }
    if let Some(s) = buf.start
        && !buf.cur.trim().is_empty()
    {
        buf.out.push((buf.cur, s));
    }
    buf.out
}

fn has_legacy_utf8(upper: &str, compact: &str) -> bool {
    compact.contains("UTF8MB3")
        || (compact.contains("CHARSET=UTF8") && !compact.contains("CHARSET=UTF8MB4"))
        || (upper.contains("CHARACTER SET UTF8") && !upper.contains("CHARACTER SET UTF8MB4"))
        || (upper.contains("COLLATE UTF8_") && !upper.contains("COLLATE UTF8MB4_"))
}

fn check_utf8mb3_and_engine(upper: &str, ln: u32, out: &mut Vec<LangRuleFinding>) {
    let compact: String = upper.chars().filter(|c| !c.is_whitespace()).collect();
    if has_legacy_utf8(upper, &compact) {
        push_mdb(
            out,
            MariaDbViolation::Utf8mb3Trap,
            ln,
            String::from("legacy 3-byte `utf8`/`utf8mb3` charset/collation; use `utf8mb4`"),
        );
    }
    if ["ENGINE=MYISAM", "ENGINE=MEMORY", "ENGINE=ARIA"]
        .iter()
        .any(|e| compact.contains(e))
    {
        push_mdb(
            out,
            MariaDbViolation::NonAcidEngineAndJson,
            ln,
            String::from("non-ACID MariaDB storage engine; use `ENGINE=InnoDB`"),
        );
    }
    let json_col = ["_JSON", "PAYLOAD", "METADATA"]
        .iter()
        .any(|k| upper.contains(k));
    if upper.contains("NOT ENFORCED")
        || (upper.contains("LONGTEXT") && json_col && !upper.contains("JSON_VALID("))
    {
        push_mdb(
            out,
            MariaDbViolation::NonAcidEngineAndJson,
            ln,
            String::from("keep `CHECK` constraints `ENFORCED` and guard JSON with `JSON_VALID`"),
        );
    }
}

fn check_blocking_alter(
    upper: &str,
    ln: u32,
    created: &BTreeSet<String>,
    out: &mut Vec<LangRuleFinding>,
) {
    let Some(tbl) = extract_table_after(upper, "ALTER TABLE ") else {
        return;
    };
    if created.contains(&tbl) {
        return;
    }
    let compact: String = upper.chars().filter(|c| !c.is_whitespace()).collect();
    let blocking = ["ALGORITHM=COPY", "LOCK=EXCLUSIVE", "LOCK=SHARED"]
        .iter()
        .any(|k| compact.contains(k));
    let modifies = [" ADD ", " MODIFY ", " CHANGE ", " DROP "]
        .iter()
        .any(|k| upper.contains(k));
    let online = compact.contains("ALGORITHM=INSTANT")
        || (compact.contains("ALGORITHM=INPLACE") && compact.contains("LOCK=NONE"));
    if blocking || (modifies && !online) {
        let msg = format!(
            "MariaDB `ALTER TABLE {tbl}` must use `ALGORITHM=INSTANT` or `ALGORITHM=INPLACE, LOCK=NONE`"
        );
        push_mdb(out, MariaDbViolation::BlockingAlterDdl, ln, msg);
    }
}

struct MdbState {
    in_tx: bool,
    mode: MariaDbMode,
    created: BTreeSet<String>,
}

fn step_mariadb_stmt(raw: &str, ln: u32, st: &mut MdbState, out: &mut Vec<LangRuleFinding>) {
    let upper = raw.to_ascii_uppercase();
    let first = first_word_upper(&upper);
    if upper.trim_start().starts_with("START TRANSACTION")
        || (first == "BEGIN" && !upper.contains("BEGIN NOT ATOMIC"))
    {
        st.in_tx = true;
        return;
    }
    if matches!(first.as_str(), "COMMIT" | "ROLLBACK") {
        st.in_tx = false;
        return;
    }
    if st.in_tx
        && matches!(
            first.as_str(),
            "CREATE" | "ALTER" | "DROP" | "TRUNCATE" | "RENAME"
        )
    {
        let msg = format!(
            "MariaDB DDL `{first}` inside an explicit transaction triggers an implicit COMMIT"
        );
        push_mdb(out, MariaDbViolation::ImplicitDdlCommit, ln, msg);
    }
    if let Some(tbl) = extract_table_after(&upper, "CREATE TABLE ") {
        st.created.insert(tbl);
    }
    check_utf8mb3_and_engine(&upper, ln, out);
    if st.mode == MariaDbMode::UpMigration && first == "ALTER" {
        check_blocking_alter(&upper, ln, &st.created, out);
    }
}

/// Evaluates MariaDB 10.11 / 11.4 rules (`MARIADB-*`) across segmented statements.
pub fn analyze_mariadb_segments(
    segments: &[SqlSegment],
    mode: MariaDbMode,
    out: &mut Vec<LangRuleFinding>,
) {
    let mut st = MdbState {
        in_tx: false,
        mode,
        created: BTreeSet::new(),
    };
    for seg in segments {
        let up = seg.text.to_ascii_uppercase();
        let stmts = if up.contains("CREATE PROCEDURE") || up.contains("CREATE FUNCTION") {
            vec![(seg.text.clone(), seg.start_line)]
        } else {
            split_simple_semicolons(seg)
        };
        for (raw, ln) in stmts {
            step_mariadb_stmt(&raw, ln, &mut st, out);
        }
    }
}
