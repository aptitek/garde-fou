//! GitHub Actions workflow & composite action security analyzer (`GHA-TEMPLATE-INJECTION`,
//! `GHA-PWN-REQUEST`, `GHA-PERMISSIONS-AND-PINNING`).

use gf_core::{LangRuleFinding, RepoPath, WorkflowViolation};

use super::anchors_merge::ResolvedYaml;
use super::events_tree::YamlScalar;
use super::gha_expr::{
    check_uses_pinning, has_pull_request_target_trigger, normalize_gha_expr,
    scan_scalar_template_injections,
};
use super::gha_run_bridge::{JobShellDefaults, bridge_gha_step_run};

fn check_step_uses(
    step: &ResolvedYaml,
    uses: &YamlScalar,
    has_pr_target: bool,
    out: &mut Vec<LangRuleFinding>,
) {
    check_uses_pinning(uses, out);
    let lower_uses = uses.value.trim().to_ascii_lowercase();
    if has_pr_target
        && lower_uses.starts_with("actions/checkout")
        && let Some(ref_s) = step
            .get("with")
            .and_then(|w| w.get("ref"))
            .and_then(ResolvedYaml::as_scalar)
    {
        let norm_ref = normalize_gha_expr(&ref_s.value);
        if norm_ref.contains("github.event.pull_request.head")
            || norm_ref.contains("github.head_ref")
        {
            out.push(LangRuleFinding::new(
                WorkflowViolation::PwnRequestCheckout,
                ref_s.span,
                "`pull_request_target` checks out untrusted PR head ref (`pwn request`)",
            ));
        }
    }
    if lower_uses.starts_with("actions/github-script")
        && let Some(script_s) = step
            .get("with")
            .and_then(|w| w.get("script"))
            .and_then(ResolvedYaml::as_scalar)
    {
        scan_scalar_template_injections(script_s, "with.script", out);
    }
}

fn check_step_security(
    step: &ResolvedYaml,
    has_pr_target: bool,
    defaults: &JobShellDefaults<'_>,
    out: &mut Vec<LangRuleFinding>,
) {
    if let Some(uses) = step.get("uses").and_then(ResolvedYaml::as_scalar) {
        check_step_uses(step, uses, has_pr_target, out);
    }
    if let Some(run_s) = step.get("run").and_then(ResolvedYaml::as_scalar) {
        scan_scalar_template_injections(run_s, "run", out);
        bridge_gha_step_run(step, run_s, defaults, out);
    }
}

fn check_workflow_permissions(
    root: &ResolvedYaml,
    jobs: &[(YamlScalar, ResolvedYaml)],
    out: &mut Vec<LangRuleFinding>,
) {
    match root.get_entry("permissions") {
        Some((_, ResolvedYaml::Scalar(s))) if s.value.trim().eq_ignore_ascii_case("write-all") => {
            out.push(LangRuleFinding::new(
                WorkflowViolation::PermissionsOrUnpinnedUses,
                s.span,
                "workflow grants global `permissions: write-all`",
            ));
        }
        Some((_, ResolvedYaml::Mapping { entries, .. })) => {
            if jobs.len() > 1
                && let Some((k, v)) = entries.iter().find(|(k, v)| {
                    k.value == "contents"
                        && v.as_scalar()
                            .is_some_and(|s| s.value.trim().eq_ignore_ascii_case("write"))
                })
                && jobs.iter().any(|(_, j)| j.get("permissions").is_none())
            {
                out.push(LangRuleFinding::new(
                    WorkflowViolation::PermissionsOrUnpinnedUses,
                    v.span(),
                    format!(
                        "workflow grants broad root `{}: write` across {} jobs; scope write permissions to individual jobs",
                        k.value,
                        jobs.len()
                    ),
                ));
            }
        }
        Some(_) => {}
        None => {
            if jobs.iter().any(|(_, job)| job.get("permissions").is_none()) {
                out.push(LangRuleFinding::new(
                    WorkflowViolation::PermissionsOrUnpinnedUses,
                    root.span(),
                    "workflow omits top-level `permissions:` while one or more jobs lack explicit `permissions:`",
                ));
            }
        }
    }
}

fn extract_run_shell_default(container: &ResolvedYaml) -> Option<&str> {
    container
        .get("defaults")
        .and_then(|d| d.get("run"))
        .and_then(|r| r.get("shell"))
        .and_then(ResolvedYaml::as_scalar)
        .map(|s| s.value.as_str())
}

fn analyze_single_job(
    job: &ResolvedYaml,
    wf_default_shell: Option<&str>,
    has_pr_target: bool,
    out: &mut Vec<LangRuleFinding>,
) {
    if let Some(job_uses) = job.get("uses").and_then(ResolvedYaml::as_scalar) {
        check_uses_pinning(job_uses, out);
    }
    let defaults = JobShellDefaults {
        workflow_shell: wf_default_shell,
        job_shell: extract_run_shell_default(job),
        runs_on: job
            .get("runs-on")
            .and_then(ResolvedYaml::as_scalar)
            .map(|s| s.value.as_str()),
    };
    if let Some(ResolvedYaml::Sequence { items: steps, .. }) = job.get("steps") {
        for step in steps {
            check_step_security(step, has_pr_target, &defaults, out);
        }
    }
}

/// Evaluates `GHA-*` rules and embedded `run:` Shell/PowerShell bridges on a workflow or composite action.
pub fn analyze_github_workflow(
    path: &RepoPath,
    root: &ResolvedYaml,
    out: &mut Vec<LangRuleFinding>,
) {
    if matches!(
        path.file_name().to_ascii_lowercase().as_str(),
        "action.yml" | "action.yaml"
    ) {
        if let Some(ResolvedYaml::Sequence { items, .. }) =
            root.get("runs").and_then(|r| r.get("steps"))
        {
            let defaults = JobShellDefaults::default();
            for step in items {
                check_step_security(step, false, &defaults, out);
            }
        }
        return;
    }

    let has_pr_target = has_pull_request_target_trigger(root);
    let wf_default_shell = extract_run_shell_default(root);
    let Some(ResolvedYaml::Mapping { entries: jobs, .. }) = root.get("jobs") else {
        check_workflow_permissions(root, &[], out);
        return;
    };
    check_workflow_permissions(root, jobs, out);
    for (_, job) in jobs {
        analyze_single_job(job, wf_default_shell, has_pr_target, out);
    }
}
