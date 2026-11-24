//! Targeted boundary and operator-mutation tests for Shell, PowerShell, `Dockerfile` bridge,
//! Tier 2 config auditors, and Tier 3 JSON parsers.

use std::path::Path;

use gf_core::{
    Config, DockerCommandForm, DockerInstructionKind, DockerShellKind, Language, RepoPath,
};
use gf_parsers::{
    detect_language_or_shebang, detect_shebang_language, parse_hadolint_json,
    parse_psscriptanalyzer_json, parse_shellcheck_json, parse_source,
};
use gf_rules::evaluate_document;

type Res = Result<(), Box<dyn std::error::Error>>;

fn codes_of(rel: &str, src: &str) -> Result<Vec<String>, Box<dyn std::error::Error>> {
    let doc = parse_source(&RepoPath::new(rel)?, src)?;
    Ok(evaluate_document(&doc, &Config::strict_defaults())
        .into_iter()
        .map(|d| format!("{}:{}", d.rule_id, d.message))
        .collect())
}

#[test]
fn shell_shebang_directives_and_sourced_libs_metrics() -> Res {
    assert_eq!(
        detect_shebang_language("#!/usr/bin/env -S FOO=1 bash\n"),
        Some(Language::Shell)
    );
    assert_eq!(
        detect_shebang_language("#!/usr/bin/env pwsh\n"),
        Some(Language::PowerShell)
    );
    assert_eq!(detect_shebang_language("#!/usr/bin/python3\n"), None);
    assert_eq!(detect_shebang_language("#!/usr/bin/env -S\n"), None);
    assert_eq!(
        detect_language_or_shebang(&RepoPath::new("bin/bootstrap")?, "#!/bin/dash\nset -eu\n"),
        Language::Shell
    );

    let sourced_lib = "# shellcheck shell=bash\nhelper_fn() {\n    a=\"$1\"\n    b=\"$2\"\n    if [[ -n $a && -n $b ]]; then\n        . ./common.sh\n    fi\n}\n";
    let doc = parse_source(&RepoPath::new("scripts/helpers.lib.sh")?, sourced_lib)?;
    assert_eq!(doc.lang_rule_findings, Vec::new());
    assert_eq!(doc.callables.len(), 1);
    assert_eq!(doc.callables[0].param_count, 2);
    assert!(doc.callables[0].cyclomatic_complexity >= 3);
    assert_eq!(doc.imports.len(), 1);
    assert_eq!(doc.imports[0].module_source, "./common.sh");
    Ok(())
}

#[test]
fn shell_bashism_catalogue_and_guarded_pipefail_edges() -> Res {
    let bashisms = "\
#!/bin/sh
set -eu
(( count = 1 ))
for ((i=0; i<2; i++)); do echo \"$i\"; done
arr=(a b)
function bad_kw() { :; }
cat <<< \"hello\"
printf '%s' $'ansi'
echo \"${arr[0]}\" &> /tmp/out
echo -n \"${HOME//a/b}\" \"${HOME:0:2}\" \"${!HOME}\"
source ./other.sh
shopt -s nullglob
set -o pipefail
";
    let b_codes = codes_of("scripts/all_bashisms.sh", bashisms)?;
    let sc3000_count = b_codes.iter().filter(|m| m.contains("[SH-SC3000]")).count();
    assert!(
        sc3000_count >= 12,
        "expected >= 12 SC3000 findings, got {sc3000_count}: {b_codes:?}"
    );

    let guarded_pipefail =
        "#!/bin/sh\nset -eu\nif set -o pipefail 2>/dev/null; then\n    printf 'ok\\n'\nfi\n";
    assert_eq!(
        codes_of("scripts/guarded.sh", guarded_pipefail)?,
        Vec::<String>::new()
    );
    Ok(())
}

