//! AST-driven architectural guardrails enforcing SRP file limits, `#![no_std]` Functional Core
//! purity (`GF078`), single console-output boundary (`output.rs`), and zero lint-bypass attributes.

use std::fs;
use std::path::{Path, PathBuf};

use gf_core::{EffectSightingKind, RepoPath, UastDocument, Validation};
use gf_engine::load_workspace_config;
use gf_parsers::parse_source;
use gf_rules::check_architecture_purity;

const MAX_FILE_LINES: usize = 250;
const CLI_CONSOLE_BOUNDARIES: &[&str] = &[
    "crates/gf-cli/src/output.rs",
    "crates/gf-cli/src/tui/terminal.rs",
];

fn workspace_root() -> Result<PathBuf, String> {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    manifest_dir
        .parent()
        .and_then(Path::parent)
        .map(Path::to_path_buf)
        .ok_or_else(|| String::from("failed to resolve workspace root from CARGO_MANIFEST_DIR"))
}

fn collect_rust_files(dir: &Path, acc: &mut Vec<PathBuf>) -> Result<(), std::io::Error> {
    if !dir.is_dir() {
        return Ok(());
    }
    let mut entries: Vec<PathBuf> = fs::read_dir(dir)?
        .collect::<Result<Vec<_>, _>>()?
        .into_iter()
        .map(|entry| entry.path())
        .collect();
    entries.sort();

    for path in entries {
        if path.is_dir() {
            collect_rust_files(&path, acc)?;
        } else if path.extension().and_then(|ext| ext.to_str()) == Some("rs") {
            acc.push(path);
        }
    }
    Ok(())
}

fn parse_all_workspace_rust_docs(
    root: &Path,
) -> Result<Vec<UastDocument>, Box<dyn std::error::Error>> {
    let mut files = Vec::new();
    collect_rust_files(&root.join("crates"), &mut files)?;
    collect_rust_files(&root.join("tests"), &mut files)?;

    let mut docs = Vec::with_capacity(files.len());
    for file in files {
        let rel_str = file
            .strip_prefix(root)?
            .to_string_lossy()
            .replace('\\', "/");
        let repo_path = RepoPath::new(&rel_str)?;
        let content = fs::read_to_string(&file)?;
        docs.push(parse_source(&repo_path, &content)?);
    }
    Ok(docs)
}

#[test]
fn enforces_max_250_lines_per_rust_file() -> Result<(), Box<dyn std::error::Error>> {
    let root = workspace_root()?;
    let mut files = Vec::new();
    collect_rust_files(&root.join("crates"), &mut files)?;
    collect_rust_files(&root.join("tests"), &mut files)?;
    assert!(
        files.len() >= 20,
        "expected at least 20 Rust files in workspace, found {}",
        files.len()
    );

    let mut violations = Vec::new();
    for file in &files {
        let content = fs::read_to_string(file)?;
        let line_count = content.lines().count();
        if line_count > MAX_FILE_LINES {
            let rel = file.strip_prefix(&root).unwrap_or(file);
            violations.push(format!(
                "{} has {line_count} lines (max {MAX_FILE_LINES})",
                rel.display()
            ));
        }
    }
    assert_eq!(violations, Vec::<String>::new());
    Ok(())
}

#[test]
fn enforces_no_std_and_imperative_shell_boundaries_via_ast()
-> Result<(), Box<dyn std::error::Error>> {
    let root = workspace_root()?;
    let loaded = load_workspace_config(&root)?;
    let docs = parse_all_workspace_rust_docs(&root)?;

    assert!(
        loaded
            .analyzer
            .architecture
            .no_std_crates
            .contains("gf-core")
            && loaded
                .analyzer
                .architecture
                .no_std_crates
                .contains("gf-rules"),
        "expected `gf-core` and `gf-rules` in `[architecture].no_std_crates`"
    );

    if let Validation::Invalid(diags) = check_architecture_purity(&docs, &loaded.analyzer) {
        let rendered: Vec<String> = diags
            .iter()
            .map(|d| {
                format!(
                    "[{}] {}:{} {}",
                    d.rule_id,
                    d.path,
                    d.span.start_line(),
                    d.message
                )
            })
            .collect();
        assert_eq!(rendered, Vec::<String>::new());
    }

    let parser_io_leaks: Vec<String> = docs
        .iter()
        .filter(|d| d.path.as_str().starts_with("crates/gf-parsers/src/"))
        .flat_map(|d| {
            d.effect_sightings
                .iter()
                .filter(|e| e.kind == EffectSightingKind::IoImportOrCall)
                .map(move |e| format!("{}:{} ({})", d.path, e.span.start_line(), e.detail))
        })
        .collect();
    assert_eq!(parser_io_leaks, Vec::<String>::new());
    Ok(())
}

#[test]
fn enforces_console_output_and_no_allow_cheating_via_ast() -> Result<(), Box<dyn std::error::Error>>
{
    let root = workspace_root()?;
    let docs = parse_all_workspace_rust_docs(&root)?;

    let bypass_violations: Vec<String> = docs
        .iter()
        .flat_map(|doc| {
            doc.bypass_findings
                .iter()
                .map(move |bp| format!("{}:{} -> {}", doc.path, bp.span.start_line(), bp.directive))
        })
        .collect();

    let console_violations: Vec<String> = docs
        .iter()
        .filter(|doc| !CLI_CONSOLE_BOUNDARIES.contains(&doc.path.as_str()))
        .flat_map(|doc| {
            doc.effect_sightings
                .iter()
                .filter(|eff| eff.kind == EffectSightingKind::ConsoleOutput)
                .map(move |eff| format!("{}:{} -> {}", doc.path, eff.span.start_line(), eff.detail))
        })
        .collect();

    let pure_ui_io_leaks: Vec<String> = docs
        .iter()
        .filter(|doc| {
            let p = doc.path.as_str();
            p.starts_with("crates/gf-cli/src/ui/")
                || (p.starts_with("crates/gf-cli/src/tui/")
                    && p != "crates/gf-cli/src/tui/terminal.rs")
        })
        .flat_map(|doc| {
            doc.effect_sightings
                .iter()
                .filter(|eff| {
                    matches!(
                        eff.kind,
                        EffectSightingKind::IoImportOrCall | EffectSightingKind::ConsoleOutput
                    )
                })
                .map(move |eff| format!("{}:{} -> {}", doc.path, eff.span.start_line(), eff.detail))
        })
        .collect();

    assert_eq!(bypass_violations, Vec::<String>::new());
    assert_eq!(console_violations, Vec::<String>::new());
    assert_eq!(pure_ui_io_leaks, Vec::<String>::new());
    Ok(())
}
