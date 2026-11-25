//! Tests for Cloudflare `D1` / `SQLite` migrations, `MIG-DESTRUCTIVE-DDL`,
//! cross-language embedded SQL injection (`Code`), and SQL Tier 2 (`GF081`) / Tier 3 (`GF082`).

use gf_core::{Config, RepoPath, SqlDialectKind};
use gf_parsers::parse_source_with_context;
use gf_rules::evaluate_document;

type Res = Result<(), Box<dyn std::error::Error>>;
type DiagRow = (String, u32, String);

fn eval_codes_with_hint(
    path: &str,
    src: &str,
    hint: Option<SqlDialectKind>,
    cfg: &Config,
) -> Result<Vec<DiagRow>, Box<dyn std::error::Error>> {
    let doc = parse_source_with_context(&RepoPath::new(path)?, src, &cfg.domains, hint)?;
    Ok(evaluate_document(&doc, cfg)
        .into_iter()
        .map(|d| (d.rule_id, d.span.start_line(), d.message))
        .collect())
}

#[test]
fn d1_and_sqlite_triggers_strict_tables_and_autoincrement_are_exact() -> Res {
    let cfg = Config::strict_defaults();
    let sqlite_clean = concat!(
        "CREATE TABLE kv (\n",
        "  k TEXT NOT NULL PRIMARY KEY,\n",
        "  v BLOB NOT NULL\n",
        ") STRICT, WITHOUT ROWID;\n",
        "CREATE TRIGGER tr_kv_audit AFTER INSERT ON kv BEGIN\n",
        "  INSERT INTO audit_log(k) VALUES (NEW.k);\n",
        "  UPDATE stats SET writes = writes + 1 WHERE name = 'kv';\n",
        "END;\n",
    );
    assert_eq!(
        eval_codes_with_hint("migrations/0001_kv.sql", sqlite_clean, None, &cfg)?,
        Vec::new()
    );

    let d1_bad = concat!(
        "BEGIN TRANSACTION;\n",
        "CREATE TABLE items (id INTEGER PRIMARY KEY AUTOINCREMENT, name TEXT NOT NULL);\n",
        "SAVEPOINT sp1;\n",
        "COMMIT;\n",
    );
    let d1_diags = eval_codes_with_hint(
        "migrations/0002_d1.sql",
        d1_bad,
        Some(SqlDialectKind::CloudflareD1),
        &cfg,
    )?;
    assert_eq!(
        d1_diags
            .iter()
            .filter(|d| d.2.contains("MIG-D1-FORBID-TRANSACTION"))
            .count(),
        3
    );
    assert!(
        d1_diags
            .iter()
            .any(|d| d.2.contains("MIG-SQLITE-AUTOINCREMENT-TRAP"))
    );
    Ok(())
}

#[test]
fn destructive_ddl_honors_new_tables_down_migrations_and_config_toggle() -> Res {
    let cfg = Config::strict_defaults();
    let destr_sql = concat!(
        "-- garde-dialect: postgres\n",
        "CREATE TABLE scratch (id BIGINT PRIMARY KEY);\n",
        "DROP TABLE scratch;\n",
        "DROP TABLE legacy_orders;\n",
        "TRUNCATE sessions;\n",
        "ALTER TABLE users DROP COLUMN old_bio;\n",
        "ALTER TABLE users RENAME COLUMN handle TO username;\n",
    );
    let destr_diags = eval_codes_with_hint("migrations/0003_drop.up.sql", destr_sql, None, &cfg)?;
    assert_eq!(
        destr_diags
            .iter()
            .filter(|d| d.2.contains("MIG-DESTRUCTIVE-DDL"))
            .count(),
        4
    );
    assert_eq!(
        eval_codes_with_hint("migrations/0003_drop.down.sql", destr_sql, None, &cfg)?,
        Vec::new()
    );
    let mut permissive_cfg = Config::strict_defaults();
    permissive_cfg.sql.allow_destructive_migrations = true;
    assert_eq!(
        eval_codes_with_hint(
            "migrations/0003_drop.up.sql",
            destr_sql,
            None,
            &permissive_cfg
        )?,
        Vec::new()
    );
    Ok(())
}

