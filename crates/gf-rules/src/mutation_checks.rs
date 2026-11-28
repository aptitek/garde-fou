//! Pure `#![no_std]` mutation-testing fortress evaluator (`GF052`): anti-vacuity,
//! zero-survivor critical module globs, and surgical per-survivor diagnostics.

use alloc::collections::BTreeSet;
use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;

use gf_core::{
    Diagnostic, MutationConfig, MutationSummary, RepoPath, Severity, Span, SurvivedMutant,
    Validation, glob_matches,
};

const MAX_REPORTED_SURVIVORS: usize = 25;

const SURVIVOR_REMEDIATION: &str = "Add a unit or property-based test (`proptest`/`fast-check`/`Hypothesis`) asserting the exact observable output difference when this expression is mutated; NEVER delete valid domain code or add `#[mutants::skip]`/`Stryker disable` to silence a surviving mutant.";

const VACUOUS_REMEDIATION: &str = "Run your stack's mutation suite (`cargo mutants`, `stryker`, `mutmut`, `gremlins`, `pitest`, `dotnet stryker`, `infection`, or `mutation_test`) over real `Code` modules before running mutation-gated checks; never filter out all source files to fake a clean report.";

fn build_survivor_diag(survivor: &SurvivedMutant, critical: bool) -> Diagnostic {
    let fn_label = survivor.function_name.as_deref().unwrap_or("<module>");
    let message = if critical {
        format!(
            "zero-tolerance surviving mutant in critical module `{fn_label}` ([{}]: {})",
            survivor.mutator, survivor.replacement
        )
    } else {
        format!(
            "surviving mutant in `{fn_label}` ([{}]: {}) was not killed by any test",
            survivor.mutator, survivor.replacement
        )
    };
    Diagnostic {
        rule_id: String::from("GF052"),
        severity: Severity::Error,
        message,
        remediation: String::from(SURVIVOR_REMEDIATION),
        measured: 1,
        threshold: 0,
        path: survivor.path.clone(),
        span: survivor.span,
    }
}

fn collect_zero_survivor_diags<'a>(
    survivors: &'a [SurvivedMutant],
    zero_globs: &[String],
    out: &mut Vec<Diagnostic>,
) -> BTreeSet<&'a SurvivedMutant> {
    let mut matched = BTreeSet::new();
    if zero_globs.is_empty() {
        return matched;
    }
    for survivor in survivors {
        let hits_critical = zero_globs
            .iter()
            .any(|pat| glob_matches(pat, survivor.path.as_str()));
        if hits_critical {
            matched.insert(survivor);
            out.push(build_survivor_diag(survivor, true));
        }
    }
    matched
}

#[derive(Clone, Copy)]
struct ScoreGateSpec<'a> {
    required_score: u32,
    report_path: &'a RepoPath,
}

fn append_global_score_diags(
    sum: &MutationSummary,
    sorted_survivors: &[SurvivedMutant],
    already_reported: &BTreeSet<&SurvivedMutant>,
    gate: ScoreGateSpec<'_>,
) -> Vec<Diagnostic> {
    let actual_score = sum.score_percent_floor().unwrap_or(0);
    if actual_score >= gate.required_score {
        return Vec::new();
    }
    let mut out = alloc::vec![Diagnostic {
        rule_id: String::from("GF052"),
        severity: Severity::Error,
        message: format!(
            "mutation score is {actual_score}% (caught={}, timeout={}, missed={}, viable={}), below required {}%",
            sum.caught,
            sum.timeout,
            sum.missed,
            sum.total_viable(),
            gate.required_score
        ),
        remediation: String::from(SURVIVOR_REMEDIATION),
        measured: actual_score,
        threshold: gate.required_score,
        path: gate.report_path.clone(),
        span: Span::FIRST,
    }];

    for survivor in sorted_survivors
        .iter()
        .filter(|s| !already_reported.contains(s))
        .take(MAX_REPORTED_SURVIVORS)
    {
        out.push(build_survivor_diag(survivor, false));
    }
    out
}

/// Evaluates a normalized [`MutationSummary`] against `[mutation]` (`GF052`) applicatively.
#[must_use]
pub fn evaluate_mutation_summary(
    summary: Option<&MutationSummary>,
    config: &MutationConfig,
    report_path: &RepoPath,
) -> Validation<(), Diagnostic> {
    if config.min_mutation_score.is_none() && config.zero_survivor_globs.is_empty() {
        return Validation::Valid(());
    }
    let fallback_req = config.min_mutation_score.unwrap_or(100);

    let Some(sum) = summary.filter(|s| s.total_viable() > 0) else {
        return Validation::invalid_one(Diagnostic {
            rule_id: String::from("GF052"),
            severity: Severity::Error,
            message: format!(
                "mutation report `{report_path}` is missing or contains zero viable mutants"
            ),
            remediation: String::from(VACUOUS_REMEDIATION),
            measured: 0,
            threshold: fallback_req,
            path: report_path.clone(),
            span: Span::FIRST,
        });
    };

    let mut sorted = sum.survivors.clone();
    sorted.sort();
    let mut diags = Vec::new();
    let critical_set =
        collect_zero_survivor_diags(&sorted, &config.zero_survivor_globs, &mut diags);

    if let Some(required_score) = config.min_mutation_score {
        diags.extend(append_global_score_diags(
            sum,
            &sorted,
            &critical_set,
            ScoreGateSpec {
                required_score,
                report_path,
            },
        ));
    }
    Validation::<(), Diagnostic>::from_errors(diags)
}
