//! Pure `#![no_std]` governance (`GF070`: `garde.lock` & `AGENTS.md`) and
//! `[architecture]` crate effect-purity (`GF078`: `no_std`, `IO`, `Console`) evaluators.

use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;

use gf_core::{
    AgenticSyncStatus, Config, Diagnostic, EffectSightingKind, LockStatus, RepoPath, Span,
    UastDocument, Validation, glob_matches,
};

use crate::checks::{SourceLoc, rule_error};

/// Evaluates cryptographic `garde.lock` synchronization against `garde-fou.toml` (`GF070`).
#[must_use]
pub fn check_lock_integrity(lock_status: &LockStatus) -> Validation<(), Diagnostic> {
    let manifest = RepoPath::manifest();
    let loc = SourceLoc::new(&manifest, Span::FIRST);
    match lock_status {
        LockStatus::NotRequired | LockStatus::Verified => Validation::Valid(()),
        LockStatus::Missing => Validation::Invalid(alloc::vec![rule_error(
            "GF070",
            String::from("`garde.lock` is missing while `[integrity] require_lock = true`"),
            "Run `garde lock` and commit `garde.lock` alongside `garde-fou.toml` to seal analyzer thresholds.",
            loc,
        )]),
        LockStatus::Tampered { expected, actual } => Validation::Invalid(alloc::vec![rule_error(
            "GF070",
            format!(
                "`garde-fou.toml` integrity mismatch against `garde.lock` (locked={expected}, actual={actual})"
            ),
            "Never weaken `garde-fou.toml` thresholds to silence violations; if a config change is authorized by a human, regenerate `garde.lock` via `garde lock`.",
            loc,
        )]),
    }
}

/// Evaluates `AGENTS.md` managed-block and `.agents/skills/garde-fou/SKILL.md` governance (`GF070`).
#[must_use]
pub fn check_agentic_governance(status: &AgenticSyncStatus) -> Validation<(), Diagnostic> {
    let agents_path = RepoPath::agents_md();
    let loc = SourceLoc::new(&agents_path, Span::FIRST);
    match status {
        AgenticSyncStatus::NotApplicable | AgenticSyncStatus::Synced => Validation::Valid(()),
        AgenticSyncStatus::MissingAgentsMd => Validation::Invalid(alloc::vec![rule_error(
            "GF070",
            String::from(
                "managed `AGENTS.md` block is missing while `[agents] sync_policy = \"required\"`"
            ),
            "Run `garde sync` to generate the contextual `AGENTS.md` managed block and `.agents/skills/`.",
            loc,
        )]),
        AgenticSyncStatus::DriftedAgentsMd => Validation::Invalid(alloc::vec![rule_error(
            "GF070",
            String::from(
                "`AGENTS.md` managed block is out of sync with workspace context, `garde-fou.toml`, or installed skills"
            ),
            "Run `garde sync` to refresh `AGENTS.md` fragments and thresholds without altering custom instructions outside managed markers.",
            loc,
        )]),
        AgenticSyncStatus::MissingOrOutdatedCoreSkill => {
            Validation::Invalid(alloc::vec![rule_error(
                "GF070",
                String::from("embedded `.agents/skills/garde-fou/SKILL.md` is missing or outdated"),
                "Run `garde sync` to install or update `.agents/skills/garde-fou/SKILL.md`.",
                loc,
            )])
        }
    }
}

#[derive(Clone, Copy)]
struct CrateSrcTarget<'a> {
    crate_name: &'a str,
    is_lib_rs: bool,
}

fn resolve_src_crate_target(raw_path: &str) -> Option<CrateSrcTarget<'_>> {
    if let Some(after_crates) = raw_path.strip_prefix("crates/") {
        let (crate_name, rest) = after_crates.split_once('/')?;
        let in_src = rest.strip_prefix("src/")?;
        return Some(CrateSrcTarget {
            crate_name,
            is_lib_rs: in_src == "lib.rs",
        });
    }
    let in_src = raw_path.strip_prefix("src/")?;
    Some(CrateSrcTarget {
        crate_name: "root",
        is_lib_rs: in_src == "lib.rs",
    })
}

