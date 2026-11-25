//! Boundary mutation tests for PostgreSQL zero-downtime defaults/types/rollback
//! and SQLite/D1 `CREATE TRIGGER ... BEGIN ... CASE ... END;` token splitting.

use gf_core::{Config, Diagnostic, RepoPath};
use gf_parsers::parse_source;
use gf_rules::evaluate_document;

type Res = Result<(), Box<dyn std::error::Error>>;

fn eval_sql(path: &str, src: &str) -> Result<Vec<Diagnostic>, Box<dyn std::error::Error>> {
    let cfg = Config::strict_defaults();
    let doc = parse_source(&RepoPath::new(path)?, src)?;
    Ok(evaluate_document(&doc, &cfg))
}

#[test]
fn pg_commit_down_migration_reindex_and_new_table_constraint_boundaries() -> Res {
    let tx_ok = concat!(
        "BEGIN;\n",
        "ALTER TABLE u ADD CONSTRAINT fk_org FOREIGN KEY (org_id) REFERENCES orgs(id) NOT VALID;\n",
        "COMMIT;\n",
        "ALTER TABLE u VALIDATE CONSTRAINT fk_org;\n",
        "CREATE INDEX CONCURRENTLY idx_u_org ON u (org_id);\n",
    );
    assert_eq!(eval_sql("migrations/001_tx.sql", tx_ok)?, Vec::new());

    let new_tbl_fk = concat!(
        "CREATE TABLE fresh (id BIGINT PRIMARY KEY, org_id BIGINT);\n",
        "ALTER TABLE fresh ADD CONSTRAINT fk_f FOREIGN KEY (org_id) REFERENCES orgs(id);\n",
    );
    assert_eq!(eval_sql("migrations/002_new.sql", new_tbl_fk)?, Vec::new());

    let down_sql = concat!(
        "CREATE TABLE legacy (ts TIMESTAMP, c CHAR(4));\n",
        "DROP INDEX idx_legacy;\n",
        "ALTER TABLE users ADD COLUMN bad INT NOT NULL;\n",
    );
    assert_eq!(
        eval_sql("migrations/003_rev.down.sql", down_sql)?,
        Vec::new()
    );

    let reindex_ok =
        "BEGIN;\nREINDEX TABLE users;\nREINDEX users;\nREINDEX INDEX \"CONCURRENTLY\";\nCOMMIT;\n";
    assert_eq!(
        eval_sql("migrations/004_reidx.sql", reindex_ok)?,
        Vec::new()
    );

    let drop_conc_span = eval_sql(
        "migrations/005_drop_conc.sql",
        "BEGIN;\n\nDROP INDEX CONCURRENTLY idx_u;\nCOMMIT;\n",
    )?;
    assert_eq!(drop_conc_span.len(), 1);
    assert_eq!(drop_conc_span[0].span.start_line(), 3);
    Ok(())
}

#[test]
fn pg_non_volatile_defaults_bpchar_and_stored_generated_columns() -> Res {
    let safe_defaults = concat!(
        "ALTER TABLE u ADD COLUMN a INT NOT NULL DEFAULT (-1);\n",
        "ALTER TABLE u ADD COLUMN b TEXT NOT NULL DEFAULT ('ok'::text);\n",
        "ALTER TABLE u ADD COLUMN c INT NOT NULL DEFAULT (1 + 2);\n",
        "ALTER TABLE u ADD COLUMN d DATE NOT NULL DEFAULT DATE '2026-01-01';\n",
        "ALTER TABLE u ADD COLUMN e TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP;\n",
        "ALTER TABLE u ADD COLUMN f TEXT NOT NULL DEFAULT lower('ABC');\n",
    );
    assert_eq!(
        eval_sql("migrations/006_safe.sql", safe_defaults)?,
        Vec::new()
    );

    for unsafe_sql in [
        "ALTER TABLE u ADD COLUMN x TEXT NOT NULL DEFAULT coalesce(gen_random_uuid()::text, 'a');",
        "ALTER TABLE u ADD COLUMN y DOUBLE PRECISION NOT NULL DEFAULT (1.0 + random());",
        "ALTER TABLE u ADD COLUMN z TEXT GENERATED ALWAYS AS (first_name || ' ' || last_name) STORED;",
    ] {
        let diags = eval_sql("migrations/007_unsafe.sql", unsafe_sql)?;
        assert_eq!(diags.len(), 1, "{unsafe_sql} -> {diags:?}");
        assert!(diags[0].message.contains("MIG-PG-ADD-NOT-NULL-UNSAFE"));
    }

    let bpchar_diags = eval_sql(
        "migrations/008_bpchar.sql",
        "ALTER TABLE u ADD COLUMN code public.bpchar;",
    )?;
    assert_eq!(bpchar_diags.len(), 1, "{bpchar_diags:?}");
    assert!(bpchar_diags[0].message.contains("MIG-PG-BAN-TYPES"));
    Ok(())
}

#[test]
fn sqlite_trigger_case_end_and_quoted_trigger_keyword_splitting() -> Res {
    let trigger_then_err = concat!(
        "-- garde-dialect: sqlite\n",
        "CREATE TRIGGER tr_audit AFTER UPDATE ON accounts BEGIN\n",
        "  SELECT CASE WHEN NEW.balance < 0 THEN 1 ELSE 0 END;\n",
        "  UPDATE accounts SET flag = 1 WHERE id = NULL;\n",
        "END;\n",
        "CREATE TABLE broken_after_trigger (;\n",
    );
    let diags = eval_sql("migrations/009_trig.sql", trigger_then_err)?;
    assert_eq!(diags.iter().filter(|d| d.rule_id == "GF000").count(), 1);
    assert_eq!(
        diags
            .iter()
            .filter(|d| d.message.contains("SQL-NULL-COMPARISON"))
            .count(),
        1
    );

    let unclosed_case_then_upd = "SELECT CASE WHEN 1;\nUPDATE accounts SET flag = 1;\n";
    let case_diags = eval_sql("queries/q.sql", unclosed_case_then_upd)?;
    assert_eq!(
        case_diags
            .iter()
            .filter(|d| d.message.contains("SQL-UNBOUNDED-MUTATION"))
            .count(),
        1
    );

    let drop_trig_then_conc = concat!(
        "DROP TRIGGER tr_old ON accounts;\n",
        "BEGIN;\n",
        "DROP INDEX CONCURRENTLY idx_acc;\n",
        "COMMIT;\n",
    );
    let dt_diags = eval_sql("migrations/010_drop_trig.sql", drop_trig_then_conc)?;
    assert_eq!(
        dt_diags
            .iter()
            .filter(|d| d.message.contains("MIG-PG-CONCURRENTLY-IN-TX"))
            .count(),
        1
    );
    Ok(())
}
