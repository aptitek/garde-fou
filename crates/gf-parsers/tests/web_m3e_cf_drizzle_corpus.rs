//! Phase 4.1: Symmetric `valid` vs `invalid` specification corpus for Astro (`.astro`),
//! React Router v7, Panda CSS & Material Design 3 Expressive (`M3E`), W3C HTML5 & WCAG 2.2 AA,
//! Cloudflare Workers / D1 / R2 / Wrangler, and Drizzle ORM (`GF061`, `GF080`, `GF081`).

use gf_core::{
    Config, Diagnostic, DomainLayoutConfig, RepoPath, SqlDialectKind, ToolConfigViolation,
    WebEdgeConfigIssue,
};
use gf_parsers::{parse_source, parse_source_with_context};
use gf_rules::evaluate_document;

type TestResult = Result<(), Box<dyn std::error::Error>>;

fn eval_file(
    rel: &str,
    src: &str,
    hint: Option<SqlDialectKind>,
) -> Result<Vec<Diagnostic>, String> {
    let path = RepoPath::new(rel).map_err(|e| e.to_string())?;
    let cfg = Config::strict_defaults();
    let doc = parse_source_with_context(&path, src, &DomainLayoutConfig::strict_defaults(), hint)
        .map_err(|e| e.to_string())?;
    Ok(evaluate_document(&doc, &cfg))
}

fn has_code_at_line(diags: &[Diagnostic], code_tag: &str, line: u32) -> bool {
    diags
        .iter()
        .any(|d| d.message.contains(code_tag) && d.span.start_line() == line)
}

#[test]
fn astro_three_zone_valid_vs_invalid_corpus() -> TestResult {
    let valid_astro = "---\nimport { HeroCard } from \"../components/HeroCard\";\nconst heading = \"Edge Portal\";\n---\n<main>\n  <h1>{heading}</h1>\n  <HeroCard title={heading} client:visible />\n</main>\n<style>\n  h1 {\n    color: var(--color-fg);\n  }\n</style>\n";
    let clean = eval_file("src/pages/index.astro", valid_astro, None)?;
    assert!(
        clean.is_empty(),
        "expected 0 violations on valid .astro, got: {clean:#?}"
    );

    let invalid_astro = "---\nimport { Card } from \"../components/Card\";\nconst href = window.location.href;\n---\n<main>\n  <div set:html={href} />\n  <Card onClick={() => {}} client:load />\n</main>\n<script is:inline>\n  const x = 1;\n</script>\n<style>\n  .box {\n    color: #ff0055;\n  }\n</style>\n";
    let diags = eval_file("src/pages/bad.astro", invalid_astro, None)?;
    for (code, ln) in [
        ("[ASTRO-FRONTMATTER-CLIENT-API]", 3),
        ("[ASTRO-NO-SET-HTML]", 6),
        ("[ASTRO-CLIENT-LOAD-BUDGET]", 7),
        ("[ASTRO-NON-SERIALIZABLE-ISLAND-PROP]", 7),
        ("[ASTRO-CLIENT-LOAD-BUDGET]", 9),
    ] {
        assert!(has_code_at_line(&diags, code, ln), "{diags:#?}");
    }
    assert!(
        diags
            .iter()
            .any(|d| d.rule_id == "GF061" && d.span.start_line() == 14),
        "{diags:#?}"
    );
    Ok(())
}

#[test]
fn react_router_v7_valid_vs_invalid_corpus() -> TestResult {
    let valid_route = "import { Link } from \"react-router\";\nexport async function loader(): Promise<{ title: string }> { return { title: \"Dashboard\" }; }\nexport async function action(): Promise<{ ok: boolean }> { return { ok: true }; }\nexport function ErrorBoundary() { return <div role=\"alert\">Error</div>; }\nexport default function RootRoute() {\n  return <nav aria-label=\"Primary\"><Link to=\"/dashboard\">Dashboard</Link><a href=\"/report.pdf\" download>PDF</a></nav>;\n}\n";
    let clean = eval_file("app/root.tsx", valid_route, None)?;
    assert!(
        clean.is_empty(),
        "expected 0 violations on valid RRv7 root.tsx, got: {clean:#?}"
    );

    let invalid_route = "import { db } from \"../db.server\";\n\nexport async function loader() {\n  await db.insert(\"users\");\n  await fetch(\"/api/sync\", { method: \"POST\" });\n  return { ok: true };\n}\n\nexport async function clientLoader() {\n  return db.query();\n}\n\nexport default function BadRoot() {\n  return <a href=\"/settings\">Settings</a>;\n}\n";
    let diags = eval_file("app/root.tsx", invalid_route, None)?;
    for (code, ln) in [
        ("[RR-ROOT-MISSING-ERROR-BOUNDARY]", 1),
        ("[RR-MUTATION-IN-LOADER]", 4),
        ("[RR-MUTATION-IN-LOADER]", 5),
        ("[RR-SERVER-LEAK-TO-CLIENT]", 10),
        ("[RR-RAW-INTERNAL-ANCHOR]", 14),
    ] {
        assert!(has_code_at_line(&diags, code, ln), "{diags:#?}");
    }
    Ok(())
}

