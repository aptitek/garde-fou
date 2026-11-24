//! Workspace graph tests for TypeScript ESM `.js` -> `.ts`/`.tsx` remapping, `import type`
//! vs runtime Tarjan cycles (`GF071`), Atomic Design `import type` boundaries (`GF072`),
//! declaration merging (`GF021`), `tsconfig` `extends`/`paths`, mock `node_modules/.bin`,
//! and Golden Invariant (`garde init --offline` -> `garde check --no-cache == 0`).

use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use gf_core::{Config, FeatureId, FrameworkId, LanguageId, ProjectContext, RepoPath};
use gf_engine::{CheckOptions, InitRequest, ScanScope, run_check_with_options, run_init};
use gf_parsers::parse_source;
use gf_rules::evaluate_workspace_sorted;

type TestRes = Result<(), Box<dyn std::error::Error>>;

static TS_WS_SEQ: AtomicU64 = AtomicU64::new(0);

fn fresh_ts_dir(tag: &str) -> Result<PathBuf, Box<dyn std::error::Error>> {
    let nanos = SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos();
    let seq = TS_WS_SEQ.fetch_add(1, Ordering::Relaxed);
    let pid = std::process::id();
    let dir = std::env::temp_dir().join(format!("gf-ts-ws-{tag}-{pid}-{nanos}-{seq}"));
    fs::create_dir_all(&dir)?;
    Ok(dir)
}

fn write_rel(root: &Path, rel: &str, content: &str) -> Result<(), std::io::Error> {
    let full = root.join(rel);
    if let Some(parent) = full.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(full, content)
}

#[cfg(unix)]
fn write_executable(root: &Path, rel: &str, script: &str) -> Result<(), std::io::Error> {
    use std::os::unix::fs::PermissionsExt as _;
    write_rel(root, rel, script)?;
    let full = root.join(rel);
    let mut perms = fs::metadata(&full)?.permissions();
    perms.set_mode(0o755);
    fs::set_permissions(&full, perms)
}

#[test]
fn import_type_vs_runtime_esm_cycles_atomic_boundaries_and_declaration_merging() -> TestRes {
    let cfg = Config::strict_defaults();
    let mut ui_ctx = ProjectContext::default();
    ui_ctx.languages.insert(LanguageId::TypeScript);
    ui_ctx.features.insert(FeatureId::Ui);

    let type_a = parse_source(
        &RepoPath::new("src/a.ts")?,
        "import type { B } from \"./b.js\";\nexport type A = { b?: B };\n",
    )?;
    let type_b = parse_source(
        &RepoPath::new("src/b.ts")?,
        "import { type A } from \"./a.js\";\nexport type B = { a?: A };\n",
    )?;
    let type_cycle_diags = evaluate_workspace_sorted(&[type_a, type_b], &ui_ctx, &cfg);
    assert!(
        !type_cycle_diags.iter().any(|d| d.rule_id == "GF071"),
        "pure `import type` cycle must NOT trigger GF071: {type_cycle_diags:#?}"
    );

    let val_a = parse_source(
        &RepoPath::new("src/a.ts")?,
        "import { g } from \"./b.js\";\nexport const f = (): number => g() + 1;\n",
    )?;
    let val_b = parse_source(
        &RepoPath::new("src/b.ts")?,
        "import { f } from \"./a.js\";\nexport const g = (): number => f() + 1;\n",
    )?;
    let val_cycle_diags = evaluate_workspace_sorted(&[val_a, val_b], &ui_ctx, &cfg);
    assert!(
        val_cycle_diags.iter().any(|d| d.rule_id == "GF071"),
        "runtime `.js` -> `.ts` cycle MUST trigger GF071"
    );

    let badge_atom = parse_source(
        &RepoPath::new("src/components/atoms/Badge.tsx")?,
        "import type { PageData } from \"../../pages/Home.js\";\nexport function Badge(p: PageData) {\n  return <span className=\"b\">{p.title}</span>;\n}\n",
    )?;
    let home_page = parse_source(
        &RepoPath::new("src/pages/Home.tsx")?,
        "import { Badge } from \"../components/atoms/Badge.jsx\";\nexport interface PageData { readonly title: string; }\nexport function Home() {\n  return <Badge title=\"Hi\" />;\n}\n",
    )?;
    let atomic_diags = evaluate_workspace_sorted(&[badge_atom, home_page], &ui_ctx, &cfg);
    assert!(
        atomic_diags
            .iter()
            .any(|d| d.rule_id == "GF072" && d.path.as_str() == "src/components/atoms/Badge.tsx"),
        "atom importing page type via `.js` must trigger GF072: {atomic_diags:#?}"
    );
    Ok(())
}

#[test]
fn declaration_merging_tsconfig_extends_and_paths_aliases_work_together() -> TestRes {
    let cfg = Config::strict_defaults();
    let mut ctx = ProjectContext::default();
    ctx.languages.insert(LanguageId::TypeScript);
    ctx.frameworks.insert(FrameworkId::React);

    let ts_base = parse_source(
        &RepoPath::new("tsconfig.base.json")?,
        "{ \"compilerOptions\": { \"strict\": true } }\n",
    )?;
    let ts_app = parse_source(
        &RepoPath::new("tsconfig.app.json")?,
        "{ \"extends\": \"./tsconfig.base.json\", \"compilerOptions\": { \"paths\": { \"@domain/*\": [\"src/domain/*\"] } } }\n",
    )?;
    let brand = parse_source(
        &RepoPath::new("src/domain/brand.ts")?,
        "export type UserId = string & { readonly __tag: unique symbol };\nexport const UserId = (raw: string): UserId => raw.trim() as UserId;\n",
    )?;
    let entry = parse_source(
        &RepoPath::new("src/index.ts")?,
        "import { UserId } from \"@domain/brand\";\nexport const rootId: UserId = UserId(\"u_1\");\n",
    )?;
    let ok_diags = evaluate_workspace_sorted(&[ts_base, ts_app.clone(), brand, entry], &ctx, &cfg);
    assert_eq!(ok_diags, Vec::new());

    let bad_base = parse_source(
        &RepoPath::new("tsconfig.base.json")?,
        "{ \"compilerOptions\": {} }\n",
    )?;
    let bad_diags = evaluate_workspace_sorted(&[bad_base, ts_app], &ctx, &cfg);
    assert!(bad_diags.iter().any(|d| d.rule_id == "GF081"));
    Ok(())
}

