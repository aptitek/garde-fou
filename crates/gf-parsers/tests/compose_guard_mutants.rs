//! Mutation-killing boundary tests for Docker Compose (`COMPOSE-*`) and Shell AST bridges.

use gf_core::{ComposeViolation, LangRuleViolation, RepoPath, ShellViolation, UastDocument};
use gf_parsers::parse_source;

type TestResult = Result<(), Box<dyn std::error::Error>>;

fn parse_doc(rel_path: &str, src: &str) -> Result<UastDocument, Box<dyn std::error::Error>> {
    Ok(parse_source(&RepoPath::new(rel_path)?, src)?)
}

fn count_compose(doc: &UastDocument, v: ComposeViolation) -> usize {
    doc.lang_rule_findings
        .iter()
        .filter(|f| f.violation == LangRuleViolation::Compose(v))
        .count()
}

fn has_shell(doc: &UastDocument, v: ShellViolation) -> bool {
    doc.lang_rule_findings
        .iter()
        .any(|f| f.violation == LangRuleViolation::Shell(v))
}

#[test]
fn compose_privilege_user_caps_secopt_and_socket_variants() -> TestResult {
    for (idx, src) in [
        "services:\n  a: { image: a:1, user: \"0\" }\n",
        "services:\n  a: { image: a:1, user: \"0:0\" }\n",
        "services:\n  a: { image: a:1, user: \"root:wheel\" }\n",
        "services:\n  a: { image: a:1, cap_add: [SYS_ADMIN] }\n",
        "services:\n  a: { image: a:1, security_opt: [seccomp:unconfined] }\n",
    ]
    .iter()
    .enumerate()
    {
        let d = parse_doc("compose.yaml", src)?;
        assert_eq!(
            count_compose(&d, ComposeViolation::PrivilegedOrHostNamespace),
            1,
            "{idx}"
        );
    }
    let ok = parse_doc(
        "compose.yaml",
        "services:\n  a: { image: a:1, user: \"1000:1000\" }\n",
    )?;
    assert_eq!(
        count_compose(&ok, ComposeViolation::PrivilegedOrHostNamespace),
        0
    );

    for sock in [
        "services:\n  a: { image: a:1, volumes: [/run/docker.sock:/run/docker.sock] }\n",
        "services:\n  a: { image: a:1, volumes: [{ type: bind, source: /var/run/docker.sock, target: /s }] }\n",
    ] {
        let d = parse_doc("compose.yaml", sock)?;
        assert_eq!(count_compose(&d, ComposeViolation::DockerSocketMount), 1);
    }
    Ok(())
}

#[test]
fn compose_image_pinning_and_env_secret_forms() -> TestResult {
    for bad_suffix in [":-latest", "-latest", "}:latest"] {
        let img = if bad_suffix.starts_with('}') {
            format!("services:\n  a: {{ image: \"${{IMG{bad_suffix}\" }}\n")
        } else {
            format!("services:\n  a: {{ image: \"${{IMG{bad_suffix}}}\" }}\n")
        };
        let d = parse_doc("compose.yaml", &img)?;
        assert_eq!(
            count_compose(&d, ComposeViolation::UnpinnedOrMissingImage),
            1
        );
    }
    let empty_name = parse_doc("compose.yaml", "services:\n  a: { image: \":1.0\" }\n")?;
    assert_eq!(
        count_compose(&empty_name, ComposeViolation::UnpinnedOrMissingImage),
        1
    );
    let ok_img = format!(
        "services:\n  a: {{ image: \"ghcr.io/o/a:${{{}}}\" }}\n",
        "TAG"
    );
    assert_eq!(
        count_compose(
            &parse_doc("compose.yaml", &ok_img)?,
            ComposeViolation::UnpinnedOrMissingImage
        ),
        0
    );

    let env_seq = format!(
        "services:\n  a:\n    image: a:1\n    environment: [API_TOKEN=hard, SSH_KEY=hard, E_SECRET=, N_SECRET=null, T_SECRET=~, \"I_SECRET=${{{}}}\"]\n",
        "HOST"
    );
    let d_env = parse_doc("compose.yaml", &env_seq)?;
    assert_eq!(
        count_compose(&d_env, ComposeViolation::PlaintextSecretInEnv),
        2
    );
    Ok(())
}