#[test]
fn panda_css_and_m3e_valid_vs_invalid_corpus() -> TestResult {
    let valid_panda = "import { css } from \"../../styled-system/css\";\nexport const surfaceCard = css({\n  bg: \"primaryContainer\",\n  color: \"onPrimaryContainer\",\n  boxShadow: \"elevation.level2\",\n  borderRadius: \"shape.corner.large\",\n  textStyle: \"title.medium\",\n  transition: \"transform var(--motion-spring-fast-spatial), background-color var(--motion-spring-fast-effects)\",\n  minW: \"12\",\n  minH: \"12\",\n});\n";
    let clean = eval_file("src/components/SurfaceCard.ts", valid_panda, None)?;
    assert!(
        clean.is_empty(),
        "expected 0 violations on valid Panda+M3E, got: {clean:#?}"
    );

    let invalid_panda = "import { css } from \"../../styled-system/css\";\n\nexport const brokenCard = css({\n  bg: \"primary\",\n  color: \"onSecondary\",\n  padding: \"[13px]\",\n  borderColor: \"blue.500\",\n  boxShadow: \"0 4px 12px rgba(0,0,0,0.25)\",\n  borderRadius: \"9px\",\n  textStyle: \"heroGiant\",\n  transition: \"background-color var(--motion-spring-fast-spatial)\",\n  minW: \"32px\",\n  _hover: { opacity: \"0.85\" },\n});\n";
    let diags = eval_file("src/components/BrokenCard.ts", invalid_panda, None)?;
    for (code, ln) in [
        ("[M3E-TONAL-PAIRING]", 5),
        ("[PANDA-ARBITRARY-OR-PRIMITIVE]", 6),
        ("[PANDA-ARBITRARY-OR-PRIMITIVE]", 7),
        ("[M3E-RAW-ELEVATION-SHAPE-TYPO]", 8),
        ("[M3E-RAW-ELEVATION-SHAPE-TYPO]", 9),
        ("[M3E-RAW-ELEVATION-SHAPE-TYPO]", 10),
        ("[M3E-SPRING-PHYSICS-MISMATCH]", 11),
        ("[M3E-TOUCH-TARGET-AND-STATE-LAYER]", 12),
        ("[M3E-TOUCH-TARGET-AND-STATE-LAYER]", 13),
    ] {
        assert!(has_code_at_line(&diags, code, ln), "{diags:#?}");
    }

    let committed = eval_file(
        "styled-system/css/index.mjs",
        "export const css = () => '';\n",
        None,
    )?;
    assert!(
        has_code_at_line(&committed, "[PANDA-GENERATED-COMMITTED]", 1),
        "{committed:#?}"
    );

    let bad_cfg = parse_source(
        &RepoPath::new("panda.config.ts")?,
        "export default { strictTokens: false };\n",
    )?;
    assert!(bad_cfg.tool_config_findings.iter().any(|f| {
        f.violation
            == ToolConfigViolation::WebEdge(WebEdgeConfigIssue::PandaStrictTokensMissingOrFalse)
    }));
    Ok(())
}

#[test]
fn w3c_html5_and_wcag22_valid_vs_invalid_corpus() -> TestResult {
    let valid_html = "<main id=\"main-content\">\n  <nav aria-label=\"Primary\"><ul><li><a href=\"#main-content\">Skip</a></li></ul></nav>\n  <form><label for=\"email-input\">Email</label><input id=\"email-input\" type=\"email\" autocomplete=\"email\" /><button type=\"submit\">Subscribe</button></form>\n</main>\n";
    let clean = eval_file("public/index.html", valid_html, None)?;
    assert!(
        clean.is_empty(),
        "expected 0 violations on valid W3C HTML5, got: {clean:#?}"
    );

    let invalid_html = "<main id=\"dup\">\n  <a href=\"/go\"><button type=\"button\">Nested</button></a>\n  <img>illegal child</img>\n  <ul><div>Not an li</div></ul>\n  <nav><a href=\"/a\">One</a></nav>\n  <nav><a href=\"/b\">Two</a></nav>\n  <div id=\"dup\" role=\"superbutton\" aria-checked=\"maybe\">Bad ARIA</div>\n  <div aria-hidden=\"true\"><button type=\"button\">Ghost</button></div>\n  <button type=\"button\" tabindex=\"3\"></button>\n  <video autoplay><source src=\"clip.mp4\" /></video>\n  <input type=\"email\" />\n</main>\n";
    let diags = eval_file("public/broken.html", invalid_html, None)?;
    for (code, ln) in [
        ("[W3C-INTERACTIVE-NESTING]", 2),
        ("[W3C-VOID-ELEMENT-CHILDREN]", 3),
        ("[W3C-CONTENT-MODEL]", 4),
        ("[W3C-LANDMARK-AND-ID-UNIQUENESS]", 5),
        ("[W3C-LANDMARK-AND-ID-UNIQUENESS]", 7),
        ("[A11Y-ARIA-INVALID]", 7),
        ("[A11Y-HIDDEN-FOCUSABLE]", 8),
        ("[A11Y-TABINDEX-AND-FOCUS-RING]", 9),
        ("[A11Y-MEDIA-FORM-MOTION]", 9),
        ("[A11Y-MEDIA-FORM-MOTION]", 10),
        ("[A11Y-MEDIA-FORM-MOTION]", 11),
    ] {
        assert!(
            has_code_at_line(&diags, code, ln),
            "missing {code} at line {ln}: {diags:#?}"
        );
    }
    Ok(())
}

