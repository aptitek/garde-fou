//! Symmetric `(invalid, subtle_valid)` AST & cross-bridge corpus for Shell (`sh`/`bash`),
//! PowerShell (`.ps1`/`.psm1`), `Dockerfile RUN`, Tier 2 configs, and Tier 3 JSON parsers.

use std::collections::BTreeSet;

use gf_core::{Config, FeatureId, ProjectContext, RepoPath};
use gf_parsers::parse_source;
use gf_rules::{evaluate_document, evaluate_workspace_sorted};

type Res = Result<(), Box<dyn std::error::Error>>;

fn eval_local_codes(
    rel_path: &str,
    src: &str,
) -> Result<Vec<(String, u32)>, Box<dyn std::error::Error>> {
    let doc = parse_source(&RepoPath::new(rel_path)?, src)?;
    let diags = evaluate_document(&doc, &Config::strict_defaults());
    Ok(diags
        .into_iter()
        .map(|d| (format!("{}:{}", d.rule_id, d.message), d.span.start_line()))
        .collect())
}

fn has_code_at(items: &[(String, u32)], needle: &str, line: u32) -> bool {
    items
        .iter()
        .any(|(msg, l)| *l == line && msg.contains(needle))
}

#[test]
fn shell_dialect_and_contextual_quoting_exemptions_are_exact() -> Res {
    let bad_posix = "\
#!/bin/sh
set -eu
if [[ -f \"$1\" ]]; then
    rm $1
fi
";
    let findings = eval_local_codes("scripts/bad.sh", bad_posix)?;
    assert!(has_code_at(&findings, "[SH-SC3000]", 3), "got {findings:?}");
    assert!(has_code_at(&findings, "[SH-SC2086]", 4), "got {findings:?}");

    let good_bash = "\
#!/bin/bash
set -euo pipefail
copy=$1
if [[ -f $copy ]]; then
    case $copy in
        *.txt) total=$(( 1 + ${#copy} )) ;;
        *) total=$? ;;
    esac
    printf '%s\\n' \"$total\"
fi
";
    assert_eq!(eval_local_codes("scripts/good.sh", good_bash)?, Vec::new());
    Ok(())
}

struct CorpusPair<'a> {
    bad_path: &'a str,
    bad_src: &'a str,
    expected: &'a [(&'a str, u32)],
    clean_path: &'a str,
    clean_src: &'a str,
}

fn assert_expected_codes_and_clean(pair: &CorpusPair<'_>) -> Res {
    let found = eval_local_codes(pair.bad_path, pair.bad_src)?;
    for &(code, line) in pair.expected {
        assert!(
            has_code_at(&found, code, line),
            "missing {code}@{line} in {found:?}"
        );
    }
    assert_eq!(
        eval_local_codes(pair.clean_path, pair.clean_src)?,
        Vec::new()
    );
    Ok(())
}

const SHELL_HAZARDS_PAIR: CorpusPair<'static> = CorpusPair {
    bad_path: "scripts/hazards.bash",
    bad_src: "#!/bin/bash\nlocal_fn() {\n    local out=\"$(uname -s)\"\n    cd \"$1\"\n    echo \"$out\"\n}\nfor item in $(ls); do\n    rm -rf \"$BUILD_DIR/\"*\n    eval \"$item\"\ndone\n# shellcheck disable=all\n",
    expected: &[
        ("[SH-STRICT-MODE]", 1),
        ("[SH-SC2155]", 3),
        ("[SH-SC2164]", 4),
        ("[SH-SC2046]", 7),
        ("[SH-SC2115]", 8),
        ("[SH-NO-EVAL]", 9),
        ("GF070:", 11),
    ],
    clean_path: "scripts/clean.bash",
    clean_src: "#!/bin/bash\nset -eu\nset -o pipefail\n# shellcheck disable=SC2034 -- consumed by child process environment\nsafe_fn() {\n    local out\n    out=\"$(uname -s)\"\n    cd \"$1\" || return 1\n    printf '%s\\n' \"$out\"\n}\nrm -rf \"${BUILD_DIR:?}/\"*\nrm -rf \"$TMP_DIR\"\n",
};

