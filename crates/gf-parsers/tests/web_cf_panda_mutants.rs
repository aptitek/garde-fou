//! Targeted mutation-killer tests for `langs/ts/{react_router.rs, cloudflare_workers.rs, panda_m3e.rs, wrangler_config.rs}`.

use gf_core::{RepoPath, UastDocument};
use gf_parsers::parse_source;

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

#[rustfmt::skip]
#[test]
fn react_router_specifiers_and_route_paths_kill_mutants() -> TestResult {
    for path in ["src/entry.client.tsx", "src/components/Nav.tsx", "components/Card.tsx"] {
        for spec in ["./auth.server", "./db.server.ts", "../server/token", "drizzle-orm", "drizzle-orm/d1"] {
            let src = format!("import {{ x }} from \"{spec}\";\nexport const C = () => null;\n");
            let d = parse_rel(path, &src)?;
            assert!(codes(&d).contains(&"RR-SERVER-LEAK-TO-CLIENT"), "{path} {spec}");
        }
    }
    for route_path in ["root.tsx", "app/root.jsx", "routes/home.tsx", "app/routes/home.tsx", "packages/web/routes/home.tsx"] {
        let d = parse_rel(route_path, "export function ErrorBoundary() { return <a href=\"/home\" />; }\n")?;
        assert!(codes(&d).contains(&"RR-RAW-INTERNAL-ANCHOR"), "{route_path}");
    }
    let non_route = parse_rel("src/utils/render.tsx", "export function Helper() { return <div><a href=\"/home\">Home</a><span href=\"/home\">x</span></div>; }\n")?;
    assert!(!codes(&non_route).contains(&"RR-RAW-INTERNAL-ANCHOR"));
    for imp in ["react-router", "@react-router/cloudflare"] {
        let d = parse_rel("src/utils/nav.tsx", &format!("import {{ x }} from \"{imp}\";\nexport function Nav() {{ return <a href=\"/x\">x</a>; }}\n"))?;
        assert!(codes(&d).contains(&"RR-RAW-INTERNAL-ANCHOR"));
    }
    Ok(())
}

#[rustfmt::skip]
#[test]
fn react_router_arrow_loaders_and_client_leaks_kill_mutants() -> TestResult {
    let route_src = concat!(
        "import { secretFn } from \"../db.server\";\n",
        "export const loader = async () => {\n  await db.insert(logs);\n  return { ok: true };\n};\n",
        "export const clientLoader = async () => {\n",
        "  const mod = await import(\"../auth.server\");\n",
        "  const safeCall = loaderHelper(\"../auth.server\");\n",
        "  const safeProp = mod.secretFn;\n",
        "  return secretFn(safeCall, safeProp);\n};\n",
    );
    let route_doc = parse_rel("app/routes/item.tsx", route_src)?;
    let findings: Vec<(&str, u32)> = route_doc.lang_rule_findings.iter().map(|f| (f.code(), f.span.start_line())).collect();
    assert_eq!(findings, vec![("RR-MUTATION-IN-LOADER", 3), ("RR-SERVER-LEAK-TO-CLIENT", 7), ("RR-SERVER-LEAK-TO-CLIENT", 10)]);
    Ok(())
}

