//! Demonstrative multi-file fixtures for `GF020` (Type-2 clones), `GF021` (dead exports
//! vs `Astro` entrypoints), and `FeatureId::Docker` (`GF030`–`GF034`).

use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use gf_core::{FeatureId, FrameworkId, LanguageId};
use gf_engine::{CheckOptions, ScanScope, run_check_with_options};

static FIXTURE_SEQ: AtomicU64 = AtomicU64::new(0);

const BILLING_PY: &str = "def compute_invoice(prices, threshold):\n\
    \x20   total = 10\n\
    \x20   for amount in prices:\n\
    \x20       if amount > threshold:\n\
    \x20           total = total + amount\n\
    \x20       elif amount == 0:\n\
    \x20           total = total - 1\n\
    \x20       if total > 500:\n\
    \x20           total = total // 2\n\
    \x20   if total < 0:\n\
    \x20       return 0\n\
    \x20   if total == 42:\n\
    \x20       total = total + 7\n\
    \x20   if total > 9000:\n\
    \x20       return 9000\n\
    \x20   return total\n";

const SHIPPING_PY: &str = "def calculate_freight(weights, limit_kg):\n\
    \x20   acc = 99\n\
    \x20   for w in weights:\n\
    \x20       if w > limit_kg:\n\
    \x20           acc = acc + w\n\
    \x20       elif w == 999:\n\
    \x20           acc = acc - 5\n\
    \x20       if acc > 1000:\n\
    \x20           acc = acc // 3\n\
    \x20   if acc < 1:\n\
    \x20       return 1\n\
    \x20   if acc == 77:\n\
    \x20       acc = acc + 3\n\
    \x20   if acc > 5000:\n\
    \x20       return 5000\n\
    \x20   return acc\n";

const BAD_DOCKERFILE: &str = "FROM python:latest\n\
    WORKDIR app\n\
    ADD local.txt /app/\n\
    ENV DB_PASSWORD=\"changeme\"\n\
    RUN sudo pip install flask\n";

const HARDENED_DOCKERFILE: &str = "# syntax=docker/dockerfile:1\n\
    FROM rust:1.88-slim AS builder\n\
    WORKDIR /src\n\
    COPY . .\n\
    RUN cargo build \\\n\
        --release\n\
    FROM debian:bookworm-slim\n\
    WORKDIR /app\n\
    COPY --from=builder /src/target/release/demo /app/demo\n\
    USER 65532:65532\n\
    ENTRYPOINT [\"/app/demo\"]\n";

fn temp_fixture_dir(tag: &str) -> Result<PathBuf, Box<dyn std::error::Error>> {
    let nanos = SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos();
    let seq = FIXTURE_SEQ.fetch_add(1, Ordering::Relaxed);
    let dir = std::env::temp_dir().join(format!("gf-ctx-{tag}-{nanos}-{seq}"));
    fs::create_dir_all(&dir)?;
    Ok(dir)
}

#[test]
fn detects_disguised_type2_clone_across_files_and_via_v2_cache()
-> Result<(), Box<dyn std::error::Error>> {
    let dir = temp_fixture_dir("clones")?;
    fs::create_dir_all(dir.join("src"))?;
    fs::write(dir.join("src/billing.py"), BILLING_PY)?;
    fs::write(dir.join("src/shipping.py"), SHIPPING_PY)?;
    fs::write(
        dir.join("src/main.py"),
        "from billing import compute_invoice\nfrom shipping import calculate_freight\n\
         print(compute_invoice([1], 0) + calculate_freight([2], 0))\n",
    )?;

    let fresh = run_check_with_options(&dir, &CheckOptions::new(ScanScope::All, true))?;
    assert_eq!(fresh.cache_misses, 3);
    assert!(fresh.diagnostics.iter().any(|d| d.rule_id == "GF020"));

    let cached = run_check_with_options(&dir, &CheckOptions::new(ScanScope::All, false))?;
    assert_eq!((cached.cache_hits, cached.cache_misses), (3, 0));
    assert_eq!(fresh.diagnostics, cached.diagnostics);
    fs::remove_dir_all(&dir)?;
    Ok(())
}

#[test]
fn astro_framework_exempts_pages_entrypoints_while_flagging_dead_exports()
-> Result<(), Box<dyn std::error::Error>> {
    let dir = temp_fixture_dir("astro")?;
    fs::create_dir_all(dir.join("src/pages"))?;
    fs::create_dir_all(dir.join("src/lib"))?;
    fs::write(
        dir.join("package.json"),
        r#"{"name":"site","dependencies":{"astro":"^5.0.0","react":"^19.0.0"}}"#,
    )?;
    fs::write(
        dir.join("src/lib/used.ts"),
        "export function formatTitle(raw: string): string {\n    return raw.trim();\n}\n",
    )?;
    fs::write(
        dir.join("src/lib/dead.ts"),
        "export function unusedLegacyHelper(x: number): number {\n    return x * 2;\n}\n",
    )?;
    fs::write(
        dir.join("src/pages/index.tsx"),
        "import { formatTitle } from \"../lib/used\";\n\
         export function HomePage() {\n    return <h1>{formatTitle(\"Hi\")}</h1>;\n}\n",
    )?;

    let report = run_check_with_options(&dir, &CheckOptions::new(ScanScope::All, true))?;
    assert!(report.context.frameworks.contains(&FrameworkId::Astro));
    assert!(report.context.frameworks.contains(&FrameworkId::React));

    let dead: Vec<_> = report
        .diagnostics
        .iter()
        .filter(|d| d.rule_id == "GF021")
        .collect();
    assert_eq!(dead.len(), 1);
    assert_eq!(dead[0].path.as_str(), "src/lib/dead.ts");
    fs::remove_dir_all(&dir)?;
    Ok(())
}

#[test]
fn rust_plus_docker_feature_triggers_gf030_through_gf034_and_passes_when_hardened()
-> Result<(), Box<dyn std::error::Error>> {
    let dir = temp_fixture_dir("docker")?;
    fs::create_dir_all(dir.join("src"))?;
    fs::write(
        dir.join("Cargo.toml"),
        "[package]\nname = \"demo\"\nversion = \"0.1.0\"\nedition = \"2024\"\n",
    )?;
    fs::write(dir.join("src/main.rs"), "fn main() {}\n")?;
    fs::write(dir.join("Dockerfile"), BAD_DOCKERFILE)?;

    let bad = run_check_with_options(&dir, &CheckOptions::new(ScanScope::All, true))?;
    assert!(bad.context.languages.contains(&LanguageId::Rust));
    assert!(bad.context.features.contains(&FeatureId::Docker));
    assert!(
        bad.context
            .frameworks
            .contains(&FrameworkId::CargoWorkspace)
    );
    for expected in ["GF030", "GF031", "GF032", "GF033", "GF034"] {
        assert!(bad.diagnostics.iter().any(|d| d.rule_id == expected));
    }

    fs::write(dir.join(".dockerignore"), ".git\n/target/\n")?;
    fs::write(dir.join("Dockerfile"), HARDENED_DOCKERFILE)?;
    let clean = run_check_with_options(&dir, &CheckOptions::new(ScanScope::All, true))?;
    assert_eq!(clean.diagnostics, Vec::new());
    fs::remove_dir_all(&dir)?;
    Ok(())
}
