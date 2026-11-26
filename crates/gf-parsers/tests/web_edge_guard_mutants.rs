//! Targeted mutation-killer tests for `tool_output_web.rs`, `langs/astro/{mod.rs, splitter.rs}`,
//! and `langs/ts/drizzle_orm.rs`.

use std::path::Path;

use gf_core::{DomainLayoutConfig, ExternalToolKind, RepoPath, SqlDialectKind, UastDocument};
use gf_parsers::{
    parse_astro_check_output, parse_html_validate_json, parse_source, parse_source_with_context,
};

type TestResult = Result<(), Box<dyn std::error::Error>>;

fn parse_rel(rel: &str, src: &str) -> Result<UastDocument, Box<dyn std::error::Error>> {
    Ok(parse_source(&RepoPath::new(rel)?, src)?)
}

fn codes(doc: &UastDocument) -> Vec<&'static str> {
    doc.lang_rule_findings
        .iter()
        .map(gf_core::LangRuleFinding::code)
        .collect()
}

#[test]
fn tier3_astro_check_output_parser_kills_mutants() -> TestResult {
    let root = Path::new("/repo");
    let astro_raw = concat!(
        "\x1b[31msrc/pages/index.astro\x1b[0m:14:7 - error ts(2322): Type 'number' is not assignable to type 'string'.\r\n",
        "src/pages/card.astro(9,4): error ts2551: Property 'titl' does not exist.\n",
        "src/pages/bare.astro:3:2 - error Synthetic failure without colon\n",
        "src/pages/index.astro:2:1 - warning ts(6133): 'unused' is declared.\n",
        "Result (3 files): 3 errors\n",
    );
    let astro_findings = parse_astro_check_output(astro_raw, root)?;
    assert_eq!(astro_findings.len(), 3);
    assert_eq!(astro_findings[0].tool, ExternalToolKind::AstroCheck);
    assert_eq!(astro_findings[0].path.as_str(), "src/pages/index.astro");
    assert_eq!((astro_findings[0].line, astro_findings[0].column), (14, 7));
    assert_eq!(astro_findings[0].native_code, "ts(2322)");
    assert_eq!(astro_findings[1].path.as_str(), "src/pages/card.astro");
    assert_eq!((astro_findings[1].line, astro_findings[1].column), (9, 4));
    assert_eq!(astro_findings[1].native_code, "ts2551");
    assert_eq!(astro_findings[2].path.as_str(), "src/pages/bare.astro");
    assert_eq!((astro_findings[2].line, astro_findings[2].column), (3, 2));
    assert_eq!(astro_findings[2].native_code, "ts");
    Ok(())
}

#[test]
fn tier3_html_validate_json_parser_kills_mutants() -> TestResult {
    let root = Path::new("/repo");
    assert_eq!(parse_html_validate_json("   ", root)?, Vec::new());
    let bare_json = r#"[{"filePath":"/repo/src/index.html","messages":[{"ruleId":"element-permitted-content","severity":2,"message":"div in span","line":8,"column":3},{"ruleId":"no-warn","severity":1,"message":"warning ignored","line":1,"column":1}]}]"#;
    let bare = parse_html_validate_json(bare_json, root)?;
    assert_eq!(bare.len(), 1);
    assert_eq!(bare[0].path.as_str(), "src/index.html");
    assert_eq!((bare[0].line, bare[0].column), (8, 3));
    assert_eq!(bare[0].native_code, "element-permitted-content");

    let wrapped_json = r#"{"results":[{"messages":[{"severity":"error","message":"str error"},{"severity":"warning","message":"str warning"},{"message":"default error"}]}]}"#;
    let wrapped = parse_html_validate_json(wrapped_json, root)?;
    assert_eq!(wrapped.len(), 2);
    assert_eq!(wrapped[0].path.as_str(), "index.html");
    assert_eq!(wrapped[0].native_code, "html-validate");
    assert_eq!(wrapped[0].message, "str error");
    assert_eq!(wrapped[1].message, "default error");
    Ok(())
}