#[test]
fn powershell_metrics_inline_params_comparisons_and_exemptions() -> Res {
    let ps_mod = "\
function Invoke-Work([string]$Name, [bool]$DryRun, [switch]$Force) {
    if ($Name -eq 'a' -and $DryRun) {
        switch ($Name) {
            'a' { return 1 }
            default { return 2 }
        }
    } elseif ($Force) {
        return 3
    }
    return 0
}
";
    let doc = parse_source(&RepoPath::new("scripts/Tools.psm1")?, ps_mod)?;
    assert_eq!(doc.lang_rule_findings, Vec::new());
    assert_eq!(doc.callables.len(), 1);
    assert_eq!(doc.callables[0].param_count, 3);
    assert!(doc.callables[0].cognitive_complexity >= 4);
    assert_eq!(doc.kiss_yagni_findings.len(), 1);
    assert_eq!(doc.kiss_yagni_findings[0].measured, 2);

    let arg_cmp = "Set-StrictMode -Version 3.0\n$script:ErrorActionPreference = \"Stop\"\nWrite-Output ($items -ine $null)\n& 'iex' 'Get-Date'\n";
    let c = codes_of("scripts/arg_cmp.ps1", arg_cmp)?;
    assert!(c.iter().any(|m| m.contains("[PS-NULL-LEFT]")), "got {c:?}");
    assert!(c.iter().any(|m| m.contains("[PS-NO-IEX]")), "got {c:?}");

    let pester = "if ($null -eq $actual -and 0 -ne $null -and 'ok' -ne $null) { }\n";
    assert_eq!(
        codes_of("tests/Unit.Tests.ps1", pester)?,
        Vec::<String>::new()
    );
    Ok(())
}

#[test]
fn dockerfile_heredoc_powershell_escape_and_exec_form_edges() -> Res {
    let ps_df = "\
# escape=`
FROM mcr.microsoft.com/powershell:7.4-alpine-3.20 AS psbase
SHELL [\"pwsh\", \"-Command\"]
RUN Write-Host 'step1'; `
    if ($arr -eq $null) { Invoke-Expression $x }
RUN [\"pwsh\", \"-NoProfile\", \"-Command\", \"Write-Host ok\"]
USER appuser
";
    let doc = parse_source(&RepoPath::new("Dockerfile")?, ps_df)?;
    let df = doc.docker.as_ref().ok_or("missing docker doc")?;
    assert_eq!(
        df.stages[0].effective_shell.kind,
        DockerShellKind::PowerShell
    );
    assert!(df.stages[0].effective_shell.pipefail_enabled);
    assert!(matches!(
        &df.instructions[3].kind,
        DockerInstructionKind::Run {
            form: DockerCommandForm::ExecForm(_),
            has_unguarded_pipe: false,
            ..
        }
    ));
    assert_eq!(doc.lang_rule_findings.len(), 2);
    assert!(
        doc.lang_rule_findings
            .iter()
            .all(|f| f.span.start_line() == 5)
    );

    let heredoc_df = "\
FROM debian:12.8-slim
RUN <<'EOF'
#!/bin/bash
set -o pipefail
curl -fsSL https://example.com | cat
rm $UNQUOTED_IN_HEREDOC
EOF
USER appuser
";
    let hdoc = parse_source(&RepoPath::new("Dockerfile")?, heredoc_df)?;
    assert_eq!(hdoc.lang_rule_findings.len(), 1);
    assert_eq!(hdoc.lang_rule_findings[0].code(), "SH-SC2086");
    assert_eq!(hdoc.lang_rule_findings[0].span.start_line(), 6);
    Ok(())
}

#[test]
fn tier2_flow_yaml_and_tier3_error_branches() -> Res {
    let flow_hl = "ignored: [DL3002, DL3007]\ntrustedRegistries:\n  - '*'\n";
    let hl_codes = codes_of("hadolint.yml", flow_hl)?;
    assert_eq!(hl_codes.len(), 3);

    let pragma_ok = "# hadolint ignore=DL3008 -- pinned via apt snapshot archive\nFROM debian:12.8-slim\nUSER app\n";
    assert_eq!(codes_of("Dockerfile", pragma_ok)?, Vec::<String>::new());
    let pragma_bad = "# hadolint ignore=DL3008\nFROM debian:12.8-slim\nUSER app\n";
    assert!(
        codes_of("Dockerfile", pragma_bad)?
            .iter()
            .any(|m| m.contains("GF070:"))
    );

    let root = Path::new("/repo");
    assert!(parse_shellcheck_json("{not-json", root).is_err());
    assert!(parse_hadolint_json("{not-array}", root).is_err());
    assert!(parse_psscriptanalyzer_json("{bad", root).is_err());

    let flat_sc = r#"[{"file":"a.sh","line":2,"column":3,"level":"error","code":"2155","message":"Declare separately"}]"#;
    let parsed = parse_shellcheck_json(flat_sc, root)?;
    assert_eq!(parsed.len(), 1);
    assert_eq!(parsed[0].native_code, "SC2155");

    let pssa_num = r#"{"RuleName":"PSAvoidUsingPlainTextForPassword","ScriptName":"s.ps1","Line":3,"Column":2,"Message":"Plain","Severity":2}"#;
    let p_res = parse_psscriptanalyzer_json(pssa_num, root)?;
    assert_eq!(p_res.len(), 1);
    assert_eq!(p_res[0].path.as_str(), "s.ps1");
    Ok(())
}