#[cfg(unix)]
fn setup_node_modules_mock_repo() -> Result<(PathBuf, PathBuf), Box<dyn std::error::Error>> {
    let dir = fresh_ts_dir("node-mocks")?;
    let status = Command::new("git")
        .args(["init", "-b", "main"])
        .current_dir(&dir)
        .status()?;
    if !status.success() {
        return Err("git init failed".into());
    }
    write_rel(
        &dir,
        "garde-fou.toml",
        "schema_version = 1\n[project]\nlanguages = [\"typescript\"]\n[tools.typescript]\noxlint = \"required\"\nbiome = \"off\"\ntsc = \"required\"\n[scanners]\nsecrets = \"off\"\nvulns = \"off\"\n",
    )?;
    write_rel(
        &dir,
        "tsconfig.json",
        "{ \"compilerOptions\": { \"strict\": true } }\n",
    )?;
    write_rel(&dir, "src/index.ts", "export const x: number = 1;\n")?;
    write_rel(&dir, "README.md", "# Docs\n")?;
    let sentinel = dir.join("ts_tools.sentinel");
    let ox_script = format!(
        "#!/bin/sh\necho oxlint >> \"{}\"\nprintf '{{\"diagnostics\":[{{\"message\":\"no eqeq\",\"code\":\"eslint(eqeqeq)\",\"severity\":\"error\",\"help\":\"Use ===\",\"filename\":\"src/index.ts\",\"labels\":[{{\"span\":{{\"line\":1,\"column\":14}}}}]}}]}}'\n",
        sentinel.display()
    );
    write_executable(&dir, "node_modules/.bin/oxlint", &ox_script)?;
    let tsc_script = format!(
        "#!/bin/sh\necho tsc >> \"{}\"\nprintf 'src/index.ts(1,14): error TS2322: Bad type.\\n'\n",
        sentinel.display()
    );
    write_executable(&dir, "node_modules/.bin/tsc", &tsc_script)?;
    Ok((dir, sentinel))
}

#[cfg(unix)]
#[test]
fn node_modules_bin_mocks_ingest_gf082_and_short_circuit_staged_readme() -> TestRes {
    let (dir, sentinel) = setup_node_modules_mock_repo()?;
    assert!(
        Command::new("git")
            .args(["add", "README.md"])
            .current_dir(&dir)
            .status()?
            .success()
    );
    let staged = run_check_with_options(&dir, &CheckOptions::new(ScanScope::StagedOnly, true))?;
    assert!(staged.is_clean());
    assert!(!sentinel.exists());

    let full = run_check_with_options(&dir, &CheckOptions::new(ScanScope::All, true))?;
    assert!(sentinel.is_file());
    let gf082: Vec<_> = full
        .diagnostics
        .iter()
        .filter(|d| d.rule_id == "GF082")
        .collect();
    assert_eq!(gf082.len(), 2);
    assert!(
        gf082
            .iter()
            .any(|d| d.message.contains("[oxlint:eslint(eqeqeq)]"))
    );
    assert!(gf082.iter().any(|d| d.message.contains("[tsc:TS2322]")));
    fs::remove_dir_all(&dir)?;
    Ok(())
}

fn scaffold_ts_stack_and_check(
    slug: &str,
    frameworks: BTreeSet<FrameworkId>,
    features: BTreeSet<FeatureId>,
) -> Result<Vec<gf_core::Diagnostic>, Box<dyn std::error::Error>> {
    let dir = fresh_ts_dir(slug)?;
    run_init(&InitRequest {
        root: dir.clone(),
        project_name: Some(String::from(slug)),
        languages: BTreeSet::from([LanguageId::TypeScript]),
        features,
        frameworks,
        run_git_init: true,
    })?;
    let toml_raw = fs::read_to_string(dir.join("garde-fou.toml"))?;
    assert!(toml_raw.contains("[tools.typescript]"));
    let mut opts = CheckOptions::new(ScanScope::All, true);
    opts.no_cache = true;
    let rep = run_check_with_options(&dir, &opts)?;
    fs::remove_dir_all(&dir)?;
    Ok(rep.diagnostics)
}

#[test]
fn golden_invariant_init_offline_react_storybook_i18n_tdd_and_astro_ui_content() -> TestRes {
    let react_diags = scaffold_ts_stack_and_check(
        "ts-react-full",
        BTreeSet::from([FrameworkId::React]),
        BTreeSet::from([
            FeatureId::Ui,
            FeatureId::I18n,
            FeatureId::Tdd,
            FeatureId::Storybook,
        ]),
    )?;
    assert_eq!(react_diags, Vec::new());

    let astro_diags = scaffold_ts_stack_and_check(
        "ts-astro-content",
        BTreeSet::from([FrameworkId::Astro]),
        BTreeSet::from([FeatureId::Ui, FeatureId::Content]),
    )?;
    assert_eq!(astro_diags, Vec::new());
    Ok(())
}
