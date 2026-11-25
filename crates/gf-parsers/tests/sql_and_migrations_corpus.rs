//! Symmetric industrial corpus for multi-dialect SQL semantics (`GF000`, `GF070`, `SQL-*`)
//! and `PostgreSQL` zero-downtime migrations (`squawk`-grade `MIG-PG-*`).

use gf_core::{Config, RepoPath};
use gf_parsers::parse_source;
use gf_rules::evaluate_document;

type Res = Result<(), Box<dyn std::error::Error>>;
type DiagRow = (String, u32, String);

fn eval_codes(path: &str, src: &str) -> Result<Vec<DiagRow>, Box<dyn std::error::Error>> {
    let doc = parse_source(&RepoPath::new(path)?, src)?;
    Ok(evaluate_document(&doc, &Config::strict_defaults())
        .into_iter()
        .map(|d| (d.rule_id, d.span.start_line(), d.message))
        .collect())
}

#[test]
fn universal_sql_clean_queries_and_plpgsql_dollar_quotes_pass() -> Res {
    let clean_queries = concat!(
        "WITH raw_users AS (SELECT * FROM users WHERE deleted_at IS NULL)\n",
        "SELECT id, email, COUNT(*) OVER () AS total\n",
        "FROM raw_users u\n",
        "WHERE u.status IS DISTINCT FROM NULL\n",
        "  AND NOT EXISTS (SELECT * FROM bans b WHERE b.user_id = u.id);\n",
        "UPDATE users SET deleted_at = NULL WHERE id = $1;\n",
        "DELETE FROM sessions WHERE expires_at < CURRENT_TIMESTAMP;\n",
        "-- squawk-ignore require-concurrent-index-creation -- offline bootstrap table before traffic\n",
    );
    assert_eq!(eval_codes("sql/users.sql", clean_queries)?, Vec::new());

    let plpgsql = concat!(
        "CREATE OR REPLACE FUNCTION touch_updated_at() RETURNS trigger LANGUAGE plpgsql AS $fn$\n",
        "BEGIN\n",
        "  NEW.updated_at := CURRENT_TIMESTAMP;\n",
        "  RETURN NEW;\n",
        "END;\n",
        "$fn$;\n",
    );
    assert_eq!(eval_codes("sql/trigger_fn.sql", plpgsql)?, Vec::new());
    Ok(())
}

#[test]
fn universal_sql_dirty_queries_bypasses_and_syntax_errors_are_flagged() -> Res {
    let bad_queries = concat!(
        "SELECT * FROM users WHERE deleted_at = NULL OR role <> NULL;\n",
        "SELECT id FROM orders WHERE user_id NOT IN (SELECT id FROM banned_users);\n",
        "UPDATE accounts SET balance = 0;\n",
        "DELETE FROM audit_logs WHERE 1 = 1;\n",
        "DELETE FROM temp_tokens WHERE NOT FALSE;\n",
        "CREATE MATERIALIZED VIEW mv_all AS SELECT u.* FROM users u;\n",
        "-- squawk-ignore\n",
        "-- sqlfluff:disable AL09\n",
    );
    let diags = eval_codes("sql/bad_queries.sql", bad_queries)?;
    for expected in [
        "SQL-SELECT-STAR",
        "SQL-NULL-COMPARISON",
        "SQL-NOT-IN-SUBQUERY",
        "SQL-UNBOUNDED-MUTATION",
    ] {
        assert!(
            diags
                .iter()
                .any(|d| d.0 == "GF080" && d.2.contains(expected)),
            "missing {expected} in {diags:?}"
        );
    }
    assert_eq!(
        diags
            .iter()
            .filter(|d| d.2.contains("SQL-UNBOUNDED-MUTATION"))
            .count(),
        3
    );
    assert_eq!(diags.iter().filter(|d| d.0 == "GF070").count(), 2);

    let syn = eval_codes(
        "sql/broken.sql",
        "SELECT id FROM users;\nSELECT FROM WHERE ;\n",
    )?;
    assert!(syn.iter().any(|d| d.0 == "GF000" && d.1 == 2), "{syn:?}");
    Ok(())
}