#[test]
fn compose_volumes_networks_extensions_and_depends_on_dag() -> TestResult {
    let binds_ok = concat!(
        "X-common: &c { restart: always }\nservices:\n  a:\n    <<: *c\n    image: a:1\n",
        "    volumes: [./loc:/a, /abs:/b, ~/h:/c, $DIR:/d, \"C:\\\\w:/e\", { type: tmpfs, target: /t }, { type: volume, source: $DV, target: /v }]\n",
        "    networks: { default: {}, x-meta: {} }\nnetworks: { default: {} }\n",
    );
    let d_ok = parse_doc("compose.yaml", binds_ok)?;
    assert_eq!(
        count_compose(&d_ok, ComposeViolation::OrphanOrUndeclaredResource),
        0
    );

    let bad_refs = "services:\n  a: { image: a:1, volumes: [db:/d, { type: volume, source: cv, target: /c }], networks: { unet: {} } }\n";
    let d_bad = parse_doc("compose.yaml", bad_refs)?;
    assert_eq!(
        count_compose(&d_bad, ComposeViolation::OrphanOrUndeclaredResource),
        3
    );

    let dag_ok = concat!(
        "services:\n  leaf: { image: a:1, healthcheck: { test: [CMD, true] } }\n",
        "  m1: { image: a:1, depends_on: { leaf: { condition: service_healthy } } }\n",
        "  m2: { image: a:1, depends_on: [leaf] }\n",
        "  top: { image: a:1, depends_on: { m1: { condition: service_started }, m2: { condition: service_started } } }\n",
    );
    assert_eq!(
        count_compose(
            &parse_doc("compose.yaml", dag_ok)?,
            ComposeViolation::BrokenDependsOnGraph
        ),
        0
    );

    let dis_hc = "services:\n  db: { image: p:1, healthcheck: { test: [CMD, true], disable: true } }\n  api: { image: a:1, depends_on: { db: { condition: service_healthy } } }\n";
    assert_eq!(
        count_compose(
            &parse_doc("compose.yaml", dis_hc)?,
            ComposeViolation::BrokenDependsOnGraph
        ),
        1
    );

    let cyc = "services:\n  s1: { image: a:1, depends_on: [s2] }\n  s2: { image: a:1, depends_on: [s1] }\n";
    assert_eq!(
        count_compose(
            &parse_doc("compose.yaml", cyc)?,
            ComposeViolation::BrokenDependsOnGraph
        ),
        1
    );
    Ok(())
}

#[test]
fn compose_shell_bridges_bash_flags_and_healthcheck_forms() -> TestResult {
    let bash_cmd = concat!(
        "services:\n  a:\n    image: bash:5.2\n",
        "    command: [\"/bin/bash\", \"--norc\", \"-ec\", \"[[ -n x ]] && echo $$UNQUOTED\"]\n",
        "    healthcheck: { test: \"CMD-SHELL echo $$HC_UNQUOTED\" }\n",
    );
    let d = parse_doc("compose.yaml", bash_cmd)?;
    assert!(!has_shell(&d, ShellViolation::PosixBashism));
    let unq = d
        .lang_rule_findings
        .iter()
        .filter(|f| f.violation == LangRuleViolation::Shell(ShellViolation::UnquotedExpansion))
        .count();
    assert_eq!(unq, 2);

    let non_c_ep = parse_doc(
        "compose.yaml",
        "services:\n  a: { image: a:1, entrypoint: [\"/bin/sh\", \"--command\", \"echo $$IGNORED\"] }\n",
    )?;
    assert!(!has_shell(&non_c_ep, ShellViolation::UnquotedExpansion));

    let d2 = parse_doc(
        "compose.yaml",
        "services:\n  a: { image: a:1, healthcheck: { test: [CMD, /bin/sh, -c, \"echo $$HC2\"] } }\n",
    )?;
    assert!(has_shell(&d2, ShellViolation::UnquotedExpansion));
    Ok(())
}
