//! Tier 2 (`GF081`), Tier 3 (`GF082`), and boundary mutation guards for `langs/sql/*`
//! and `tool_output_sql.rs`.

use std::path::Path;

use gf_core::{Config, ExternalToolKind, RepoPath, SqlDialectKind, SqlRole};
use gf_parsers::{parse_source, parse_sqlfluff_json, parse_sqruff_json, parse_squawk_json};
use gf_rules::evaluate_document;

type Res = Result<(), Box<dyn std::error::Error>>;

#[test]
fn sql_tier2_configs_and_malformed_inputs_are_audited_precisely() -> Res {
    let cfg = Config::strict_defaults();
    let squawk_cfg = "\n# comment\nexcluded_rules = [\"require-concurrent-index-creation\", \"ok-rule\"]\nexcluded_paths = [\"./migrations/**\", \"docs/demo.sql\"]\n";
    let sq_diags = evaluate_document(
        &parse_source(&RepoPath::new(".squawk.toml")?, squawk_cfg)?,
        &cfg,
    );
    assert_eq!(sq_diags.len(), 2);
    assert_eq!(sq_diags[0].span.start_line(), 3);
    assert_eq!(sq_diags[1].span.start_line(), 4);

    let bad_squawk_toml = "excluded_rules = [\n";
    assert_eq!(
        evaluate_document(
            &parse_source(&RepoPath::new("squawk.toml")?, bad_squawk_toml)?,
            &cfg
        )[0]
        .rule_id,
        "GF000"
    );

    let sqruff_ini = "# header\n[sqlfluff]\ndialect = postgres\nexclude_rules = all # inline\n";
    let sqruff_diags =
        evaluate_document(&parse_source(&RepoPath::new(".sqruff")?, sqruff_ini)?, &cfg);
    assert_eq!(sqruff_diags.len(), 1);
    assert_eq!(sqruff_diags[0].span.start_line(), 4);

    let fluff_ini = "[sqlfluff]\nexclude_rules = L001, CV05, AM04\n";
    assert_eq!(
        evaluate_document(
            &parse_source(&RepoPath::new(".sqlfluff")?, fluff_ini)?,
            &cfg
        )
        .len(),
        2
    );
    Ok(())
}

#[test]
fn sql_tier2_drizzle_config_credentials_are_audited() -> Res {
    let cfg = Config::strict_defaults();
    let drizzle_bad = concat!(
        "export default {\n",
        "  dbCredentials: {\n",
        "    url: \"postgres://admin:s3cr3t@db.internal:5432/app\",\n",
        "    password: \"inline_pw\",\n",
        "    authToken: process.env.TURSO_TOKEN,\n",
        "  }\n",
        "};\n",
    );
    let drizzle_diags = evaluate_document(
        &parse_source(&RepoPath::new("drizzle.config.ts")?, drizzle_bad)?,
        &cfg,
    );
    assert_eq!(
        drizzle_diags
            .iter()
            .filter(|d| d.rule_id == "GF081")
            .count(),
        2
    );
    let drizzle_ok = "export default { url: `postgres://${process.env.DB_PASS}@localhost/db` };\n";
    assert_eq!(
        evaluate_document(
            &parse_source(&RepoPath::new("drizzle.config.mjs")?, drizzle_ok)?,
            &cfg
        ),
        Vec::new()
    );
    Ok(())
}

