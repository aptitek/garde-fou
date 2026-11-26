//! Pure `#![no_std]` workspace rules for modern toolchains (`GRADLE-USE-VERSION-CATALOG`)
//! and MERN frontend/backend isolation (`GF072`).

use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;

use gf_core::{
    Diagnostic, FrameworkId, ProjectContext, Severity, ToolchainViolation, UastDocument, Validation,
};

const BACKEND_ONLY_IMPORTS: &[&str] = &[
    "express",
    "mongoose",
    "mongodb",
    "node:fs",
    "node:fs/promises",
    "node:child_process",
    "fs",
    "fs/promises",
    "child_process",
];

fn is_frontend_path(path: &str) -> bool {
    path.starts_with("apps/web/")
        || path.starts_with("client/")
        || path.strip_suffix(".tsx").is_some()
        || path.strip_suffix(".jsx").is_some()
}

fn is_backend_only_import(module_source: &str) -> bool {
    let clean = module_source.trim().trim_matches(['"', '\'']);
    BACKEND_ONLY_IMPORTS
        .iter()
        .any(|&m| clean == m || clean.starts_with(&format!("{m}/")))
}

/// Evaluates workspace-level Gradle Version Catalog enforcement (`GF081`) and MERN
/// frontend/backend architectural boundaries (`GF072`).
#[must_use]
pub fn check_workspace_toolchains_and_mern(
    docs: &[UastDocument],
    context: &ProjectContext,
) -> Validation<(), Diagnostic> {
    let mut out = Vec::new();
    check_gradle_catalog(docs, &mut out);
    check_mern_boundaries(docs, context, &mut out);
    if out.is_empty() {
        Validation::Valid(())
    } else {
        Validation::Invalid(out)
    }
}

fn check_gradle_catalog(docs: &[UastDocument], out: &mut Vec<Diagnostic>) {
    let has_catalog = docs
        .iter()
        .filter_map(|d| d.toolchain_meta.as_ref())
        .any(|m| m.is_gradle_version_catalog);
    if !has_catalog {
        return;
    }
    for doc in docs {
        let Some(meta) = &doc.toolchain_meta else {
            continue;
        };
        for (span, coord) in &meta.hardcoded_gradle_deps {
            let v = ToolchainViolation::GradleUseVersionCatalog;
            out.push(Diagnostic {
                rule_id: String::from("GF081"),
                severity: Severity::Error,
                path: doc.path.clone(),
                span: *span,
                message: format!(
                    "[{}] hardcoded dependency `{coord}` in `build.gradle.kts`; use `libs.*` from `gradle/libs.versions.toml`",
                    v.code()
                ),
                measured: 1,
                threshold: 0,
                remediation: String::from(v.remediation()),
            });
        }
    }
}

fn check_mern_boundaries(
    docs: &[UastDocument],
    context: &ProjectContext,
    out: &mut Vec<Diagnostic>,
) {
    let is_mern = context.frameworks.contains(&FrameworkId::React)
        && (context.frameworks.contains(&FrameworkId::Express)
            || context.frameworks.contains(&FrameworkId::Mongoose));
    if !is_mern {
        return;
    }
    for doc in docs.iter().filter(|d| is_frontend_path(d.path.as_str())) {
        for imp in &doc.imports {
            if is_backend_only_import(&imp.module_source) {
                out.push(Diagnostic {
                    rule_id: String::from("GF072"),
                    severity: Severity::Error,
                    path: doc.path.clone(),
                    span: imp.span,
                    message: format!(
                        "MERN boundary violation: frontend module `{}` must not import backend/Node package `{}`",
                        doc.path, imp.module_source
                    ),
                    measured: 1,
                    threshold: 0,
                    remediation: String::from(
                        "Keep `express`, `mongoose`, `mongodb`, and `node:fs`/`node:child_process` strictly inside backend packages (`apps/api/**`); share pure types via `packages/contracts`.",
                    ),
                });
            }
        }
    }
}
