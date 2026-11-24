//! Exhaustive guard and operator-boundary tests killing surviving mutants in `langs/shell/*`
//! and `langs/powershell/*`.

use gf_core::{Config, RepoPath, ShellDialect};
use gf_parsers::{langs::shell::dialect::detect_shell_dialect, parse_source};
use gf_rules::evaluate_document;

type Res = Result<(), Box<dyn std::error::Error>>;

fn codes(rel: &str, src: &str) -> Result<Vec<String>, Box<dyn std::error::Error>> {
    let doc = parse_source(&RepoPath::new(rel)?, src)?;
    Ok(evaluate_document(&doc, &Config::strict_defaults())
        .into_iter()
        .map(|d| format!("{}:{}", d.rule_id, d.message))
        .collect())
}

#[test]
fn shell_strict_mode_flags_and_named_options_kill_all_mutants() -> Res {
    assert_eq!(
        codes(
            "s/named.sh",
            "#!/bin/sh\nset -o errexit\nset -o nounset\nprintf 'ok'\n"
        )?,
        Vec::<String>::new()
    );
    assert_eq!(
        codes(
            "s/named.bash",
            "#!/bin/bash\nset -o errexit -o nounset -o pipefail\nprintf 'ok'\n"
        )?,
        Vec::<String>::new()
    );

    for bad_sh in [
        "#!/bin/sh\nset -e\nprintf 'missing u'\n",
        "#!/bin/sh\nset -u\nprintf 'missing e'\n",
        "#!/bin/sh\necho -eu\nprintf 'not set'\n",
    ] {
        let c = codes("s/bad.sh", bad_sh)?;
        assert!(
            c.iter().any(|m| m.contains("[SH-STRICT-MODE]")),
            "expected SH-STRICT-MODE for {bad_sh:?}, got {c:?}"
        );
    }

    for bad_bash in [
        "#!/bin/bash\nset -eu\nprintf 'no pipefail'\n",
        "#!/bin/bash\nset -eo pipefail\nprintf 'no u'\n",
        "#!/bin/bash\nset -uo pipefail\nprintf 'no e'\n",
    ] {
        let c = codes("s/bad.bash", bad_bash)?;
        assert!(
            c.iter().any(|m| m.contains("[SH-STRICT-MODE]")),
            "expected SH-STRICT-MODE for {bad_bash:?}, got {c:?}"
        );
    }
    Ok(())
}

#[test]
fn shell_dialect_and_unicode_brace_sanitizer_boundaries() -> Res {
    assert_eq!(
        detect_shell_dialect(&RepoPath::new("a.zsh")?, "echo 1\n"),
        ShellDialect::Bash
    );
    assert_eq!(
        detect_shell_dialect(
            &RepoPath::new("a.sh")?,
            "\n# shellcheck shell=bash\necho 1\n"
        ),
        ShellDialect::Bash
    );
    assert_eq!(
        detect_shell_dialect(&RepoPath::new("a.sh")?, "echo 1\n"),
        ShellDialect::PosixSh
    );
    for lib in ["s/a_lib.sh", "s/a.inc.sh", ".bashrc"] {
        assert_eq!(codes(lib, "printf 'lib'\n")?, Vec::<String>::new());
    }
    let latin1_with_brace = parse_source(
        &RepoPath::new("s/u.lib.sh")?,
        "# shellcheck shell=sh\n. ./ÿ.sh # {𧶷\n",
    )?;
    assert_eq!(latin1_with_brace.imports[0].module_source, "./ÿ.sh");
    let no_brace_uni = parse_source(
        &RepoPath::new("s/u2.lib.sh")?,
        "# shellcheck shell=sh\n. ./𧶷.sh\n",
    )?;
    assert_eq!(no_brace_uni.imports[0].module_source, "./𧶷.sh");
    Ok(())
}

#[test]
fn shell_clean_posix_and_single_bashism_boundaries() -> Res {
    let posix_clean = "#!/bin/sh\nset -eu\nx=\"${ARG:-/tmp/val}\"\necho $? ${#x}\nexport PLAIN_VAR=1\nrm -f \"$HOME/\"\nrm --force \"$HOME/\"\nrm -rf \"$HOME/safe_subdir\"\nrm -- -rf \"$HOME/\"\ncd /tmp\nif cd /var; then { printf '%s\\n' \"$x\"; }; fi\nwhile cd /tmp; do break; done\nmy_fn() { cd /tmp || return 1; }\necho ok | cat\n";
    let clean_doc = parse_source(&RepoPath::new("s/clean.sh")?, posix_clean)?;
    assert_eq!(clean_doc.lang_rule_findings, Vec::new());
    assert!(clean_doc.referenced_symbols.contains("rm"));
    assert!(!clean_doc.referenced_symbols.contains(""));
    let ba = codes(
        "s/arith.bash",
        "#!/bin/bash\nset -euo pipefail\nx=1\n(( x + $x ))\n{ echo $x; }\n",
    )?;
    assert_eq!(ba.iter().filter(|m| m.contains("[SH-SC2086]")).count(), 1);
    for single_bashism in [
        "#!/bin/sh\nset -eu\necho \"${HOME//a/b}\"\n",
        "#!/bin/sh\nset -eu\necho -n hi\n",
        "#!/bin/sh\nset -eu\nset -o pipefail\n",
        "#!/bin/sh\nset -eu\nif true; then set -o pipefail; fi\n",
    ] {
        assert!(
            codes("s/one.sh", single_bashism)?
                .iter()
                .any(|m| m.contains("[SH-SC3000]"))
        );
    }
    Ok(())
}