#[rustfmt::skip]
#[test]
fn astro_frontmatter_splitter_and_island_mutants_are_killed() -> TestResult {
    let fm_astro = concat!(
        "---\n/* / * still open\n---\n*/\n",
        "const ratio = 10/2; /*\n---\n*/\n",
        "const tpl = `line1\\`still\n---\nline2`;\n",
        "const okObj = { window: 1, val: cfg.window, r: ratio };\n",
        "const badHook = useState(0);\n",
        "const badMemberHook = React.useEffect(() => {});\n",
        "const normalCall = String(okObj.window);\n---\n<main>\n",
        "  <HeroWidget title=\"static\" only={ratio} idValue={ratio} client:visible />\n",
        "  <InteractiveChart client:load />\n  <Card onClick={fn} client:load />\n",
        "  <HeroWidget render={() => null} client:idle />\n",
        "  <HeroWidget cb={function() {}} client:idle />\n",
        "  <HeroWidget created={new Date()} client:idle />\n</main>\n<script src=\"/app.js\" />\n",
        "<script>\n  const nested = \"<script is:inline>ignored\";\n</script><script is:inline>\n  const inlineAfter = 1;\n</script>\n",
        "<scripting>ignored</scripting>\n",
    );
    let doc = parse_rel("src/pages/test.astro", fm_astro)?;
    assert!(doc.referenced_symbols.contains("HeroWidget") && doc.referenced_symbols.contains("InteractiveChart"));
    let line_codes: Vec<(&str, u32)> = doc.lang_rule_findings.iter().map(|f| (f.code(), f.span.start_line())).collect();
    for clean_ln in [11, 17, 18, 25, 26] {
        assert!(!line_codes.iter().any(|&(_, ln)| ln == clean_ln), "{line_codes:?}");
    }
    for exp in [
        ("ASTRO-FRONTMATTER-CLIENT-API", 12), ("ASTRO-FRONTMATTER-CLIENT-API", 13),
        ("ASTRO-CLIENT-LOAD-BUDGET", 19), ("ASTRO-NON-SERIALIZABLE-ISLAND-PROP", 19),
        ("ASTRO-NON-SERIALIZABLE-ISLAND-PROP", 20), ("ASTRO-NON-SERIALIZABLE-ISLAND-PROP", 21),
        ("ASTRO-NON-SERIALIZABLE-ISLAND-PROP", 22), ("ASTRO-CLIENT-LOAD-BUDGET", 27),
    ] {
        assert!(line_codes.contains(&exp), "missing {exp:?} in {line_codes:?}");
    }
    let bad_fm = "---\nconst a = { key: window };\nconst b = window.innerWidth;\n---\n<p>hi</p>\n";
    assert_eq!(codes(&parse_rel("src/pages/bad_fm.astro", bad_fm)?), vec!["ASTRO-FRONTMATTER-CLIENT-API", "ASTRO-FRONTMATTER-CLIENT-API"]);
    assert_eq!(parse_rel("src/pages/empty.astro", "   ")?.lang_rule_findings, Vec::new());
    assert_eq!(parse_rel("src/pages/unclosed.astro", "---\nconst x = 1;\n")?.lang_rule_findings, Vec::new());
    Ok(())
}

#[test]
fn drizzle_orm_monetary_columns_and_where_chains_kill_mutants() -> TestResult {
    let schema_src = concat!(
        "import { drizzle } from \"drizzle-orm/node-postgres\";\n",
        "import { pgTable, integer, real, doublePrecision, float } from \"drizzle-orm/pg-core\";\n",
        "export const items = pgTable(\"items\", {\n",
        "  id: integer(\"id\").primaryKey(),\n  price: real(\"price\"),\n",
        "  unit_price: doublePrecision(\"unit_price\"),\n  totalAmount: float(\"total_amount\"),\n",
        "  cleanPrice: integer(\"clean_price\"),\n  ratio: real(\"ratio\"),\n});\n",
    );
    let schema_doc = parse_rel("src/db/schema.ts", schema_src)?;
    let float_count = schema_doc
        .lang_rule_findings
        .iter()
        .filter(|f| f.code() == "DRIZZLE-FLOAT-MONEY")
        .count();
    assert_eq!(float_count, 3);
    assert!(!codes(&schema_doc).contains(&"DRIZZLE-DIALECT-MISMATCH"));

    let mutations_src = concat!(
        "export async function run(db: Db, map: Map<string, string>, cache: Set<string>) {\n",
        "  await db.update(users).set({ role: \"user\" }).where(eq(users.id, 1));\n",
        "  await db.update(users).set({ role: \"user\" }).$dynamic().where(eq(users.id, 1));\n",
        "  await db.delete(users).where(eq(users.id, 1));\n",
        "  map.set(\"k\", \"v\");\n  cache.delete(\"k\");\n",
        "  await db.update(users).set({ role: \"admin\" }).returning();\n",
        "  await wrap.where(db.delete(users));\n}\n",
    );
    let mut_doc = parse_rel("src/db/mutations.ts", mutations_src)?;
    let mut_lines: Vec<u32> = mut_doc
        .lang_rule_findings
        .iter()
        .filter(|f| f.code() == "DRIZZLE-UNBOUNDED-MUTATION")
        .map(|f| f.span.start_line())
        .collect();
    assert_eq!(mut_lines, vec![7, 8]);
    Ok(())
}

#[test]
fn drizzle_orm_dialect_inference_variants_kill_mutants() -> TestResult {
    for src in [
        "import { drizzle } from \"drizzle-orm/node-postgres\";\nimport { sqliteTable } from \"drizzle-orm/sqlite-core\";\n",
        "import { drizzle } from \"drizzle-orm/libsql\";\nimport { pgTable } from \"drizzle-orm/pg-core\";\n",
        "import { drizzle } from \"drizzle-orm/mysql2\";\nimport { pgTable } from \"drizzle-orm/pg-core\";\n",
        "import { pgTable } from \"drizzle-orm/pg-core\";\nexport interface Env { DB: D1Database; }\n",
        "import { pgTable } from \"drizzle-orm/pg-core\";\nconst cfg = \"d1_databases\";\n",
    ] {
        assert_eq!(
            codes(&parse_rel("src/db/bad.ts", src)?),
            vec!["DRIZZLE-DIALECT-MISMATCH"]
        );
    }
    let mysql_ok = "import { drizzle } from \"drizzle-orm/mysql2\";\nimport { mysqlTable } from \"drizzle-orm/mysql-core\";\n";
    assert_eq!(
        codes(&parse_rel("src/db/mysql_ok.ts", mysql_ok)?),
        Vec::<&str>::new()
    );
    let ws_pg_doc = parse_source_with_context(
        &RepoPath::new("src/db/ws_pg.ts")?,
        "import { mysqlTable } from \"drizzle-orm/mysql-core\";\n",
        &DomainLayoutConfig::strict_defaults(),
        Some(SqlDialectKind::Postgres),
    )?;
    assert_eq!(codes(&ws_pg_doc), vec!["DRIZZLE-DIALECT-MISMATCH"]);
    Ok(())
}
