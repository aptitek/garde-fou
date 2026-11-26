//! Symmetric `(invalid, subtle_valid)` corpus & `proptest` invariants for modern exclusive
//! toolchains (`pnpm`, `uv`, Gradle `.kts`), modern Node.js (`NODE-*`), and MERN TypeScript
//! (`EXPRESS-*`, `MONGO-*`).

use gf_core::{MernViolation, NodeViolation, RepoPath, ToolConfigViolation, ToolchainViolation};
use gf_parsers::parse_source;
use proptest::prelude::*;

fn tc_codes(path: &str, src: &str) -> Result<Vec<(String, u32)>, Box<dyn std::error::Error>> {
    let doc = parse_source(&RepoPath::new(path)?, src)?;
    Ok(doc
        .tool_config_findings
        .iter()
        .filter_map(|f| match f.violation {
            ToolConfigViolation::Toolchain(v) => {
                Some((String::from(v.code()), f.span.start_line()))
            }
            _ => None,
        })
        .collect())
}

fn lang_codes(path: &str, src: &str) -> Result<Vec<(String, u32)>, Box<dyn std::error::Error>> {
    let doc = parse_source(&RepoPath::new(path)?, src)?;
    Ok(doc
        .lang_rule_findings
        .iter()
        .map(|f| (String::from(f.code()), f.span.start_line()))
        .collect())
}

fn has(list: &[(String, u32)], code: &str, line: u32) -> bool {
    list.iter().any(|(c, l)| c == code && *l == line)
}

#[test]
fn pnpm_uv_and_gradle_exclusive_toolchain_corpus() -> Result<(), Box<dyn std::error::Error>> {
    let (pnpm, uv, gradle) = (
        ToolchainViolation::PnpmOnly.code(),
        ToolchainViolation::UvOnly.code(),
        ToolchainViolation::GradleKtsOnly.code(),
    );
    assert!(has(&tc_codes("package-lock.json", "{}")?, pnpm, 1));
    assert!(has(&tc_codes("yarn.lock", "")?, pnpm, 1));
    let bad_pkg = r#"{"name":"a","scripts":{"build":"npm run compile"}}"#;
    assert!(has(&tc_codes("package.json", bad_pkg)?, pnpm, 1));
    let bad_df = "FROM node:22-slim\nRUN npm ci\nRUN pip install -r requirements.txt\n";
    let df_codes = tc_codes("Dockerfile", bad_df)?;
    assert!(has(&df_codes, pnpm, 2) && has(&df_codes, uv, 3));
    assert!(has(&tc_codes("requirements.txt", "flask==3.0\n")?, uv, 1));
    let bad_nb = "{\"cells\":[{\"source\":[\"!pip install pandas\\n\"]}]}\n";
    assert!(has(&tc_codes("notebooks/eda.ipynb", bad_nb)?, uv, 1));
    assert!(has(&tc_codes("build.gradle", "plugins {}\n")?, gradle, 1));
    let bad_cat = "[versions]\nktor = \"3.0.0-SNAPSHOT\"\n";
    assert!(has(
        &tc_codes("gradle/libs.versions.toml", bad_cat)?,
        gradle,
        2
    ));

    let good_pkg = r#"{"name":"a","type":"module","packageManager":"pnpm@10.6.5","scripts":{"build":"pnpm exec tsc"}}"#;
    let good_df = "FROM node:22-slim\nRUN corepack enable && pnpm install --frozen-lockfile\nRUN uv sync --locked\n";
    assert_eq!(tc_codes("package.json", good_pkg)?.len(), 0);
    assert_eq!(tc_codes("Dockerfile", good_df)?.len(), 0);
    Ok(())
}

#[test]
fn modern_nodejs_protocol_sync_io_and_esm_corpus() -> Result<(), Box<dyn std::error::Error>> {
    let bad = "\
import fs from \"fs\";
export async function handler(p: string) {
  const raw = fs.readFileSync(p, \"utf8\");
  const buf = new Buffer(raw);
  process.exit(1);
  return buf;
}
";
    let findings = lang_codes("src/handler.ts", bad)?;
    assert!(has(&findings, NodeViolation::ProtocolImport.code(), 1));
    assert!(has(&findings, NodeViolation::NoSyncIoInHandler.code(), 3));
    assert!(has(&findings, NodeViolation::ModernEsmHygiene.code(), 4));
    assert!(has(&findings, NodeViolation::ModernEsmHygiene.code(), 5));

    let good = "\
import { readFile } from \"node:fs/promises\";
export async function handler(p: string): Promise<Buffer> {
  const raw = await readFile(p, \"utf8\");
  return Buffer.from(raw);
}
";
    let good_node: Vec<_> = lang_codes("src/good.ts", good)?
        .into_iter()
        .filter(|(c, _)| c.starts_with("NODE-"))
        .collect();
    assert_eq!(good_node, Vec::<(String, u32)>::new());
    Ok(())
}

#[test]
fn mern_express_and_mongoose_security_corpus() -> Result<(), Box<dyn std::error::Error>> {
    let bad = "\
export const app = express();
app.use(cors({ origin: \"*\", credentials: true }));
app.post(\"/login\", async (req, res) => {
  const dto = req.body as LoginDto;
  await UserModel.findOne({ email: dto.email });
  await UserModel.find({ $where: \"this.admin\" });
  await UserModel.deleteMany({});
  res.status(500).json({ err: err.stack });
});
app.listen(3000);
";
    let findings = lang_codes("apps/api/src/app.ts", bad)?;
    let (cors, cast, inj, unb, listen) = (
        MernViolation::ExpressInsecureCorsOrErrorLeak.code(),
        MernViolation::ExpressUnvalidatedRequestCast.code(),
        MernViolation::MongoNoSqlInjection.code(),
        MernViolation::MongoUnboundedQueryOrMutation.code(),
        MernViolation::ExpressListenInAppModule.code(),
    );
    assert!(has(&findings, cors, 2) && has(&findings, cast, 4) && has(&findings, inj, 5));
    assert!(has(&findings, inj, 6) && has(&findings, unb, 6) && has(&findings, unb, 7));
    assert!(has(&findings, cors, 8) && has(&findings, listen, 10));

    let good = "\
export const app = express();
app.post(\"/login\", async (req, res) => {
  const dto = LoginSchema.parse(req.body);
  const user = await UserModel.findOne({ email: { $eq: dto.email } }).lean();
  res.json({ ok: Boolean(user) });
});
";
    assert_eq!(lang_codes("apps/api/src/clean_app.ts", good)?.len(), 0);
    Ok(())
}

proptest! {
    #[test]
    fn mern_and_toolchain_parsers_never_panic_on_arbitrary_utf8(raw in "\\PC{0,160}") {
        for p in ["package.json", "Dockerfile", "gradle/libs.versions.toml", "src/api.ts"] {
            if let Ok(rp) = RepoPath::new(p)
                && let Ok(doc) = parse_source(&rp, &raw)
            {
                prop_assert!(doc.effective_lines <= doc.total_lines);
            }
        }
    }

    #[test]
    fn node_and_mern_line_numbers_covary_exactly_with_leading_comments(k in 0_u32..10_u32) {
        let prefix = "// pad\n".repeat(usize::try_from(k).unwrap_or(0));
        let src = format!("{prefix}import fs from \"fs\";\n");
        if let Ok(rp) = RepoPath::new("src/f.ts")
            && let Ok(doc) = parse_source(&rp, &src)
        {
            prop_assert_eq!(doc.lang_rule_findings[0].span.start_line(), k + 1);
        }
    }
}