#[test]
fn shell_hazards_quoting_and_unguarded_cd_boundaries() -> Res {
    let imp_doc = parse_source(
        &RepoPath::new("s/imp.sh")?,
        "#!/bin/sh\nset -eu\n. \"\"\n. \"$DYN/x.sh\"\n",
    )?;
    assert_eq!(imp_doc.imports, Vec::new());
    let posix_traps = "#!/bin/sh\nset -eu\nfunction\ttab_fn() { cd /var; }\nlocal plain_local=1\nlocal f_def; inner() { echo \"$(date)\"; }\necho hi |& cat\nprintf '%s' \"${x^}\" \"${x,}\"\n[ -n $UNQUOTED_IN_BRACKET ]\n{ echo $UNQUOTED_IN_BRACE; }\nassigned=$(echo $UNQUOTED_IN_ASSIGN_SUB)\necho ${OUTER_U:-$INNER_U}\necho \"${OUTER_Q:-$(echo $INNER_SPLIT)}\"\nrm --recursive \"$EMPTY_TARGET/\"\n";
    let trap_codes = codes("s/traps.sh", posix_traps)?;
    for needle in [
        "`function` keyword",
        "`|&`",
        "[SH-SC2164]",
        "[SH-SC2115]",
        "UNQUOTED_IN_BRACKET",
        "UNQUOTED_IN_BRACE",
        "UNQUOTED_IN_ASSIGN_SUB",
        "INNER_SPLIT",
    ] {
        assert!(
            trap_codes.iter().any(|m| m.contains(needle)),
            "missing {needle}"
        );
    }
    assert!(!trap_codes.iter().any(|m| m.contains("[SH-SC2155]")));
    assert_eq!(
        trap_codes
            .iter()
            .filter(|m| m.contains("OUTER_U") || m.contains("INNER_U"))
            .count(),
        1
    );
    let unguarded_cds = "# shellcheck shell=sh\nif cd /ok_if; then :; fi\nwhile cd /ok_while; do break; done\nif true; then cd /tmp; fi\nwhile true; do cd /var; break; done\nuntil false; do cd /etc; break; done\nfalse || cd /opt\nfn_in_list() { cd /srv; } || true\n";
    assert_eq!(
        codes("s/cds.lib.sh", unguarded_cds)?
            .iter()
            .filter(|m| m.contains("[SH-SC2164]"))
            .count(),
        5
    );
    Ok(())
}

#[test]
fn powershell_header_boundaries() -> Res {
    assert_eq!(
        codes(
            "s/short_v.ps1",
            "Set-StrictMode -v Latest\n$ErrorActionPreference = 'Stop'\n"
        )?,
        Vec::<String>::new()
    );
    for bad_hdr in [
        "Set-StrictMode -Version 1.0\n$ErrorActionPreference = 'Stop'\nWrite-Verbose 'x'\n",
        "Set-StrictMode -Version Latest\n$OtherPref = 'Stop'\nWrite-Verbose 'x'\n",
        "Set-StrictMode -Version Latest\nWrite-Verbose 'missing eap'\n",
        "Write-Output Set-StrictMode -Version Latest\n$ErrorActionPreference = 'Stop'\n",
        "if ($true) { Set-StrictMode -Version Latest; $ErrorActionPreference = 'Stop' }\n",
    ] {
        assert!(
            codes("s/hdr.ps1", bad_hdr)?
                .iter()
                .any(|m| m.contains("[PS-STRICT-HEADER]"))
        );
    }
    assert_eq!(
        codes(
            "PSScriptAnalyzerSettings.psd1",
            "@{\n    ExcludeRules = @('PSAvoidUsingWriteHost')\n}\n"
        )?,
        Vec::<String>::new()
    );
    Ok(())
}

#[test]
fn powershell_metrics_and_rule_boundaries() -> Res {
    let ps_fn = "\
Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'
function Test-Flags {
    [Diagnostics.CodeAnalysis.SuppressMessageAttribute('PSAvoidUsingWriteHost', '', Justification='..........')]
    param(
        [System.Boolean]$HasSecret,
        [System.Management.Automation.SwitchParameter]$Flag,
        $Opt = $false
    )
    if (\"lit\" -eq $null -or -2.5 -ne $null) { Write-Verbose 'ok' }
    $c1 = 'a' + $Flag -eq $null
    $c2 = $Flag + 'a' -eq $null
    Write-Verbose \"$c1 $c2\"
    try { Write-Verbose 't' } catch { ; }
}
";
    let doc = parse_source(&RepoPath::new("s/flags.ps1")?, ps_fn)?;
    assert_eq!(doc.callables.len(), 1);
    assert_eq!(doc.callables[0].name, "Test-Flags");
    assert_eq!(doc.kiss_yagni_findings[0].measured, 3);
    assert_eq!(doc.bypass_findings.len(), 1);
    assert!(doc.referenced_symbols.contains("Write-Verbose"));
    assert!(!doc.referenced_symbols.contains(""));
    let f_codes: Vec<&str> = doc
        .lang_rule_findings
        .iter()
        .map(gf_core::LangRuleFinding::code)
        .collect();
    assert!(!f_codes.contains(&"PS-PLAIN-SECRET-PARAM"));
    assert_eq!(f_codes.iter().filter(|&&c| c == "PS-NULL-LEFT").count(), 2);
    assert_eq!(
        f_codes.iter().filter(|&&c| c == "PS-EMPTY-CATCH").count(),
        1
    );
    Ok(())
}