#[rustfmt::skip]
#[test]
fn cloudflare_workers_env_and_globals_kill_mutants() -> TestResult {
    let worker_src = concat!(
        "import fs from \"node:fs\";\nimport crypto from \"node:crypto\";\n",
        "export interface Env { DB: D1Database; BUCKET: R2Bucket; }\nexport interface Other { NOT_ENV: string; }\n",
        "let globalCounter = 0;\nglobalCounter = 1;\nconst BOOT_DATE = new Date();\nconst BOOT_MAP = new Map();\n",
        "export default {\n  async fetch(request: Request, req: Request, env: Env): Promise<Response> {\n",
        "    let localCounter = 0;\n    localCounter += 1;\n    globalCounter += 1;\n    const now = new Date();\n",
        "    void now; void BOOT_DATE; void BOOT_MAP; void fs; void crypto;\n",
        "    request.cf = {};\n    req.cf = {};\n    request.cf.colo = \"CDG\";\n    req.cf.country = \"FR\";\n",
        "    const okDb = env.DB;\n    const badEnv = env.NOT_ENV;\n    void okDb; void badEnv;\n",
        "    return new Response(\"ok\");\n  }\n};\n",
    );
    let doc = parse_rel("src/worker.ts", worker_src)?;
    let we = doc.web_edge.as_ref().ok_or("missing web_edge")?;
    assert_eq!(we.node_colon_imports[0].0, "node:crypto");
    assert!(we.documented_env_keys.contains("DB") && !we.documented_env_keys.contains("NOT_ENV"));
    let line_codes: Vec<(&str, u32)> = doc.lang_rule_findings.iter().filter(|f| f.code().starts_with("CF-")).map(|f| (f.code(), f.span.start_line())).collect();
    for clean_ln in [6, 8, 12, 14, 20] {
        assert!(!line_codes.iter().any(|&(_, ln)| ln == clean_ln), "{line_codes:?}");
    }
    for exp in [
        ("CF-FORBIDDEN-NODE-API", 1), ("CF-GLOBAL-SCOPE-IMPURE", 7), ("CF-GLOBAL-SCOPE-IMPURE", 13),
        ("CF-R2-STREAM-AND-KEY-SAFETY", 16), ("CF-R2-STREAM-AND-KEY-SAFETY", 17),
        ("CF-R2-STREAM-AND-KEY-SAFETY", 18), ("CF-R2-STREAM-AND-KEY-SAFETY", 19), ("CF-BINDING-COHERENCE", 21),
    ] {
        assert!(line_codes.contains(&exp), "missing {exp:?} in {line_codes:?}");
    }
    let no_env = parse_rel("src/no_env.ts", "export default { async fetch(_r: Request, env: unknown) { return env.ANY; } };\n")?;
    assert!(!codes(&no_env).contains(&"CF-BINDING-COHERENCE"));
    Ok(())
}

#[rustfmt::skip]
#[test]
fn cloudflare_workers_d1_limits_and_exec_kill_mutants() -> TestResult {
    let q100 = vec!["?"; 100].join(", ");
    let q101 = vec!["?"; 101].join(", ");
    let d1_src = format!(
        "export async function run(env: Env, other: Db, x: string) {{\n\
           await env.DB.prepare(\"SELECT {q100}\").all();\n\
           await env.DB.prepare(\"SELECT {q101}\").all();\n\
           await env.DB.exec(\"CREATE TABLE t (id INT)\");\n\
           await env.DB.exec(`DROP TABLE ${{x}}`);\n\
           await env.DB.exec(\"DROP TABLE \" + x);\n\
           await other.exec(`DROP TABLE ${{x}}`);\n\
           await env.DB.query(`SELECT ${{x}}`);\n\
           for (const id of [1, 2]) {{\n\
             await other.all(id);\n\
             await env.DB.prepare(\"SELECT ?\").bind(id).first();\n\
           }}\n\
         }}\n"
    );
    let doc = parse_rel("src/d1.ts", &d1_src)?;
    let d1_lines: Vec<u32> = doc.lang_rule_findings.iter().filter(|f| f.code() == "CF-D1-N-PLUS-ONE-OR-LIMIT").map(|f| f.span.start_line()).collect();
    assert_eq!(d1_lines, vec![3, 5, 6, 11]);
    Ok(())
}