#[test]
fn ts_and_python_embedded_sql_injection_bridge_has_zero_false_positives() -> Res {
    let cfg = Config::strict_defaults();
    let ts_safe = concat!(
        "export async function loadUser(sql: unknown, prisma: any, db: any, id: string): Promise<void> {\n",
        "  await (sql as any)`SELECT id, email FROM users WHERE id = ${id}`;\n",
        "  await prisma.$queryRaw`SELECT id FROM users WHERE id = ${id}`;\n",
        "  db.prepare(`SELECT id FROM users WHERE id = ?`).get(id);\n",
        "  db.query(\"SELECT id FROM users WHERE id = $1\", [id]);\n",
        "}\n",
    );
    let ts_safe_inj: Vec<_> = eval_codes_with_hint("src/repo.ts", ts_safe, None, &cfg)?
        .into_iter()
        .filter(|d| d.2.contains("SQL-INJECTION-INTERPOLATION"))
        .collect();
    assert_eq!(ts_safe_inj, Vec::new());

    let ts_unsafe = concat!(
        "export async function bad(db: any, prisma: any, uid: string): Promise<void> {\n",
        "  await db.query(`SELECT id FROM users WHERE id = '${uid}'`);\n",
        "  await db.execute(\"DELETE FROM sessions WHERE token = \" + uid);\n",
        "  await prisma.$queryRawUnsafe(`SELECT ${uid}`);\n",
        "}\n",
    );
    let ts_diags = eval_codes_with_hint("src/bad_repo.ts", ts_unsafe, None, &cfg)?;
    assert_eq!(
        ts_diags
            .iter()
            .filter(|d| d.2.contains("SQL-INJECTION-INTERPOLATION"))
            .count(),
        3
    );

    let py_src = concat!(
        "def safe_q(cur: object, uid: int) -> None:\n",
        "    cur.execute(\"SELECT id FROM users WHERE id = %s\", (uid,))\n\n",
        "def unsafe_q(cur: object, text: object, uid: str) -> None:\n",
        "    cur.execute(f\"SELECT id FROM users WHERE name = '{uid}'\")\n",
        "    cur.execute(\"DELETE FROM users WHERE id = {}\".format(uid))\n",
        "    text(\"SELECT id FROM users WHERE slug = '%s'\" % uid)\n",
    );
    let py_diags = eval_codes_with_hint("src/repo.py", py_src, None, &cfg)?;
    assert_eq!(
        py_diags
            .iter()
            .filter(|d| d.2.contains("SQL-INJECTION-INTERPOLATION"))
            .count(),
        3
    );
    Ok(())
}

#[test]
fn rust_embedded_sql_injection_bridge_is_exact() -> Res {
    let cfg = Config::strict_defaults();
    let rs_src = concat!(
        "/// Safe and unsafe Rust SQL examples.\n",
        "pub fn demo_queries(pool: &Pool, uid: i64) {\n",
        "    sqlx::query(\"SELECT id FROM users WHERE id = $1\").bind(uid);\n",
        "    sqlx::query!(\"SELECT id FROM users WHERE id = $1\", uid);\n",
        "    sqlx::query(&format!(\"SELECT id FROM users WHERE id = {uid}\"));\n",
        "    pool.execute(format!(\"DELETE FROM sessions WHERE uid = {uid}\"));\n",
        "}\n",
    );
    let rs_diags = eval_codes_with_hint("src/repo.rs", rs_src, None, &cfg)?;
    assert_eq!(
        rs_diags
            .iter()
            .filter(|d| d.2.contains("SQL-INJECTION-INTERPOLATION"))
            .count(),
        2
    );
    Ok(())
}
