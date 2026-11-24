//! Stage 2 multi-file Python package graph tests (`GF020`, `GF021`, `GF071`, `GF072`, `GF076`)
//! over a realistic `src/billing/{__init__.py, domain.py, repo.py, service.py}` + `tests/` package.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use gf_core::Diagnostic;
use gf_engine::{CheckOptions, CheckReport, ScanScope, run_check_with_options};

type TestRes = Result<(), Box<dyn std::error::Error>>;

static GRAPH_SEQ: AtomicU64 = AtomicU64::new(0);

const BASE_GARDE_TOML: &str = "\
schema_version = 1
[tools.python]
ruff = \"off\"
mypy = \"off\"
pyright = \"off\"
[scanners]
secrets = \"off\"
vulns = \"off\"
";

const ASYNC_CLONE_A: &str = "\
async def settle_europe_ledger(entries: list[int], cap: int) -> int:
    balance = 0
    for amount in entries:
        if amount > cap:
            balance = balance + cap
        elif amount < 0:
            balance = balance - 1
        else:
            balance = balance + amount
        if balance > 1000:
            balance = balance // 2
    if balance < 0:
        return 0
    if balance == 7:
        balance = balance + 3
    if balance > 5000:
        return 5000
    return balance
";

const ASYNC_CLONE_B: &str = "\
async def settle_apac_ledger(payments: list[int], ceiling: int) -> int:
    total = 0
    for item in payments:
        if item > ceiling:
            total = total + ceiling
        elif item < 0:
            total = total - 1
        else:
            total = total + item
        if total > 1000:
            total = total // 2
    if total < 0:
        return 0
    if total == 7:
        total = total + 3
    if total > 5000:
        return 5000
    return total
";

fn fresh_pkg_dir(tag: &str) -> Result<PathBuf, Box<dyn std::error::Error>> {
    let nanos = SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos();
    let seq = GRAPH_SEQ.fetch_add(1, Ordering::Relaxed);
    let pid = std::process::id();
    let dir = std::env::temp_dir().join(format!("gf-py-graph-{tag}-{pid}-{nanos}-{seq}"));
    fs::create_dir_all(&dir)?;
    Ok(dir)
}

fn write_files(root: &Path, files: &[(&str, &str)]) -> Result<(), std::io::Error> {
    for &(rel, body) in files {
        let full = root.join(rel);
        if let Some(parent) = full.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(full, body)?;
    }
    Ok(())
}

fn by_rule<'a>(rep: &'a CheckReport, rule: &str) -> Vec<&'a Diagnostic> {
    rep.diagnostics
        .iter()
        .filter(|d| d.rule_id == rule)
        .collect()
}

#[test]
fn detects_relative_and_absolute_python_cycle_and_boundary_violation() -> TestRes {
    let dir = fresh_pkg_dir("cycle-boundary")?;
    let toml_with_boundary = format!(
        "{BASE_GARDE_TOML}\n\
         [[boundaries]]\n\
         from = \"src/billing/domain.py\"\n\
         deny = [\"src/billing/infra_db.py\"]\n\
         reason = \"Domain model must stay pure and never import infrastructure adapters.\"\n"
    );
    write_files(
        &dir,
        &[
            ("garde-fou.toml", &toml_with_boundary),
            (
                "src/billing/__init__.py",
                "from .service import issue_invoice\n\n__all__ = [\"issue_invoice\"]\n",
            ),
            (
                "src/billing/infra_db.py",
                "def query_tax_rate() -> int:\n    return 20\n",
            ),
            (
                "src/billing/domain.py",
                "from dataclasses import dataclass\nfrom .service import normalize_cents\nfrom billing.infra_db import query_tax_rate\n\n@dataclass(frozen=True)\nclass Invoice:\n    cents: int\n\ndef compute_gross(raw: int) -> int:\n    return normalize_cents(raw) + query_tax_rate()\n",
            ),
            (
                "src/billing/service.py",
                "from billing.domain import Invoice, compute_gross\n\ndef normalize_cents(raw: int) -> int:\n    return max(0, raw)\n\ndef issue_invoice(raw: int) -> Invoice:\n    return Invoice(cents=compute_gross(raw))\n",
            ),
        ],
    )?;

    let rep = run_check_with_options(&dir, &CheckOptions::new(ScanScope::All, true))?;
    let cycles = by_rule(&rep, "GF071");
    assert_eq!(cycles.len(), 2);
    assert!(cycles.iter().all(|d| {
        d.message
            .contains("src/billing/domain.py -> src/billing/service.py -> src/billing/domain.py")
    }));

    let boundaries = by_rule(&rep, "GF072");
    assert_eq!(boundaries.len(), 1);
    assert_eq!(boundaries[0].path.as_str(), "src/billing/domain.py");
    assert_eq!(boundaries[0].span.start_line(), 3);
    assert!(boundaries[0].message.contains("billing.infra_db"));

    fs::remove_dir_all(&dir)?;
    Ok(())
}