#[rustfmt::skip]
#[test]
fn panda_m3e_tokens_elevation_shape_and_targets_kill_mutants() -> TestResult {
    let panda_src = concat!(
        "import { css } from \"../styled-system/css\";\n",
        "export const clean = css({\n",
        "  bg: \"surfaceContainer\",\n  color: \"onSurface\",\n",
        "  borderColor: \"semantic.500\",\n  outlineColor: \"blue.primary\",\n",
        "  border: \"[unclosed\",\n  stroke: \"var(--color-ink, #112233)\",\n",
        "  padding: \"token(spacing.md, 16px)\",\n  boxShadow: \"none\",\n",
        "  shadow: \"elevation.level2\",\n  borderRadius: \"var(--shape-corner-md)\",\n",
        "  rounded: \"var(--radius-lg)\",\n  minW: \"0px\",\n  minH: \"48px\",\n",
        "  minWidth: \"0rem\",\n  minHeight: \"3rem\",\n",
        "  _focus: { opacity: \"0.8\" },\n  _hover: { bg: \"surfaceContainerHigh\" },\n});\n",
        "export const ValidLevels = [css({ shadow: \"var(--elevation-level-1)\" }), css({ shadow: \"level3\" })];\n",
        "export const dirty = css({\n",
        "  bg: \"blue.500\",\n  color: \"solarized.base03\",\n  borderColor: \"solarized.base1\",\n",
        "  fill: \"#ff0000\",\n  padding: \"16px\",\n  margin: \"[13px]\",\n",
        "  top: \"token!\",\n  left: \"token!important\",\n",
        "  minW: \"47px\",\n  minHeight: \"2.5rem\",\n  _hover: { opacity: \"0.5\" },\n});\n",
    );
    let doc = parse_rel("src/styles.ts", panda_src)?;
    let line_codes: Vec<(&str, u32)> = doc.lang_rule_findings.iter().map(|f| (f.code(), f.span.start_line())).collect();
    for clean_ln in 2..=8 {
        assert!(!line_codes.iter().any(|&(c, ln)| ln == clean_ln && (c.starts_with("PANDA-") || c.starts_with("M3E-"))));
    }
    assert_eq!(line_codes.iter().filter(|&&(c, _)| c == "PANDA-ARBITRARY-OR-PRIMITIVE").count(), 14);
    assert_eq!(line_codes.iter().filter(|&&(c, _)| c == "M3E-TONAL-PAIRING").count(), 0);
    assert_eq!(line_codes.iter().filter(|&&(c, _)| c == "M3E-RAW-ELEVATION-SHAPE-TYPO").count(), 0);
    let touch_lines: Vec<u32> = doc.lang_rule_findings.iter().filter(|f| f.code() == "M3E-TOUCH-TARGET-AND-STATE-LAYER").map(|f| f.span.start_line()).collect();
    assert_eq!(touch_lines, vec![31, 32, 33]);
    Ok(())
}

#[rustfmt::skip]
#[test]
fn wrangler_config_dates_bindings_and_vars_kill_mutants() -> TestResult {
    let valid_toml = concat!(
        "name = \"edge-api\"\ncompatibility_date = \"2025-12-31\"\n",
        "compatibility_flags = [\"nodejs_compat\"]\n[vars]\nPUBLIC_ENV = \"production\"\nAPI_SECRET = \"\"\n",
        "[[d1_databases]]\nbinding = \"DB\"\nremote = false\n",
        "[[r2_buckets]]\nbinding = \"ASSETS\"\n[[kv_namespaces]]\nbinding = \"KV\"\n",
        "[[queues.producers]]\nbinding = \"QUEUE_P\"\n[[queues.consumers]]\nbinding = \"QUEUE_C\"\n",
        "[[durable_objects.bindings]]\nname = \"ROOM_DO\"\n[ai]\nbinding = \"AI\"\n",
    );
    let ok_doc = parse_rel("wrangler.toml", valid_toml)?;
    assert_eq!(ok_doc.tool_config_findings, Vec::new());
    let wr = ok_doc.web_edge.as_ref().and_then(|w| w.wrangler.as_ref()).ok_or("missing wrangler")?;
    assert!(wr.has_nodejs_compat && wr.has_d1 && wr.d1_r2_bindings.len() == 2);
    for b in ["DB", "ASSETS", "KV", "QUEUE_P", "QUEUE_C", "ROOM_DO", "AI", "PUBLIC_ENV", "API_SECRET"] {
        assert!(wr.declared_bindings.contains(b), "missing binding {b}");
    }
    let empty_d1 = parse_rel("wrangler.json", r#"{"compatibility_date":"2020-01-01","d1_databases":[],"r2_buckets":[{"binding":"   "}]}"#)?;
    let wr_empty = empty_d1.web_edge.as_ref().and_then(|w| w.wrangler.as_ref()).ok_or("missing")?;
    assert!(!wr_empty.has_d1 && wr_empty.declared_bindings.is_empty());
    for bad_date in ["2025-01-1", "2025/01-01", "2025-01/01", "2019-12-31", "2101-01-01", "2025-00-15", "2025-13-15", "2025-05-00", "2025-05-32", "abcd-ef-gh"] {
        let d = parse_rel("wrangler.toml", &format!("compatibility_date = \"{bad_date}\"\n"))?;
        assert_eq!(d.tool_config_findings.len(), 1, "date {bad_date} should fail");
    }
    let bad_wrangler = parse_rel("wrangler.toml", "compatibility_date = \"2025-01-01\"\n[vars]\nAPI_SECRET = \"sk-live\"\n[[d1_databases]]\nbinding = \"DB\"\nremote = true\n")?;
    assert_eq!(bad_wrangler.tool_config_findings.len(), 2);
    Ok(())
}
