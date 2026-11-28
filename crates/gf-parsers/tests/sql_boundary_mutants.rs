//! Boundary mutation tests for SQL universal AST rules, dialect detection,
//! D1/SQLite destructive DDL tracking, Drizzle config URLs, and embedded SQL bridges.

use gf_core::{Config, Diagnostic, RepoPath, SqlDialectKind, SqlRole};
use gf_parsers::parse_source;
use gf_rules::evaluate_document;

type Res = Result<(), Box<dyn std::error::Error>>;

fn eval_sql(path: &str, src: &str) -> Result<Vec<Diagnostic>, Box<dyn std::error::Error>> {
    let cfg = Config::strict_defaults();
    let doc = parse_source(&RepoPath::new(path)?, src)?;
    Ok(evaluate_document(&doc, &cfg))
}

#[test]
fn universal_sql_tautologies_views_and_migration_select_star_boundaries() -> Res {
    assert_eq!(
        eval_sql("queries/a.sql", "UPDATE u SET a = 1 WHERE false = false;")?.len(),
        1
    );
    assert_eq!(
        eval_sql(
            "queries/a.sql",
            "UPDATE u SET a = 1 WHERE 1 = 1 AND id = 7;"
        )?,
        Vec::new()
    );
    let union_paren = "(SELECT * FROM u) UNION ALL (SELECT id FROM u);";
    assert_eq!(eval_sql("queries/u.sql", union_paren)?.len(), 1);

    let view_diags = eval_sql(
        "migrations/001_v.sql",
        "CREATE VIEW v_u AS SELECT * FROM u;",
    )?;
    assert_eq!(view_diags.len(), 1);
    assert!(view_diags[0].message.contains("`CREATE VIEW`"));

    assert_eq!(
        eval_sql("migrations/001_q.sql", "SELECT * FROM u;")?,
        Vec::new()
    );
    assert_eq!(
        eval_sql(
            "migrations/001_s.sql",
            "-- garde-dialect: sqlite\nCREATE INDEX idx_u ON u (email);\n"
        )?,
        Vec::new()
    );
    assert_eq!(
        eval_sql("queries/idx.sql", "CREATE INDEX idx_u ON u (email);")?,
        Vec::new()
    );
    Ok(())
}

#[test]
fn d1_sqlite_and_destructive_ddl_tx_commit_rollback_boundaries() -> Res {
    let d1_rb = eval_sql("migrations/001_rb.sql", "-- garde-dialect: d1\nROLLBACK;\n")?;
    assert_eq!(d1_rb.len(), 1);
    assert!(d1_rb[0].message.contains("MIG-D1-FORBID-TRANSACTION"));

    let sq_alter = eval_sql(
        "migrations/001_ai.sql",
        "-- garde-dialect: sqlite\nALTER TABLE u ADD COLUMN id INTEGER PRIMARY KEY AUTOINCREMENT;\n",
    )?;
    assert_eq!(sq_alter.len(), 1);
    assert!(
        sq_alter[0]
            .message
            .contains("MIG-SQLITE-AUTOINCREMENT-TRAP")
    );

    assert_eq!(
        eval_sql(
            "migrations/001_ren.sql",
            "ALTER TABLE old_u RENAME TO new_u;"
        )?
        .len(),
        1
    );
    assert_eq!(
        eval_sql(
            "migrations/001_my.sql",
            "-- garde-dialect: mysql\nRENAME TABLE old_u TO new_u;\n"
        )?
        .len(),
        1
    );

    let rb_drop = "BEGIN; CREATE TABLE t (id INT); ROLLBACK; DROP TABLE t;";
    assert_eq!(eval_sql("migrations/001_tx1.sql", rb_drop)?.len(), 1);

    let commit_then_rb = "BEGIN; CREATE TABLE t (id INT); COMMIT; ROLLBACK; DROP TABLE t;";
    assert_eq!(
        eval_sql("migrations/001_tx2.sql", commit_then_rb)?,
        Vec::new()
    );

    let fresh_ops =
        "CREATE TABLE f (id INT, old INT); ALTER TABLE f DROP COLUMN old; DROP TABLE f;";
    assert_eq!(eval_sql("migrations/001_fr.sql", fresh_ops)?, Vec::new());
    Ok(())
}

struct MigrationRoleCase {
    path: &'static str,
    exp_ver: Option<&'static str>,
    exp_stem: &'static str,
    exp_down: bool,
}

