//! Unit & boundary mutation tests for `gf_rules::sql_workspace_checks`
//! (`MIG-DESTRUCTIVE-DDL` and `MIG-VERSION-COLLISION-OR-GAP` -> `GF080`).

use gf_core::{
    Config, DestructiveDdlFinding, Language, ProjectContext, RepoPath, Span, SqlDialectKind,
    SqlDocument, SqlRole, UastDocument,
};
use gf_rules::{evaluate_document, evaluate_workspace_sorted};

type Res = Result<(), Box<dyn std::error::Error>>;

fn mig_doc(
    path: &str,
    ver: Option<&str>,
    stem: &str,
    is_down: bool,
) -> Result<UastDocument, Box<dyn std::error::Error>> {
    let group_dir = path.rsplit_once('/').map_or("", |(dir, _)| dir);
    let mut doc = UastDocument::new(RepoPath::new(path)?, Language::Sql, 10, 5)?;
    doc.sql_doc = Some(SqlDocument {
        dialect: SqlDialectKind::Postgres,
        role: SqlRole::Migration {
            version_prefix: ver.map(String::from),
            migration_stem: String::from(stem),
            is_down,
        },
        migration_group_dir: String::from(group_dir),
        destructive_ddl: Vec::new(),
        rls_enabled_tables: std::collections::BTreeSet::new(),
        rls_forced_tables: std::collections::BTreeSet::new(),
        rls_policies_by_table: std::collections::BTreeMap::new(),
        plsql_package_specs: std::collections::BTreeMap::new(),
        plsql_package_bodies: std::collections::BTreeMap::new(),
    });
    Ok(doc)
}

#[test]
fn local_destructive_ddl_respects_allow_destructive_migrations_flag() -> Res {
    let span = Span::new(4, 4)?;
    let mut doc = mig_doc("migrations/0002_drop.sql", Some("0002"), "0002_drop", false)?;
    if let Some(sql) = &mut doc.sql_doc {
        sql.destructive_ddl.push(DestructiveDdlFinding {
            span,
            detail: String::from("`DROP TABLE legacy_orders` drops an existing table"),
        });
    }

    let strict_cfg = Config::strict_defaults();
    let diags = evaluate_document(&doc, &strict_cfg);
    assert_eq!(diags.len(), 1, "{diags:?}");
    assert_eq!(diags[0].rule_id, "GF080");
    assert_eq!(diags[0].span, span);
    assert!(diags[0].message.contains("[MIG-DESTRUCTIVE-DDL]"));
    assert!(diags[0].message.contains("DROP TABLE legacy_orders"));

    let mut allowed_cfg = Config::strict_defaults();
    allowed_cfg.sql.allow_destructive_migrations = true;
    assert_eq!(evaluate_document(&doc, &allowed_cfg), Vec::new());

    let plain_doc = UastDocument::new(RepoPath::new("queries/get.sql")?, Language::Sql, 3, 2)?;
    assert_eq!(evaluate_document(&plain_doc, &strict_cfg), Vec::new());
    Ok(())
}

#[test]
fn workspace_migrations_detect_duplicate_prefixes_per_group() -> Res {
    let cfg = Config::strict_defaults();
    let ctx = ProjectContext::default();
    let docs = vec![
        mig_doc("migrations/0001_init.sql", Some("0001"), "0001_init", false)?,
        mig_doc(
            "migrations/0004_users.sql",
            Some("0004"),
            "0004_users",
            false,
        )?,
        mig_doc(
            "migrations/0004_billing.sql",
            Some("0004"),
            "0004_billing",
            false,
        )?,
        mig_doc(
            "services/auth/migrations/0004_tokens.sql",
            Some("0004"),
            "0004_tokens",
            false,
        )?,
        mig_doc("migrations/init.sql", None, "init", false)?,
    ];
    let diags = evaluate_workspace_sorted(&docs, &ctx, &cfg);
    assert_eq!(diags.len(), 1, "{diags:?}");
    assert_eq!(diags[0].rule_id, "GF080");
    assert_eq!(diags[0].path.as_str(), "migrations/0004_users.sql");
    assert!(diags[0].message.contains("[MIG-VERSION-COLLISION-OR-GAP]"));
    assert!(diags[0].message.contains("0004_billing.sql"));
    Ok(())
}

#[test]
fn workspace_migrations_detect_orphan_downs_and_ignore_non_migrations() -> Res {
    let cfg = Config::strict_defaults();
    let ctx = ProjectContext::default();
    let mut schema_doc = UastDocument::new(RepoPath::new("sql/schema.sql")?, Language::Sql, 4, 3)?;
    schema_doc.sql_doc = Some(SqlDocument {
        dialect: SqlDialectKind::Postgres,
        role: SqlRole::QueryOrSchema,
        migration_group_dir: String::from("sql"),
        destructive_ddl: Vec::new(),
        rls_enabled_tables: std::collections::BTreeSet::new(),
        rls_forced_tables: std::collections::BTreeSet::new(),
        rls_policies_by_table: std::collections::BTreeMap::new(),
        plsql_package_specs: std::collections::BTreeMap::new(),
        plsql_package_bodies: std::collections::BTreeMap::new(),
    });
    let plain_doc = UastDocument::new(RepoPath::new("README.md")?, Language::Markdown, 2, 2)?;
    let docs = vec![
        mig_doc(
            "migrations/0001_init.up.sql",
            Some("0001"),
            "0001_init",
            false,
        )?,
        mig_doc(
            "migrations/0001_init.down.sql",
            Some("0001"),
            "0001_init",
            true,
        )?,
        mig_doc(
            "migrations/0009_ghost.down.sql",
            Some("0009"),
            "0009_ghost",
            true,
        )?,
        schema_doc,
        plain_doc,
    ];
    let diags = evaluate_workspace_sorted(&docs, &ctx, &cfg);
    assert_eq!(diags.len(), 1, "{diags:?}");
    assert_eq!(diags[0].rule_id, "GF080");
    assert_eq!(diags[0].path.as_str(), "migrations/0009_ghost.down.sql");
    assert!(diags[0].message.contains("orphan rollback migration"));
    assert!(diags[0].message.contains("0009_ghost"));
    Ok(())
}