const PS_HAZARDS_PAIR: CorpusPair<'static> = CorpusPair {
    bad_path: "scripts/bad.ps1",
    bad_src: "param(\n    [string]$Password\n)\nif ($items -eq $null) {\n    Invoke-Expression $cmd\n}\ntry { Get-Item 'x' } catch {\n    # swallowed\n}\n$global:SharedState = 42\nfunction Invoke-Suppressed {\n    [Diagnostics.CodeAnalysis.SuppressMessageAttribute('PSAvoidGlobalVars', '')]\n    param()\n}\n",
    expected: &[
        ("[PS-STRICT-HEADER]", 1),
        ("[PS-PLAIN-SECRET-PARAM]", 2),
        ("[PS-NULL-LEFT]", 4),
        ("[PS-NO-IEX]", 5),
        ("[PS-EMPTY-CATCH]", 7),
        ("[PS-GLOBAL-VAR]", 10),
        ("GF070:", 12),
    ],
    clean_path: "scripts/clean.ps1",
    clean_src: "<#\n.SYNOPSIS\n    Safe script with Invoke-Expression | iex inside doc comment.\n#>\n[Diagnostics.CodeAnalysis.SuppressMessageAttribute('PSAvoidUsingWriteHost', '', Justification = 'Intentional interactive CLI progress')]\nparam(\n    [SecureString]$Password,\n    [PSCredential]$Credential,\n    [switch]$ForceSecretSync\n)\nSet-StrictMode -Version Latest\n$ErrorActionPreference = 'Stop'\n$literalCode = @\"\nInvoke-Expression $bad\nif ($arr -eq $null) { }\n\"@\nif ($null -eq $literalCode -or $LASTEXITCODE -eq $null) {\n    try { Write-Output 'ok' } catch { Write-Error $_ }\n}\n",
};

#[test]
fn shell_and_powershell_symmetric_hazard_corpora_are_exact() -> Res {
    for pair in [&SHELL_HAZARDS_PAIR, &PS_HAZARDS_PAIR] {
        assert_expected_codes_and_clean(pair)?;
    }
    Ok(())
}

#[test]
fn dockerfile_run_bridge_remaps_exact_lines_and_catches_hadolint_rules() -> Res {
    let bad_df = "\
FROM debian:12.8-slim AS builder
RUN curl -fsSL https://example.com/a.tgz | tar -xz
RUN echo \"step 1\" && \\
    echo \"step 2\" && \\
    echo \"step 3\" && \\
    if [[ -f /etc/os-release ]]; then echo ok; fi
COPY --from=ghost /out /app/out
USER appuser
CMD app --serve
";
    let doc = parse_source(&RepoPath::new("Dockerfile")?, bad_df)?;
    let mut ctx = ProjectContext::default();
    ctx.features.insert(FeatureId::Docker);
    ctx.dockerignore_entries = Some(BTreeSet::from([String::from(".git")]));
    let mut all = evaluate_document(&doc, &Config::strict_defaults());
    all.extend(evaluate_workspace_sorted(
        &[doc],
        &ctx,
        &Config::strict_defaults(),
    ));

    assert!(all.iter().any(|d| d.rule_id == "GF033"
        && d.span.start_line() == 2
        && d.message.contains("[DK-DL4006]")));
    assert!(all.iter().any(|d| d.rule_id == "GF080"
        && d.span.start_line() == 6
        && d.message.contains("[SH-SC3000]")));
    assert!(all.iter().any(|d| d.rule_id == "GF032"
        && d.span.start_line() == 7
        && d.message.contains("[DK-DL3022]")));
    assert!(all.iter().any(|d| d.rule_id == "GF033"
        && d.span.start_line() == 9
        && d.message.contains("[DK-DL3025]")));
    Ok(())
}

#[test]
fn dockerfile_inherits_stage_shell_and_pipefail_across_from_alias() -> Res {
    let mut ctx = ProjectContext::default();
    ctx.features.insert(FeatureId::Docker);
    ctx.dockerignore_entries = Some(BTreeSet::from([String::from(".git")]));
    let inherited_clean_df = "\
FROM debian:12.8-slim AS base
SHELL [\"/bin/bash\", \"-o\", \"pipefail\", \"-c\"]
RUN [[ -d /var ]] && printf 'a\\n' | cat

FROM base AS runtime
RUN [[ -d /tmp ]] && printf 'b\\n' | cat
SHELL [\"/bin/bash\", \"-c\"]
RUN set -o pipefail && printf 'c\\n' | cat
COPY --from=base /var/tmp /tmp
USER appuser
ENTRYPOINT [\"/bin/bash\", \"-c\", \"echo ready\"]
";
    let clean_doc = parse_source(&RepoPath::new("Dockerfile")?, inherited_clean_df)?;
    assert_eq!(
        evaluate_document(&clean_doc, &Config::strict_defaults()),
        Vec::new()
    );
    assert_eq!(
        evaluate_workspace_sorted(&[clean_doc], &ctx, &Config::strict_defaults()),
        Vec::new()
    );
    Ok(())
}