#[test]
fn sql_tier3_parsers_handle_empty_fallbacks_and_both_sqruff_sqlfluff_shapes() -> Res {
    let root = Path::new("/repo");
    assert_eq!(parse_squawk_json("   ", root)?, Vec::new());
    assert_eq!(parse_sqruff_json("", root)?, Vec::new());
    assert_eq!(parse_sqlfluff_json("\n", root)?, Vec::new());
    assert!(parse_squawk_json("{not-json", root).is_err());

    let squawk_items = parse_squawk_json(
        "[{\"file\":\"C:\\\\repo\\\\migrations\\\\001.sql\",\"line\":0,\"column\":36,\"rule_name\":\"\",\"message\":\"m\",\"help\":\"\"}]",
        Path::new("C:\\repo"),
    )?;
    assert_eq!(squawk_items.len(), 1);
    assert_eq!(squawk_items[0].tool, ExternalToolKind::Squawk);
    assert_eq!(squawk_items[0].path.as_str(), "migrations/001.sql");
    assert_eq!(squawk_items[0].line, 1);
    assert_eq!(squawk_items[0].column, 37);
    assert_eq!(squawk_items[0].native_code, "squawk");
    assert_ne!(squawk_items[0].remediation, "");

    let sqruff_map = parse_sqruff_json(
        "{\"sql/q.sql\":[{\"code\":null,\"message\":\"Use IS NULL\",\"range\":{\"start\":{\"line\":4,\"character\":12}}}]}",
        root,
    )?;
    assert_eq!(sqruff_map[0].line, 4);
    assert_eq!(sqruff_map[0].column, 12);
    assert_eq!(sqruff_map[0].native_code, "sqruff");

    let fluff_arr = parse_sqlfluff_json(
        "[{\"filepath\":\"/repo/sql/q.sql\",\"violations\":[{\"line_no\":7,\"line_pos\":3,\"code\":\"\",\"description\":\"Use IS NULL\"}]}]",
        root,
    )?;
    assert_eq!(fluff_arr[0].tool, ExternalToolKind::Sqlfluff);
    assert_eq!(fluff_arr[0].path.as_str(), "sql/q.sql");
    assert_eq!(fluff_arr[0].line, 7);
    assert_eq!(fluff_arr[0].column, 3);
    assert_eq!(fluff_arr[0].native_code, "sqlfluff");
    Ok(())
}

#[test]
fn dialect_pragmas_and_prisma_migration_roles_are_exact() -> Res {
    let prisma_up = parse_source(
        &RepoPath::new("prisma/migrations/20260401_init/migration.sql")?,
        "/* header */\n-- garde-dialect: duckdb\nCREATE TABLE items (id INTEGER PRIMARY KEY);\n",
    )?;
    let sql_doc = prisma_up.sql_doc.as_ref().ok_or("missing sql_doc")?;
    assert_eq!(sql_doc.dialect, SqlDialectKind::DuckDb);
    assert_eq!(sql_doc.migration_group_dir, "prisma/migrations");
    assert_eq!(
        sql_doc.role,
        SqlRole::Migration {
            version_prefix: Some(String::from("20260401")),
            migration_stem: String::from("20260401_init"),
            is_down: false,
        }
    );

    let prisma_down = parse_source(
        &RepoPath::new("prisma/migrations/20260401_init/down.sql")?,
        "DROP TABLE items;\n",
    )?;
    assert!(matches!(
        prisma_down.sql_doc.as_ref().map(|s| &s.role),
        Some(SqlRole::Migration { is_down: true, .. })
    ));
    Ok(())
}

#[test]
fn reindex_and_tautology_union_star_edges_are_exact() -> Res {
    let cfg = Config::strict_defaults();
    let edges = concat!(
        "REINDEX;\n",
        "REINDEX TABLE users;\n",
        "DELETE FROM logs WHERE id = 5 OR 'a' = 'a';\n",
        "DELETE FROM logs WHERE true AND 2 = 2;\n",
        "DELETE FROM logs WHERE 1 = 2;\n",
        "SELECT id FROM a UNION ALL SELECT * FROM b;\n",
    );
    let diags = evaluate_document(
        &parse_source(&RepoPath::new("sql/edges.sql")?, edges)?,
        &cfg,
    );
    assert!(
        diags
            .iter()
            .any(|d| d.rule_id == "GF000" && d.span.start_line() == 1)
    );
    assert_eq!(
        diags
            .iter()
            .filter(|d| d.message.contains("SQL-UNBOUNDED-MUTATION"))
            .count(),
        2
    );
    assert_eq!(
        diags
            .iter()
            .filter(|d| d.message.contains("SQL-SELECT-STAR"))
            .count(),
        1
    );
    Ok(())
}
