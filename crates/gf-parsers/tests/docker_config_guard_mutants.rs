//! Targeted guard and boundary tests killing surviving mutants in `dockerfile*.rs`
//! and `shell_docker_config.rs`.

use gf_core::{Config, DockerShellKind, FeatureId, ProjectContext, RepoPath};
use gf_parsers::parse_source;
use gf_rules::{evaluate_document, evaluate_workspace_sorted};

type Res = Result<(), Box<dyn std::error::Error>>;

fn eval_df_all(src: &str) -> Result<Vec<String>, Box<dyn std::error::Error>> {
    let doc = parse_source(&RepoPath::new("Dockerfile")?, src)?;
    let mut ctx = ProjectContext::default();
    ctx.features.insert(FeatureId::Docker);
    ctx.dockerignore_entries = Some(std::iter::once(String::from(".git")).collect());
    let mut out: Vec<String> = evaluate_document(&doc, &Config::strict_defaults())
        .into_iter()
        .chain(evaluate_workspace_sorted(
            &[doc],
            &ctx,
            &Config::strict_defaults(),
        ))
        .map(|d| format!("{}:{}:{}", d.rule_id, d.span.start_line(), d.message))
        .collect();
    out.sort();
    out.dedup();
    Ok(out)
}

#[test]
fn dockerfile_delimiters_shells_image_refs_copy_add_and_keywords() -> Res {
    let heredocs = "\
FROM registry.example.io:5000/org/app:1.2.3 AS base
RUN <<MY_EOF-1
# sudo in heredoc comment must be ignored

rm -rf /var/lib/apt/lists/*
MY_EOF-1
RUN <<EOF;
echo ok
EOF
EXPOSE 8080
COPY --link --chown=1000:1000 app /app/
ADD --chown=1000:1000 archive.tar.gz /app/
USER appuser
";
    assert_eq!(eval_df_all(heredocs)?, Vec::<String>::new());

    let bad_df = "\
FROM ubuntu@
RUN <<EOF_APT
apt-get install -y curl
EOF_APT
ADD --chown=1000 only_one_operand
BOGUS_KEYWORD something
USER appuser
";
    let bad_msgs = eval_df_all(bad_df)?;
    assert!(bad_msgs.iter().any(|m| m.starts_with("GF030:")));
    assert!(bad_msgs.iter().any(|m| m.starts_with("GF033:")));
    assert_eq!(
        bad_msgs.iter().filter(|m| m.starts_with("GF000:")).count(),
        2
    );
    Ok(())
}

#[test]
fn dockerfile_from_tag_never_inherits_stage_alias_user() -> Res {
    let stage_non_inherit = "\
FROM alpine:3.20 AS base
USER appuser
FROM base:3.20
";
    let s1 = eval_df_all(stage_non_inherit)?;
    assert!(
        s1.iter().any(|m| m.starts_with("GF031:")),
        "FROM base:3.20 has a tag so it must not inherit USER from AS base: {s1:?}"
    );
    let stage_org_non_inherit = "\
FROM alpine:3.20 AS base
USER appuser
FROM myorg/base:1.0
";
    assert!(
        eval_df_all(stage_org_non_inherit)?
            .iter()
            .any(|m| m.starts_with("GF031:"))
    );
    Ok(())
}

#[test]
fn dockerfile_shell_argv_powershell_backslash_and_continued_comments() -> Res {
    let sh_df = "\
FROM alpine:3.20
SHELL [\"/bin/sh\", \"--pipefail\", \"foo\", \"pipefail\", \"-c\"]
RUN echo a \\
    # mid-run comment line 4
    && [[ -f /etc/os-release ]]
USER appuser
";
    let sh_doc = parse_source(&RepoPath::new("Dockerfile")?, sh_df)?;
    let df = sh_doc.docker.as_ref().ok_or("missing docker")?;
    assert_eq!(df.stages[0].effective_shell.kind, DockerShellKind::PosixSh);
    assert!(!df.stages[0].effective_shell.pipefail_enabled);
    assert_eq!(sh_doc.lang_rule_findings.len(), 1);
    assert_eq!(sh_doc.lang_rule_findings[0].code(), "SH-SC3000");
    assert_eq!(sh_doc.lang_rule_findings[0].span.start_line(), 5);

    let bash_no_pf = "\
FROM ubuntu:24.04
SHELL [\"/bin/bash\", \"-c\"]
RUN [[ -d /tmp ]] && curl -fsSL https://example.com | cat
USER appuser
";
    let b_msgs = eval_df_all(bash_no_pf)?;
    assert!(!b_msgs.iter().any(|m| m.contains("SH-SC3000")));
    assert!(b_msgs.iter().any(|m| m.contains("DK-DL4006")));

    let ps_backslash = "\
FROM mcr.microsoft.com/powershell:7.4-alpine-3.20
SHELL [\"pwsh\", \"-Command\"]
RUN if ($items \\
    -eq $null) { Write-Verbose 'bad' }
RUN   --mount=type=cache,target=/tmp
USER appuser
";
    let ps_doc = parse_source(&RepoPath::new("Dockerfile")?, ps_backslash)?;
    assert_eq!(ps_doc.lang_rule_findings.len(), 1);
    assert_eq!(ps_doc.lang_rule_findings[0].code(), "PS-NULL-LEFT");
    assert_eq!(ps_doc.lang_rule_findings[0].span.start_line(), 3);

    let other_shell = "\
FROM python:3.12-slim
SHELL [\"/usr/bin/python3\", \"-c\"]
RUN print('ok') | print('not shell')
USER appuser
";
    assert_eq!(eval_df_all(other_shell)?, Vec::<String>::new());
    Ok(())
}

#[test]
fn shellcheckrc_and_hadolint_yaml_quote_comment_and_section_boundaries() -> Res {
    let sc_clean = "\
disable=SC2086
disable=SC2000-SC2000
disable=SC2005-SC2000
disable=SC20ab-SC2099
";
    let sc_doc = parse_source(&RepoPath::new(".shellcheckrc")?, sc_clean)?;
    assert_eq!(sc_doc.tool_config_findings, Vec::new());

    let hl_quoted_hashes = "\
---
ignored: [\"DL3008'\"] # , DL3002]
trustedRegistries: ['safe.io\"'] # , *]
";
    let hl_doc = parse_source(&RepoPath::new(".hadolint.yaml")?, hl_quoted_hashes)?;
    assert_eq!(hl_doc.tool_config_findings, Vec::new());

    let hl_quoted_hashes2 = "\
ignored: ['DL3008\"'] # , DL3002]
";
    assert_eq!(
        parse_source(&RepoPath::new(".hadolint.yaml")?, hl_quoted_hashes2)?.tool_config_findings,
        Vec::new()
    );

    let hl_commented_out = "\
ignored: # 'unclosed_quote_in_comment

  - 'DL3002' # trailing comment after sq
  - \"DL3007\" # trailing comment after dq
  # - SC2086
trustedRegistries: '*' # wildcard scalar
";
    let hl_bad = parse_source(&RepoPath::new(".hadolint.yaml")?, hl_commented_out)?;
    assert_eq!(hl_bad.tool_config_findings.len(), 3);
    Ok(())
}