#[test]
fn cloudflare_workers_and_drizzle_valid_vs_invalid_corpus() -> TestResult {
    let valid_worker = "import { integer, index, sqliteTable } from \"drizzle-orm/sqlite-core\";\nexport interface Env { readonly DB: unknown; readonly BUCKET: { get(k: string): Promise<unknown> }; }\nexport const orders = sqliteTable(\"orders\", {\n  id: integer(\"id\").primaryKey(),\n  customerId: integer(\"customer_id\").references(() => orders.id),\n  totalAmountCents: integer(\"total_amount_cents\").notNull(),\n}, (t) => [index(\"customer_idx\").on(t.customerId)]);\nexport async function handleRequest(env: Env): Promise<unknown> {\n  return { now: Date.now(), db: env.DB, obj: await env.BUCKET.get(\"reports/daily.json\") };\n}\n";
    let clean = eval_file(
        "src/worker.ts",
        valid_worker,
        Some(SqlDialectKind::CloudflareD1),
    )?;
    assert!(
        clean.is_empty(),
        "expected 0 violations on valid Worker+Drizzle, got: {clean:#?}"
    );

    let invalid_worker = "import fs from \"node:fs\";\nimport { pgTable, real, integer } from \"drizzle-orm/pg-core\";\nexport interface Env {\n  readonly DB: { prepare(q: string): { bind(x: number): { run(): Promise<unknown> } } };\n  readonly BUCKET: { put(k: string, v: string): Promise<unknown> };\n}\nlet leakedCounter = 0;\nconst bootTimestamp = Date.now();\nexport const invoices = pgTable(\"invoices\", {\n  id: integer(\"id\").primaryKey(),\n  userId: integer(\"user_id\").references(() => invoices.id),\n  totalAmount: real(\"total_amount\"),\n});\nexport async function badHandler(\n  req: { cf: Record<string, unknown> },\n  env: Env,\n  db: { delete(t: unknown): Promise<unknown> },\n  sql: { raw(q: string): unknown },\n): Promise<unknown> {\n  leakedCounter += 1;\n  req.cf.colo = \"CDG\";\n  for (const id of [1, 2, 3]) {\n    await env.DB.prepare(\"INSERT INTO t VALUES (?)\").bind(id).run();\n  }\n  await env.BUCKET.put(\"../secrets/key.pem\", \"x\");\n  await db.delete(invoices);\n  const rawExpr = sql.raw(`SELECT * FROM invoices WHERE id = ${leakedCounter}`);\n  return { fs, bootTimestamp, rawExpr };\n}\n";
    let diags = eval_file(
        "src/bad_worker.ts",
        invalid_worker,
        Some(SqlDialectKind::CloudflareD1),
    )?;
    for (code, ln) in [
        ("[CF-FORBIDDEN-NODE-API]", 1),
        ("[DRIZZLE-DIALECT-MISMATCH]", 2),
        ("[CF-GLOBAL-SCOPE-IMPURE]", 8),
        ("[DRIZZLE-FK-MISSING-INDEX]", 11),
        ("[DRIZZLE-FLOAT-MONEY]", 12),
        ("[CF-GLOBAL-SCOPE-IMPURE]", 20),
        ("[CF-R2-STREAM-AND-KEY-SAFETY]", 21),
        ("[CF-D1-N-PLUS-ONE-OR-LIMIT]", 23),
        ("[CF-R2-STREAM-AND-KEY-SAFETY]", 25),
        ("[DRIZZLE-UNBOUNDED-MUTATION]", 26),
        ("[DRIZZLE-RAW-UNSAFE]", 27),
    ] {
        assert!(
            has_code_at_line(&diags, code, ln),
            "missing {code} at line {ln}: {diags:#?}"
        );
    }
    Ok(())
}
