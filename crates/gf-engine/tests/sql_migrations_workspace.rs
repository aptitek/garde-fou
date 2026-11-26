//! End-to-end workspace tests for SQL dialect auto-detection (`wrangler.toml` `d1_databases`,
//! `drizzle.config.ts`), `MIG-VERSION-COLLISION-OR-GAP`, and `GF076` TDD / `GF001` exemptions.

use std::fmt::Write as _;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use gf_core::{Config, FeatureId, LanguageId, ScannerPolicy};
use gf_engine::{ScanScope, run_check};

static DIR_SEQ: AtomicU64 = AtomicU64::new(0);

struct TempRepo {
    root: PathBuf,
}

impl TempRepo {
    fn new(prefix: &str) -> Result<Self, Box<dyn std::error::Error>> {
        let id = DIR_SEQ.fetch_add(1, Ordering::Relaxed);
        let root =
            std::env::temp_dir().join(format!("gf-sql-ws-{prefix}-{}-{id}", std::process::id()));
        if root.exists() {
            fs::remove_dir_all(&root)?;
        }
        fs::create_dir_all(&root)?;
        Ok(Self { root })
    }

    fn write(&self, rel: &str, content: &str) -> Result<(), Box<dyn std::error::Error>> {
        let full = self.root.join(rel);
        if let Some(parent) = full.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(full, content)?;
        Ok(())
    }

    fn path(&self) -> &Path {
        &self.root
    }
}

impl Drop for TempRepo {
    fn drop(&mut self) {
        if let Err(_e) = fs::remove_dir_all(&self.root) {}
    }
}

fn offline_config() -> Config {
    let mut cfg = Config::strict_defaults();
    cfg.integrity.require_lock = false;
    cfg.agents.sync_policy = ScannerPolicy::Off;
    cfg.scanners.secrets = ScannerPolicy::Off;
    cfg.scanners.vulns = ScannerPolicy::Off;
    cfg.tools.sql.squawk = ScannerPolicy::Off;
    cfg.tools.sql.sqruff = ScannerPolicy::Off;
    cfg.tools.sql.sqlfluff = ScannerPolicy::Off;
    cfg.tools.toml.taplo = ScannerPolicy::Off;
    cfg
}

#[test]
fn wrangler_d1_and_drizzle_sqlite_context_detection_drive_migration_rules()
-> Result<(), Box<dyn std::error::Error>> {
    let repo = TempRepo::new("d1-ctx")?;
    repo.write(
        "wrangler.toml",
        "name = \"edge-worker\"\nmain = \"src/index.ts\"\ncompatibility_date = \"2025-01-01\"\n\n[[d1_databases]]\nbinding = \"DB\"\ndatabase_name = \"prod\"\ndatabase_id = \"00000000-0000-0000-0000-000000000000\"\n",
    )?;
    repo.write(
        "migrations/0001_init.sql",
        "BEGIN TRANSACTION;\nCREATE TABLE users (id INTEGER PRIMARY KEY, email TEXT NOT NULL);\nCOMMIT;\n",
    )?;

    let cfg = offline_config();
    let rep_d1 = run_check(repo.path(), ScanScope::All, &cfg)?;
    assert!(rep_d1.context.languages.contains(&LanguageId::Sql));
    assert!(
        rep_d1
            .diagnostics
            .iter()
            .any(|d| d.message.contains("MIG-D1-FORBID-TRANSACTION")),
        "expected D1 transaction ban: {:?}",
        rep_d1.diagnostics
    );

    // Removing `d1_databases` switches auto-dialect back to Postgres where `BEGIN ... COMMIT` around CREATE TABLE is valid!
    repo.write(
        "wrangler.toml",
        "name = \"plain-worker\"\ncompatibility_date = \"2025-01-01\"\n",
    )?;
    let rep_pg = run_check(repo.path(), ScanScope::All, &cfg)?;
    assert_ne!(rep_d1.cache_fingerprint, rep_pg.cache_fingerprint);
    assert!(
        rep_pg.diagnostics.is_empty(),
        "unexpected diagnostics without D1: {:?}",
        rep_pg.diagnostics
    );
    Ok(())
}

#[test]
fn workspace_migration_graph_detects_version_collisions_and_orphan_down_files()
-> Result<(), Box<dyn std::error::Error>> {
    let repo = TempRepo::new("mig-graph")?;
    repo.write(
        "migrations/0001_org.up.sql",
        "CREATE TABLE orgs (id BIGINT PRIMARY KEY);\n",
    )?;
    repo.write("migrations/0001_org.down.sql", "DROP TABLE orgs;\n")?;
    repo.write(
        "migrations/0004_users.sql",
        "CREATE TABLE users (id BIGINT PRIMARY KEY);\n",
    )?;
    repo.write(
        "migrations/0004_billing.sql",
        "CREATE TABLE invoices (id BIGINT PRIMARY KEY);\n",
    )?;
    repo.write(
        "migrations/0009_orphan.down.sql",
        "DROP TABLE ghost_table;\n",
    )?;

    let cfg = offline_config();
    let report = run_check(repo.path(), ScanScope::All, &cfg)?;
    let graph_diags: Vec<_> = report
        .diagnostics
        .iter()
        .filter(|d| d.message.contains("MIG-VERSION-COLLISION-OR-GAP"))
        .collect();
    assert_eq!(graph_diags.len(), 2, "{:?}", report.diagnostics);
    assert!(
        graph_diags
            .iter()
            .any(|d| d.path.as_str() == "migrations/0004_users.sql")
    );
    assert!(
        graph_diags
            .iter()
            .any(|d| d.path.as_str() == "migrations/0009_orphan.down.sql")
    );
    Ok(())
}

#[test]
fn tdd_feature_and_file_line_limit_exempt_large_sql_migrations()
-> Result<(), Box<dyn std::error::Error>> {
    let repo = TempRepo::new("tdd-sql")?;
    repo.write(
        "src/math.rs",
        "/// Adds one.\n#[must_use]\npub const fn add_one(x: i32) -> i32 {\n    x + 1\n}\n",
    )?;
    repo.write(
        "tests/math_test.rs",
        "#[test]\nfn adds_one() {\n    assert_eq!(add_one(1), 2);\n}\n",
    )?;
    // Generate a 280-line valid SQL schema migration (> 250 max_file_lines) with NO `.test.sql` companion
    let mut big_sql = String::from("CREATE TABLE big_schema (\n  id BIGINT PRIMARY KEY");
    for i in 1..=275 {
        write!(big_sql, ",\n  col_{i} TEXT NOT NULL DEFAULT ''")?;
    }
    big_sql.push_str("\n);\n");
    repo.write("migrations/0001_big_schema.sql", &big_sql)?;

    let mut cfg = offline_config();
    cfg.project.features.insert(FeatureId::Tdd);
    let report = run_check(repo.path(), ScanScope::All, &cfg)?;
    assert!(
        report.diagnostics.is_empty(),
        "SQL migrations must not trigger GF076 TDD companion or GF001 line cap: {:?}",
        report.diagnostics
    );
    Ok(())
}