#[test]
fn enforces_dunder_all_dead_exports_without_false_positives_on_reexports_or_fixtures() -> TestRes {
    let dir = fresh_pkg_dir("exports")?;
    write_files(
        &dir,
        &[
            ("garde-fou.toml", BASE_GARDE_TOML),
            (
                "src/billing/__init__.py",
                "from .domain import Invoice\nfrom .service import create_invoice\n\n__all__ = [\"Invoice\", \"create_invoice\"]\n",
            ),
            (
                "src/billing/domain.py",
                "from dataclasses import dataclass\n\n@dataclass(frozen=True)\nclass Invoice:\n    cents: int\n",
            ),
            (
                "src/billing/repo.py",
                "__all__ = [\"load_rate\"]\n\ndef load_rate() -> int:\n    return _internal_seed()\n\ndef _internal_seed() -> int:\n    return 5\n\ndef unexported_public_name() -> int:\n    return 9\n",
            ),
            (
                "src/billing/service.py",
                "from .domain import Invoice\nfrom .repo import load_rate\n\ndef create_invoice(base: int) -> Invoice:\n    return Invoice(cents=base + load_rate())\n\ndef orphan_discount(cents: int) -> int:\n    return cents - 1\n",
            ),
            (
                "tests/test_service.py",
                "import pytest\nfrom billing import create_invoice\n\n@pytest.fixture\ndef base_amount() -> int:\n    return 100\n\ndef test_creates_invoice(base_amount: int) -> None:\n    inv = create_invoice(base_amount)\n    assert inv.cents == 105\n",
            ),
        ],
    )?;

    let rep = run_check_with_options(&dir, &CheckOptions::new(ScanScope::All, true))?;
    let dead = by_rule(&rep, "GF021");
    assert_eq!(dead.len(), 1);
    assert_eq!(dead[0].path.as_str(), "src/billing/service.py");
    assert_eq!(dead[0].span.start_line(), 7);
    assert!(dead[0].message.contains("orphan_discount"));
    assert_eq!(rep.diagnostics.len(), 1);

    fs::remove_dir_all(&dir)?;
    Ok(())
}

#[test]
fn detects_async_type2_clones_and_validates_pytest_raises_vs_fake_tests() -> TestRes {
    let dir = fresh_pkg_dir("clones-tdd")?;
    let service_py = format!(
        "{ASYNC_CLONE_A}\n\ndef validate_positive(amount: int) -> int:\n    if amount <= 0:\n        raise ValueError(\"non-positive\")\n    return amount\n"
    );
    let repo_py = format!(
        "from .service import settle_europe_ledger, validate_positive\n\n{ASYNC_CLONE_B}\n\n__all__ = [\"settle_apac_ledger\", \"settle_europe_ledger\", \"validate_positive\"]\n"
    );
    write_files(
        &dir,
        &[
            ("garde-fou.toml", BASE_GARDE_TOML),
            (
                "src/billing/__init__.py",
                "from .repo import settle_apac_ledger, settle_europe_ledger, validate_positive\n\n__all__ = [\"settle_apac_ledger\", \"settle_europe_ledger\", \"validate_positive\"]\n",
            ),
            ("src/billing/service.py", &service_py),
            ("src/billing/repo.py", &repo_py),
            (
                "tests/test_service.py",
                "import pytest\nfrom billing import validate_positive\n\ndef test_raises_is_valid() -> None:\n    with pytest.raises(ValueError):\n        validate_positive(0)\n\ndef test_fake_no_assert() -> None:\n    validate_positive(10)\n\ndef test_tautology() -> None:\n    assert True\n",
            ),
        ],
    )?;

    let rep = run_check_with_options(&dir, &CheckOptions::new(ScanScope::All, true))?;
    let clones = by_rule(&rep, "GF020");
    assert_eq!(clones.len(), 2);
    assert!(
        clones
            .iter()
            .any(|d| d.path.as_str() == "src/billing/service.py"
                && d.message.contains("settle_europe_ledger"))
    );
    assert!(
        clones
            .iter()
            .any(|d| d.path.as_str() == "src/billing/repo.py"
                && d.message.contains("settle_apac_ledger"))
    );

    let tdd = by_rule(&rep, "GF076");
    let tdd_lines: Vec<u32> = tdd.iter().map(|d| d.span.start_line()).collect();
    assert_eq!(tdd_lines, vec![8, 11]);

    fs::remove_dir_all(&dir)?;
    Ok(())
}