const MIGRATION_ROLE_CASES: [MigrationRoleCase; 5] = [
    MigrationRoleCase {
        path: "migrations/up.sql",
        exp_ver: None,
        exp_stem: "migration",
        exp_down: false,
    },
    MigrationRoleCase {
        path: "migrations/migration.sql",
        exp_ver: None,
        exp_stem: "migration",
        exp_down: false,
    },
    MigrationRoleCase {
        path: "migrations/0001_users.up.sql",
        exp_ver: Some("0001"),
        exp_stem: "0001_users",
        exp_down: false,
    },
    MigrationRoleCase {
        path: "sql/custom.up.sql",
        exp_ver: None,
        exp_stem: "custom",
        exp_down: false,
    },
    MigrationRoleCase {
        path: "sql/custom.down.sql",
        exp_ver: None,
        exp_stem: "custom",
        exp_down: true,
    },
];

#[test]
fn dialect_role_suffixes_sqlite_signatures_and_suppression_spans() -> Res {
    for case in MIGRATION_ROLE_CASES {
        let doc = parse_source(&RepoPath::new(case.path)?, "SELECT 1;")?;
        let Some(sql_doc) = doc.sql_doc else {
            return Err("missing sql_doc".into());
        };
        assert_eq!(
            sql_doc.role,
            SqlRole::Migration {
                version_prefix: case.exp_ver.map(String::from),
                migration_stem: String::from(case.exp_stem),
                is_down: case.exp_down,
            }
        );
    }

    for sig in [
        "CREATE TABLE t (k TEXT PRIMARY KEY) WITHOUT ROWID;",
        "CREATE TABLE t (id INTEGER PRIMARY KEY AUTOINCREMENT);",
        "CREATE TABLE t (id INT) STRICT;",
        "CREATE TABLE t (id INT) STRICT\n",
    ] {
        let doc = parse_source(&RepoPath::new("queries/s.sql")?, sig)?;
        assert_eq!(doc.sql_doc.map(|d| d.dialect), Some(SqlDialectKind::Sqlite));
    }

    for bad_sup in [
        "\n\n-- squawk-ignore -- long valid reason text\n",
        "\n\n-- squawk-ignore all -- long valid reason text\n",
        "\n\n-- squawk-ignore * -- long valid reason text\n",
        "\n\n-- squawk-ignore ban-drop-table -- short\n",
    ] {
        let diags = eval_sql("migrations/001.sql", bad_sup)?;
        assert_eq!(diags.len(), 1);
        assert_eq!(diags[0].rule_id, "GF070");
        assert_eq!(diags[0].span.start_line(), 3);
    }
    Ok(())
}

#[test]
fn drizzle_url_and_embedded_sql_interpolation_false_positive_guards() -> Res {
    let gf81 = |src: &str| -> Result<usize, Box<dyn std::error::Error>> {
        let d = eval_sql("drizzle.config.ts", src)?;
        Ok(d.iter().filter(|x| x.rule_id == "GF081").count())
    };
    assert_eq!(
        gf81("export default { url: 'postgres://localhost:5432/app' };")?,
        0
    );
    assert_eq!(
        gf81("export default { url: `postgres://u:${import.meta.env.PW}@db/app` };")?,
        0
    );
    assert_eq!(
        gf81("export default { url: 'libsql://x.turso.io?authToken=secret' };")?,
        1
    );
    assert_eq!(
        gf81("export default { url: 'mysql://localhost/db?password=secret' };")?,
        1
    );

    let inj_count = |path: &str, src: &str| -> Result<usize, Box<dyn std::error::Error>> {
        let d = eval_sql(path, src)?;
        Ok(d.iter()
            .filter(|x| x.message.contains("SQL-INJECTION-INTERPOLATION"))
            .count())
    };
    assert_eq!(
        inj_count(
            "src/a.ts",
            "export const f = (db: unknown, x: string) => (db as { execute: (s: string) => void }).execute(`hello ${x}`);"
        )?,
        0
    );
    assert_eq!(
        inj_count(
            "src/a.ts",
            "export const f = (x: string) => client.execute('hello ' + x) ?? client.execute(('SELECT * FROM t WHERE id = ' + x));"
        )?,
        1
    );
    assert_eq!(
        inj_count(
            "src/a.py",
            "def f(cur: object, x: str) -> None:\n    cur.execute(f'hello {x}')\n    cur.execute('hello {}'.format(x))\n    cur.execute('hello %s' % x)\n    cur.execute('SELECT 1' + ' WHERE 1=1')\n    cur.execute((f'SELECT * FROM t WHERE id = {x}'))\n"
        )?,
        1
    );
    Ok(())
}
