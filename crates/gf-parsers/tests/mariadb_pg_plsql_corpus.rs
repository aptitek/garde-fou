//! Symmetric `(invalid, subtle_valid)` corpus for MariaDB 10.11/11.4, advanced PostgreSQL/PL/pgSQL,
//! Oracle PL/SQL (`segmenter`, `plsql_ast`, `plsql_checks`), and `zpa-cli` Generic Issue JSON (`GF080`/`GF082`).

use std::path::Path;

use gf_core::{
    ExternalToolKind, MariaDbViolation, PostgresAdvViolation, ProceduralSqlViolation, RepoPath,
};
use gf_parsers::{parse_source, parse_zpa_generic_issues_json};

const PG_BAD: &str = "\
-- garde-dialect: postgres
CREATE FUNCTION admin_reset(p_org uuid, p_tbl text, p_id int)
RETURNS void
LANGUAGE plpgsql
SECURITY DEFINER
AS $$
DECLARE
  v_email text;
BEGIN
  SELECT email INTO v_email FROM app.users WHERE org_id = p_org;
  EXECUTE format('SELECT * FROM %s WHERE id = %s', p_tbl, p_id);
END;
$$;
";

const PG_GOOD: &str = "\
-- garde-dialect: postgres
CREATE FUNCTION admin_reset(p_org uuid, p_tbl text, p_id int)
RETURNS void
LANGUAGE plpgsql
SECURITY DEFINER
SET search_path = pg_catalog, pg_temp
AS $$
DECLARE
  v_email text;
BEGIN
  SELECT email INTO STRICT v_email FROM app.users WHERE id = p_id;
  EXECUTE format('SELECT id FROM %I WHERE id = $1', p_tbl) USING p_id;
END;
$$;
";

const PLSQL_BAD: &str = "\
CREATE OR REPLACE PACKAGE emp_pkg AS
  PROCEDURE run_batch(p_dept IN NUMBER);
END emp_pkg;
/
CREATE OR REPLACE PACKAGE BODY emp_pkg AS
  PROCEDURE run_batch(p_dept IN NUMBER) IS
    PRAGMA AUTONOMOUS_TRANSACTION;
    CURSOR c IS SELECT id FROM emp;
  BEGIN
    OPEN c;
    FETCH c BULK COLLECT INTO l_rows;
    EXECUTE IMMEDIATE 'DELETE FROM emp WHERE dept_id = ' || p_dept;
    FOR r IN c LOOP
      UPDATE emp SET sal = sal * 1.1 WHERE id = r.id;
      COMMIT;
    END LOOP;
  EXCEPTION
    WHEN OTHERS THEN
      DBMS_OUTPUT.PUT_LINE('oups');
  END run_batch;
END emp_pkg;
/
";

const PLSQL_GOOD: &str = "\
CREATE OR REPLACE PACKAGE emp_pkg AUTHID CURRENT_USER AS
  PROCEDURE run_batch(p_id IN NUMBER, p_col IN VARCHAR2);
END emp_pkg;
/
CREATE OR REPLACE PACKAGE BODY emp_pkg AS
  PROCEDURE run_batch(p_id IN NUMBER, p_col IN VARCHAR2) IS
    v_ratio NUMBER := v_total / v_count;
    v_txt VARCHAR2(64) := q'[select 'a' / 'b' from dual]';
  BEGIN
    FETCH c BULK COLLECT INTO l_ids LIMIT 500;
    FORALL i IN 1..l_ids.COUNT
      UPDATE emp SET sal = sal * 1.1 WHERE id = l_ids(i);
    EXECUTE IMMEDIATE 'DELETE FROM emp WHERE id = :1' USING p_id;
    EXECUTE IMMEDIATE 'SELECT ' || DBMS_ASSERT.SIMPLE_SQL_NAME(p_col) || ' FROM dual';
  EXCEPTION
    WHEN OTHERS THEN
      RAISE;
  END run_batch;
END emp_pkg;
/
";

fn findings(path: &str, src: &str) -> Result<Vec<(String, u32)>, Box<dyn std::error::Error>> {
    let doc = parse_source(&RepoPath::new(path)?, src)?;
    assert_eq!(doc.syntax_errors.len(), 0, "{:?}", doc.syntax_errors);
    Ok(doc
        .lang_rule_findings
        .iter()
        .map(|f| (String::from(f.code()), f.span.start_line()))
        .collect())
}

