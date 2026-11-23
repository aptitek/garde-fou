//! Architectural guardrails enforcing SRP file limits and Functional Core purity.

use std::fs;
use std::path::{Path, PathBuf};

const MAX_FILE_LINES: usize = 250;

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

fn production_prefix(source: &str) -> &str {
    source
        .split_once("#[cfg(test)]")
        .map_or(source, |(prod, _)| prod)
}

#[test]
fn enforces_max_250_lines_per_rust_file() -> Result<(), Box<dyn std::error::Error>> {
    let root = workspace_root()?;
    let mut files = Vec::new();
    collect_rust_files(&root.join("crates"), &mut files)?;
    collect_rust_files(&root.join("tests"), &mut files)?;
    assert!(
        files.len() >= 5,
        "expected at least 5 Rust files in workspace, found {}",
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
    assert!(
        violations.is_empty(),
        "SRP file length violations:\n{}",
        violations.join("\n")
    );
    Ok(())
}

#[test]
fn enforces_no_std_and_imperative_shell_boundaries() -> Result<(), Box<dyn std::error::Error>> {
    let root = workspace_root()?;
    for pure_crate in ["gf-core", "gf-rules"] {
        let lib_path = root
            .join("crates")
            .join(pure_crate)
            .join("src")
            .join("lib.rs");
        let lib_src = fs::read_to_string(&lib_path)?;
        assert!(
            lib_src.contains("#![no_std]"),
            "{} must declare #![no_std]",
            lib_path.display()
        );
        assert!(
            lib_src.contains("extern crate alloc;"),
            "{} must declare `extern crate alloc;`",
            lib_path.display()
        );
    }

    let forbidden_io = ["std::fs", "std::process", "std::env"];
    for io_free_crate in ["gf-core", "gf-rules", "gf-parsers"] {
        let mut src_files = Vec::new();
        collect_rust_files(
            &root.join("crates").join(io_free_crate).join("src"),
            &mut src_files,
        )?;
        for file in src_files {
            let content = fs::read_to_string(&file)?;
            for token in forbidden_io {
                assert!(
                    !content.contains(token),
                    "forbidden I/O import `{token}` found in {}",
                    file.display()
                );
            }
            if io_free_crate != "gf-parsers" {
                let prod = production_prefix(&content);
                assert!(
                    !prod.contains("extern crate std") && !prod.contains("use std::"),
                    "`std` usage outside #[cfg(test)] in {}",
                    file.display()
                );
            }
        }
    }
    Ok(())
}

#[test]
fn enforces_console_output_and_no_allow_cheating() -> Result<(), Box<dyn std::error::Error>> {
    let root = workspace_root()?;
    let allowed_output = root
        .join("crates")
        .join("gf-cli")
        .join("src")
        .join("output.rs");
    let print_macro = concat!("print", "ln!(");
    let eprint_macro = concat!("eprint", "ln!(");
    let allow_attr = concat!("#[", "allow(");

    let mut files = Vec::new();
    collect_rust_files(&root.join("crates"), &mut files)?;
    collect_rust_files(&root.join("tests"), &mut files)?;

    for file in files {
        let content = fs::read_to_string(&file)?;
        assert!(
            !content.contains(allow_attr),
            "forbidden attribute found in {} (use #[expect(..., reason = ...)] only when architecturally justified)",
            file.display()
        );
        if file != allowed_output {
            assert!(
                !content.contains(print_macro) && !content.contains(eprint_macro),
                "console macro found outside output boundary in {}",
                file.display()
            );
        }
    }
    Ok(())
}