#[test]
fn postgres_zero_downtime_safe_online_migration_passes_cleanly() -> Res {
    let safe_mig = concat!(
        "CREATE TABLE audit_events (\n",
        "  id BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,\n",
        "  actor_id BIGINT NOT NULL,\n",
        "  created_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP\n",
        ");\n",
        "CREATE INDEX idx_audit_actor ON audit_events (actor_id);\n",
        "CREATE UNIQUE INDEX CONCURRENTLY idx_users_handle ON users (handle);\n",
        "DROP INDEX CONCURRENTLY IF EXISTS idx_users_legacy;\n",
        "REINDEX INDEX CONCURRENTLY idx_users_handle;\n",
        "ALTER TABLE users ADD COLUMN status TEXT NOT NULL DEFAULT 'active';\n",
        "ALTER TABLE users ADD COLUMN synced_at TIMESTAMPTZ NOT NULL DEFAULT now();\n",
        "ALTER TABLE users ADD CONSTRAINT users_UniqueHandle UNIQUE USING INDEX idx_users_handle;\n",
        "ALTER TABLE users ADD CONSTRAINT users_org_fk FOREIGN KEY (org_id) REFERENCES orgs(id) NOT VALID;\n",
        "ALTER TABLE users ADD CONSTRAINT users_nick_nn CHECK (nickname IS NOT NULL) NOT VALID;\n",
        "ALTER TABLE users VALIDATE CONSTRAINT users_nick_nn;\n",
        "ALTER TABLE users ALTER COLUMN nickname SET NOT NULL;\n",
        "ALTER TABLE users ALTER COLUMN bio TYPE VARCHAR(100);\n",
        "ALTER TABLE users ALTER COLUMN bio TYPE VARCHAR(250);\n",
        "ALTER TABLE users ALTER COLUMN bio TYPE TEXT;\n",
    );
    assert_eq!(
        eval_codes("migrations/0002_safe_online.sql", safe_mig)?,
        Vec::new()
    );
    Ok(())
}

#[test]
fn postgres_zero_downtime_unsafe_index_and_tx_locks_are_caught() -> Res {
    let unsafe_idx = concat!(
        "BEGIN;\n",
        "CREATE TABLE rolled_back_tbl (id BIGINT PRIMARY KEY);\n",
        "ROLLBACK;\n",
        "CREATE INDEX idx_rolled_back ON rolled_back_tbl (id);\n",
        "DROP INDEX idx_old_blocking;\n",
        "BEGIN;\n",
        "CREATE INDEX CONCURRENTLY idx_in_tx ON users (email);\n",
        "DROP INDEX CONCURRENTLY idx_drop_in_tx;\n",
        "REINDEX INDEX CONCURRENTLY idx_reidx_in_tx;\n",
        "ALTER TABLE users ADD CONSTRAINT chk_same_tx CHECK (age > 0) NOT VALID;\n",
        "ALTER TABLE users VALIDATE CONSTRAINT chk_same_tx;\n",
        "COMMIT;\n",
    );
    let diags = eval_codes("migrations/0003_unsafe_idx.sql", unsafe_idx)?;
    assert_eq!(
        diags
            .iter()
            .filter(|d| d.2.contains("MIG-PG-CONCURRENT-INDEX"))
            .count(),
        2
    );
    assert_eq!(
        diags
            .iter()
            .filter(|d| d.2.contains("MIG-PG-CONCURRENTLY-IN-TX"))
            .count(),
        3
    );
    assert_eq!(
        diags
            .iter()
            .filter(|d| d.2.contains("MIG-PG-CONSTRAINT-NOT-VALID"))
            .count(),
        1
    );
    Ok(())
}

#[test]
fn postgres_zero_downtime_unsafe_alter_table_rewrites_are_caught() -> Res {
    let unsafe_alter = concat!(
        "ALTER TABLE users ADD COLUMN bad_nn TEXT NOT NULL;\n",
        "ALTER TABLE users ADD COLUMN bad_vol UUID NOT NULL DEFAULT gen_random_uuid();\n",
        "ALTER TABLE users ADD COLUMN bad_ser SERIAL;\n",
        "ALTER TABLE users ALTER COLUMN raw_col SET NOT NULL;\n",
        "ALTER TABLE users ADD CONSTRAINT fk_lock FOREIGN KEY (team_id) REFERENCES teams(id);\n",
        "ALTER TABLE users ADD CONSTRAINT chk_lock CHECK (score >= 0);\n",
        "ALTER TABLE users ADD CONSTRAINT uq_lock UNIQUE (email);\n",
        "ALTER TABLE users ALTER COLUMN code TYPE VARCHAR(100);\n",
        "ALTER TABLE users ALTER COLUMN code TYPE VARCHAR(50);\n",
        "ALTER TABLE users ALTER COLUMN legacy_flag TYPE BIGINT USING legacy_flag::bigint;\n",
        "ALTER TABLE users ADD COLUMN country CHAR(2), ADD COLUMN naive_ts TIMESTAMP;\n",
    );
    let diags = eval_codes("migrations/0004_unsafe_alter.sql", unsafe_alter)?;
    assert_eq!(
        diags
            .iter()
            .filter(|d| d.2.contains("MIG-PG-ADD-NOT-NULL-UNSAFE"))
            .count(),
        4
    );
    assert_eq!(
        diags
            .iter()
            .filter(|d| d.2.contains("MIG-PG-CONSTRAINT-NOT-VALID"))
            .count(),
        3
    );
    assert_eq!(
        diags
            .iter()
            .filter(|d| d.2.contains("MIG-PG-ALTER-COLUMN-TYPE"))
            .count(),
        2
    );
    assert_eq!(
        diags
            .iter()
            .filter(|d| d.2.contains("MIG-PG-BAN-TYPES"))
            .count(),
        2
    );
    Ok(())
}
