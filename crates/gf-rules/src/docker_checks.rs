//! Contextual rules for `FeatureId::Docker`: stage pinning (`GF030`), non-root `USER` (`GF031`),
//! and `.dockerignore` project coherence (`GF034`).

use alloc::collections::BTreeSet;
use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;

use gf_core::{
    Diagnostic, DockerDocument, FeatureId, FromStage, LanguageId, ProjectContext, RepoPath, Span,
    UastDocument, Validation,
};

use crate::checks::{SourceLoc, rule_error as docker_error};
use crate::docker_instr_checks::check_instruction;

/// Evaluates `GF030`–`GF034` whenever `ctx.features` contains [`FeatureId::Docker`].
pub fn check_docker_feature(
    docs: &[UastDocument],
    ctx: &ProjectContext,
) -> Validation<(), Diagnostic> {
    if !ctx.features.contains(&FeatureId::Docker) {
        return Validation::Valid(());
    }

    let mut diags = Vec::new();
    let mut first_dockerfile: Option<&RepoPath> = None;

    for doc in docs {
        let Some(docker) = &doc.docker else {
            continue;
        };
        if first_dockerfile.is_none() {
            first_dockerfile = Some(&doc.path);
        }
        check_stages(&doc.path, docker, &mut diags);
        for instr in &docker.instructions {
            check_instruction(&doc.path, instr, &mut diags);
        }
    }

    if let Some(df_path) = first_dockerfile {
        check_dockerignore_coherence(df_path, ctx, &mut diags);
    }
    Validation::<(), Diagnostic>::from_errors(diags)
}

fn is_root_user(user: Option<&str>) -> bool {
    let Some(u) = user.map(str::trim) else {
        return true;
    };
    u.is_empty()
        || u.eq_ignore_ascii_case("root")
        || u == "0"
        || u.starts_with("root:")
        || u.starts_with("0:")
}

fn check_stages(path: &RepoPath, docker: &DockerDocument, out: &mut Vec<Diagnostic>) {
    let mut prior_aliases: BTreeSet<String> = BTreeSet::new();
    for stage in &docker.stages {
        check_single_stage_pinning(path, stage, &prior_aliases, out);
        if let Some(alias) = &stage.alias {
            prior_aliases.insert(alias.to_ascii_lowercase());
        }
    }

    if let Some(final_stage) = docker.stages.last()
        && !final_stage.image.eq_ignore_ascii_case("scratch")
        && is_root_user(final_stage.effective_user.as_deref())
    {
        out.push(docker_error(
            "GF031",
            format!(
                "final stage `{}` runs as `root` (missing non-root `USER`)",
                final_stage.image
            ),
            "Create a dedicated unprivileged user/UID and switch to it with `USER <non-root>` in the final runtime stage.",
            SourceLoc::new(path, final_stage.span),
        ));
    }
}

fn check_single_stage_pinning(
    path: &RepoPath,
    stage: &FromStage,
    prior_aliases: &BTreeSet<String>,
    out: &mut Vec<Diagnostic>,
) {
    let lower = stage.image.to_ascii_lowercase();
    if lower == "scratch" || prior_aliases.contains(&lower) {
        return;
    }
    let unpinned = stage.digest.is_none()
        && (stage.tag.is_none()
            || stage
                .tag
                .as_deref()
                .is_some_and(|t| t.eq_ignore_ascii_case("latest")));
    if unpinned {
        out.push(docker_error(
            "GF030",
            format!(
                "base image `{}` uses `:latest` or omits an explicit version/digest",
                stage.image
            ),
            "Pin base images to an explicit version tag (e.g. `rust:1.88-slim`) or an immutable `@sha256:...` digest.",
            SourceLoc::new(path, stage.span),
        ));
    }
}

fn required_dockerignore_entries(ctx: &ProjectContext) -> Vec<&'static str> {
    let mut req = alloc::vec![".git"];
    if ctx.languages.contains(&LanguageId::Rust) {
        req.push("target");
    }
    if ctx.languages.contains(&LanguageId::TypeScript) {
        req.push("node_modules");
    }
    if ctx.languages.contains(&LanguageId::Python) {
        req.push(".venv");
        req.push("__pycache__");
    }
    req
}

fn check_dockerignore_coherence(
    dockerfile_path: &RepoPath,
    ctx: &ProjectContext,
    out: &mut Vec<Diagnostic>,
) {
    let loc = SourceLoc::new(dockerfile_path, Span::FIRST);
    let Some(entries) = &ctx.dockerignore_entries else {
        out.push(docker_error(
            "GF034",
            String::from("missing `.dockerignore` at repository root alongside `Dockerfile`"),
            "Create a `.dockerignore` excluding `.git` and language build/dependency directories to prevent context bloat and secret leakage.",
            loc,
        ));
        return;
    };

    let missing: Vec<&str> = required_dockerignore_entries(ctx)
        .into_iter()
        .filter(|req| !entries.contains(*req))
        .collect();
    if !missing.is_empty() {
        out.push(docker_error(
            "GF034",
            format!(
                "`.dockerignore` is missing required exclusions: {}",
                missing.join(", ")
            ),
            "Add the missing `.git` and language build directories (`target`, `node_modules`, `.venv`, `__pycache__`) to `.dockerignore`.",
            loc,
        ));
    }
}