fn assert_has_all(list: &[(String, u32)], expected: &[(&str, u32)]) {
    for &(code, line) in expected {
        assert!(
            list.iter().any(|(c, l)| c == code && *l == line),
            "missing ({code}, line {line}) in {list:?}"
        );
    }
}

#[test]
fn mariadb_corpus_enforces_implicit_commit_utf8mb3_engine_alter_and_delimiter()
-> Result<(), Box<dyn std::error::Error>> {
    let bad = "-- garde-dialect: mariadb\nSTART TRANSACTION;\nALTER TABLE users ADD COLUMN age INT ALGORITHM=INSTANT;\nUPDATE users SET age = 0 WHERE id = 1;\nCOMMIT;\nCREATE TABLE logs (id BIGINT PRIMARY KEY, msg VARCHAR(200)) ENGINE=MyISAM DEFAULT CHARSET=utf8;\n";
    assert_has_all(
        &findings("migrations/0002_bad.sql", bad)?,
        &[
            (MariaDbViolation::ImplicitDdlCommit.code(), 3),
            (MariaDbViolation::NonAcidEngineAndJson.code(), 6),
            (MariaDbViolation::Utf8mb3Trap.code(), 6),
        ],
    );
    let good = "-- garde-dialect: mariadb\nCREATE TABLE logs (id BIGINT PRIMARY KEY, msg VARCHAR(200)) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_uca1400_ai_ci;\nSTART TRANSACTION;\nUPDATE logs SET msg = 'ok' WHERE id = 1;\nCOMMIT;\n";
    assert_eq!(findings("migrations/0002_good.sql", good)?.len(), 0);
    let proc_sql = "-- garde-dialect: mariadb\nDELIMITER //\nCREATE PROCEDURE bump()\nBEGIN\n  UPDATE users SET age = age + 1 WHERE id = 1;\nEND //\nDELIMITER ;\n";
    assert_eq!(findings("sql/proc.sql", proc_sql)?.len(), 0);
    Ok(())
}

#[test]
fn postgres_security_definer_and_plpgsql_exact_line_corpus()
-> Result<(), Box<dyn std::error::Error>> {
    assert_has_all(
        &findings("sql/admin.sql", PG_BAD)?,
        &[
            (PostgresAdvViolation::SecurityDefinerSearchPath.code(), 5),
            (PostgresAdvViolation::PlpgsqlMissingIntoStrict.code(), 10),
            (PostgresAdvViolation::PlpgsqlUnsafeDynamicExecute.code(), 11),
        ],
    );
    assert_eq!(findings("sql/admin_good.sql", PG_GOOD)?.len(), 0);
    Ok(())
}

#[test]
fn oracle_plsql_corpus_enforces_procedural_rules_and_slash_literals()
-> Result<(), Box<dyn std::error::Error>> {
    assert_has_all(
        &findings("src/emp_pkg.pkb", PLSQL_BAD)?,
        &[
            (ProceduralSqlViolation::MissingAuthidCurrentUser.code(), 1),
            (ProceduralSqlViolation::UnclosedCursor.code(), 10),
            (ProceduralSqlViolation::BulkCollectNoLimit.code(), 11),
            (ProceduralSqlViolation::ExecuteImmediateInjection.code(), 12),
            (ProceduralSqlViolation::DmlInLoop.code(), 14),
            (ProceduralSqlViolation::CommitInLoop.code(), 15),
            (ProceduralSqlViolation::SwallowedWhenOthers.code(), 18),
        ],
    );
    assert_eq!(findings("src/emp_good.pkb", PLSQL_GOOD)?.len(), 0);
    Ok(())
}

#[test]
fn zpa_generic_issues_json_parses_deterministic_findings() -> Result<(), Box<dyn std::error::Error>>
{
    let raw = r#"{"issues":[{"engineId":"zpa","ruleId":"CommitRollback","severity":"MAJOR","type":"BUG","primaryLocation":{"message":"Avoid COMMIT in loop","filePath":"src/emp_pkg.pkb","textRange":{"startLine":15,"startColumn":7}}}]}"#;
    let parsed = parse_zpa_generic_issues_json(raw, Path::new("/ws"))?;
    assert_eq!(parsed.len(), 1);
    assert_eq!(parsed[0].tool, ExternalToolKind::Zpa);
    assert_eq!(parsed[0].native_code, "CommitRollback");
    assert_eq!((parsed[0].line, parsed[0].column), (15, 7));
    assert_eq!(parsed[0].path.as_str(), "src/emp_pkg.pkb");
    Ok(())
}