fn is_console_exempt_file(raw_path: &str, config: &Config) -> bool {
    raw_path == "crates/gf-cli/src/output.rs"
        || config
            .entrypoints
            .iter()
            .any(|pat| glob_matches(pat, raw_path))
}

fn check_doc_no_std_purity(
    doc: &UastDocument,
    target: CrateSrcTarget<'_>,
    out: &mut Vec<Diagnostic>,
) {
    if target.is_lib_rs
        && !doc
            .effect_sightings
            .iter()
            .any(|e| e.kind == EffectSightingKind::HasNoStdAttr)
    {
        out.push(rule_error(
            "GF078",
            format!(
                "crate `{}` (`{}`) must declare `#![no_std]` at crate root",
                target.crate_name, doc.path
            ),
            "Add `#![no_std]` (and `extern crate alloc;` when needed) to `src/lib.rs` as mandated by `[architecture].no_std_crates`.",
            SourceLoc::new(&doc.path, Span::FIRST),
        ));
    }
    for eff in &doc.effect_sightings {
        if eff.kind == EffectSightingKind::StdUsage {
            out.push(rule_error(
                "GF078",
                format!(
                    "forbidden `std` usage (`{}`) inside `#![no_std]` crate `{}`",
                    eff.detail, target.crate_name
                ),
                "Use `core::` and `alloc::` primitives instead of `std::` inside `no_std_crates` (gate unit-test `std` imports behind `#[cfg(test)]`).",
                SourceLoc::new(&doc.path, eff.span),
            ));
        }
    }
}

fn check_doc_io_and_console(
    doc: &UastDocument,
    crate_name: &str,
    config: &Config,
    out: &mut Vec<Diagnostic>,
) {
    let arch = &config.architecture;
    let forbid_io =
        !arch.io_allowed_crates.is_empty() && !arch.io_allowed_crates.contains(crate_name);
    let forbid_console = !arch.console_allowed_crates.is_empty()
        && !arch.console_allowed_crates.contains(crate_name)
        && !is_console_exempt_file(doc.path.as_str(), config);

    for eff in &doc.effect_sightings {
        match eff.kind {
            EffectSightingKind::IoImportOrCall if forbid_io => {
                out.push(rule_error(
                    "GF078",
                    format!(
                        "forbidden I/O effect (`{}`) in pure crate `{crate_name}`",
                        eff.detail
                    ),
                    "Keep pure domain/parser/rule crates free of filesystem, network, environment, and process I/O; isolate side effects in declared `io_allowed_crates`.",
                    SourceLoc::new(&doc.path, eff.span),
                ));
            }
            EffectSightingKind::ConsoleOutput if forbid_console => {
                out.push(rule_error(
                    "GF078",
                    format!(
                        "forbidden console output (`{}`) in crate `{crate_name}`",
                        eff.detail
                    ),
                    "Return structured values or diagnostics instead of writing to `stdout`/`stderr` outside declared `console_allowed_crates`.",
                    SourceLoc::new(&doc.path, eff.span),
                ));
            }
            _ => {}
        }
    }
}

/// Evaluates `[architecture]` `no_std_crates`, `io_allowed_crates`, and `console_allowed_crates` (`GF078`).
#[must_use]
pub fn check_architecture_purity(
    docs: &[UastDocument],
    config: &Config,
) -> Validation<(), Diagnostic> {
    let arch = &config.architecture;
    if arch.no_std_crates.is_empty()
        && arch.io_allowed_crates.is_empty()
        && arch.console_allowed_crates.is_empty()
    {
        return Validation::Valid(());
    }

    let mut diags = Vec::new();
    for doc in docs {
        let Some(target) = resolve_src_crate_target(doc.path.as_str()) else {
            continue;
        };
        if !arch.no_std_crates.is_empty()
            && (arch.no_std_crates.contains(target.crate_name) || arch.no_std_crates.contains("*"))
        {
            check_doc_no_std_purity(doc, target, &mut diags);
        }
        check_doc_io_and_console(doc, target.crate_name, config, &mut diags);
    }
    Validation::<(), Diagnostic>::from_errors(diags)
}
