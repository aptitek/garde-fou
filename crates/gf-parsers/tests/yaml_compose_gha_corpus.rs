//! Symmetric positive/negative corpus tests (`YAML-*`, `COMPOSE-*`, `GHA-*`, `GF000`, `GF070`,
//! `GF081`, `GF082`) and embedded Shell/PowerShell AST bridges.

use std::path::Path;

use gf_core::{Config, RepoPath};
use gf_parsers::{
    parse_actionlint_json, parse_dclint_json, parse_source, parse_yamllint_parsable,
    parse_zizmor_sarif,
};
use gf_rules::evaluate_document;

type Res = Result<(), Box<dyn std::error::Error>>;

fn eval_codes(path: &str, src: &str) -> Result<Vec<String>, Box<dyn std::error::Error>> {
    let doc = parse_source(&RepoPath::new(path)?, src)?;
    Ok(evaluate_document(&doc, &Config::strict_defaults())
        .into_iter()
        .map(|d| format!("{}:{}:{}", d.rule_id, d.span.start_line(), d.message))
        .collect())
}

fn has_at(items: &[String], needle: &str, line: u32) -> bool {
    let tag = format!(":{line}:");
    items.iter().any(|m| m.contains(needle) && m.contains(&tag))
}

#[test]
fn yaml_1_2_quirks_merges_anchor_bombs_and_comment_bypasses_are_exact() -> Res {
    let bad_yaml = "dup: 1\ndup: 2\nflag_yes: yes\nflag_on: on\nmode: 0755\nratio: 1:20:30\na1: &b1\n  shared: 1\na2: &b2\n  shared: 2\nmerged:\n  <<: [*b1, *b2]\n# yamllint disable-file\n# zizmor: ignore[template-injection]\n";
    let codes = eval_codes("config/app.yaml", bad_yaml)?;
    for (needle, line) in [
        ("[YAML-DUPLICATE-KEY]", 2),
        ("[YAML-TRUTHY]", 3),
        ("[YAML-TRUTHY]", 4),
        ("[YAML-IMPLICIT-OCTAL]", 5),
        ("[YAML-SEXAGESIMAL]", 6),
        ("[YAML-DUPLICATE-KEY]", 12),
        ("GF070:13:", 13),
        ("GF070:14:", 14),
    ] {
        assert!(
            has_at(&codes, needle, line),
            "missing {needle}@{line} in {codes:?}"
        );
    }

    let bomb = "l0: &a0 [1, 2, 3, 4, 5, 6, 7, 8]\nl1: &a1 [*a0, *a0, *a0, *a0, *a0, *a0, *a0, *a0]\nl2: &a2 [*a1, *a1, *a1, *a1, *a1, *a1, *a1, *a1]\nl3: &a3 [*a2, *a2, *a2, *a2, *a2, *a2, *a2, *a2]\nl4: [*a3, *a3, *a3, *a3, *a3, *a3, *a3, *a3]\n";
    let bomb_codes = eval_codes("config/bomb.yaml", bomb)?;
    assert!(bomb_codes.iter().any(|m| m.contains("[YAML-ANCHOR-BOMB]")));
    assert!(
        eval_codes("broken.yaml", "a: [1, 2\n")?
            .iter()
            .any(|m| m.starts_with("GF000:2:"))
    );

    let clean_yaml = "# yamllint disable-line rule:line-length -- long canonical URL required\nbase: &defaults\n  retries: 3\n  enabled: true\nservice:\n  <<: *defaults\n  retries: 5\n  mode_str: \"0755\"\n  mode_oct: 0o755\n  zero: 0\n  port: \"22:22\"\n  answer: \"yes\"\n";
    assert_eq!(
        eval_codes("config/clean.yaml", clean_yaml)?,
        Vec::<String>::new()
    );
    Ok(())
}

#[test]
fn compose_security_topology_overrides_and_shell_bridge_are_exact() -> Res {
    let bad_compose = "x-common: &common\n  restart: always\nservices:\n  api:\n    <<: *common\n    image: nginx:latest\n    privileged: true\n    network_mode: host\n    user: \"0:0\"\n    cap_add: [CAP_SYS_ADMIN]\n    security_opt: [seccomp:unconfined]\n    ports:\n      - 22:22\n    volumes:\n      - /var/run/docker.sock:/var/run/docker.sock\n      - ghost_vol:/data\n    environment:\n      API_SECRET: super-secret-literal\n    depends_on:\n      db:\n        condition: service_healthy\n      missing_svc:\n        condition: service_started\n    command: [\"sh\", \"-c\", \"echo ok\\nrm $$UNQUOTED_COMPOSE\"]\n  db:\n    image: postgres:16.4\n    depends_on: [api]\nvolumes:\n  unused_vol: {}\n";
    let cmp = eval_codes("compose.yaml", bad_compose)?;
    for needle in [
        "[COMPOSE-UNPINNED-IMAGE]",
        "[COMPOSE-PRIVILEGED-OR-HOST]",
        "[COMPOSE-UNQUOTED-PORTS]",
        "[YAML-SEXAGESIMAL]",
        "[COMPOSE-DOCKER-SOCKET]",
        "[COMPOSE-ORPHAN-OR-UNDECLARED-RESOURCE]",
        "[COMPOSE-SECRET-IN-ENV]",
        "[COMPOSE-BROKEN-DEPENDS-ON]",
    ] {
        assert!(
            cmp.iter().any(|m| m.contains(needle)),
            "missing {needle} in {cmp:?}"
        );
    }
    assert!(
        has_at(&cmp, "[SH-SC2086]", 24),
        "expected remapped shell line 24 in {cmp:?}"
    );

    let clean_compose = "x-base: &base\n  restart: unless-stopped\nservices:\n  api:\n    <<: *base\n    image: ghcr.io/org/api:1.4.2\n    user: \"10001:10001\"\n    ports:\n      - \"8080:8080\"\n    volumes:\n      - ./config:/etc/app:ro\n      - app_data:/var/lib/app\n    networks: [default, backend]\n    environment:\n      - API_SECRET\n      - DB_PASSWORD=${DB_PASSWORD:?required}\n      - PUBLIC_KEY=ssh-ed25519-pub\n    depends_on:\n      db:\n        condition: service_healthy\n    healthcheck:\n      test: [\"CMD-SHELL\", \"printf '%s' \\\"$$HOSTNAME\\\"\"]\n  db:\n    image: postgres@sha256:abcdef0123456789\n    networks: [backend]\n    healthcheck:\n      test: [\"CMD\", \"pg_isready\"]\nvolumes:\n  app_data: {}\nnetworks:\n  backend: {}\n";
    assert_eq!(
        eval_codes("compose.yaml", clean_compose)?,
        Vec::<String>::new()
    );

    let clean_override =
        "services:\n  api:\n    environment:\n      LOG_LEVEL: debug\n    depends_on: [db]\n";
    assert_eq!(
        eval_codes("docker-compose.override.yml", clean_override)?,
        Vec::<String>::new()
    );
    Ok(())
}

#[test]
fn github_actions_injection_pwn_request_pinning_and_run_bridges_are_exact() -> Res {
    let bad_wf = "name: unsafe-ci\non:\n  pull_request_target:\njobs:\n  pwn:\n    runs-on: ubuntu-latest\n    steps:\n      - uses: actions/checkout@main\n        with:\n          ref: ${{ github.event.pull_request.head.sha }}\n      - uses: actions/github-script@v7\n        with:\n          script: |\n            console.log(\"${{ github['event']['issue']['title'] }}\")\n      - name: Bash step\n        run: |\n          echo \"${{ github.head_ref }}\"\n          rm $UNQUOTED_GHA_VAR\n  win:\n    runs-on: windows-latest\n    steps:\n      - run: |\n          if ($items -eq $null) { Invoke-Expression $cmd }\n";
    let gha = eval_codes(".github/workflows/unsafe.yml", bad_wf)?;
    assert!(
        !gha.iter().any(|m| m.contains("[YAML-TRUTHY]")),
        "root `on:` must be exempt: {gha:?}"
    );
    assert!(
        has_at(&gha, "[GHA-PERMISSIONS-AND-PINNING]", 1),
        "got {gha:?}"
    );
    assert!(
        has_at(&gha, "[GHA-PERMISSIONS-AND-PINNING]", 8),
        "got {gha:?}"
    );
    assert!(has_at(&gha, "[GHA-PWN-REQUEST]", 10), "got {gha:?}");
    assert!(has_at(&gha, "[GHA-TEMPLATE-INJECTION]", 14), "got {gha:?}");
    assert!(has_at(&gha, "[GHA-TEMPLATE-INJECTION]", 17), "got {gha:?}");
    assert!(has_at(&gha, "[SH-SC2086]", 18), "got {gha:?}");
    assert!(has_at(&gha, "[PS-NULL-LEFT]", 23), "got {gha:?}");
    assert!(has_at(&gha, "[PS-NO-IEX]", 23), "got {gha:?}");

    let clean_wf = "name: hardened-ci\non:\n  push:\n    branches: [main]\npermissions:\n  contents: read\njobs:\n  verify:\n    runs-on: ubuntu-latest\n    steps:\n      - uses: actions/checkout@11bd71901bbe5b1630ceea73d27597364c9af683\n      - uses: ./github-local-action\n      - name: Safe env binding and multiline GHA expr\n        env:\n          PR_TITLE: ${{ github.event.pull_request.title }}\n        run: |\n          printf '%s\\n' \"$PR_TITLE\" \"${{\n            matrix.os\n          }}\"\n          mkdir -p \"$RUNNER_TEMP/out\"\n";
    assert_eq!(
        eval_codes(".github/workflows/clean.yml", clean_wf)?,
        Vec::<String>::new()
    );
    Ok(())
}

#[test]
fn tier2_yaml_compose_and_zizmor_config_audits_are_exact() -> Res {
    assert_eq!(
        eval_codes(
            ".yamllint.yml",
            "extends: relaxed\nrules:\n  truthy: disable\n"
        )?
        .iter()
        .filter(|m| m.starts_with("GF081:"))
        .count(),
        2
    );
    assert_eq!(
        eval_codes(
            ".dclintrc.json",
            "// jsonc\n{\n  \"exclude\": [\"*\"],\n  \"rules\": { \"require-quotes-in-ports\": 0 }\n}\n"
        )?
        .iter()
        .filter(|m| m.starts_with("GF081:"))
        .count(),
        2
    );
    assert!(
        eval_codes(
            "zizmor.yml",
            "rules:\n  template-injection:\n    disable: true\n"
        )?
        .iter()
        .any(|m| m.starts_with("GF081:"))
    );
    Ok(())
}

#[test]
fn tier3_windows_safe_yamllint_dclint_actionlint_and_zizmor_parsers_are_exact() -> Res {
    let win_root = Path::new("C:\\repo");
    let yl = parse_yamllint_parsable(
        "C:\\repo\\compose.yaml:12:5: [error] duplication of key \"api\" (key-duplicates)\nC:\\repo\\a.yml:3:1: [note] ignored\n",
        win_root,
    )?;
    assert_eq!(yl.len(), 1);
    assert_eq!(
        (
            yl[0].path.as_str(),
            yl[0].line,
            yl[0].column,
            yl[0].native_code.as_str()
        ),
        ("compose.yaml", 12, 5, "key-duplicates")
    );

    let dc = parse_dclint_json(
        r#"[{"filePath":"/ws/compose.yaml","messages":[{"rule":"require-quotes-in-ports","message":"Quote ports","type":"error","line":9,"column":7}]}]"#,
        Path::new("/ws"),
    )?;
    assert_eq!(
        (dc.len(), dc[0].path.as_str(), dc[0].line),
        (1, "compose.yaml", 9)
    );

    let al = parse_actionlint_json(
        r#"[{"message":"shellcheck reported issue","filepath":".github/workflows/ci.yml","line":20,"column":9,"kind":"shellcheck"}]"#,
        Path::new("/ws"),
    )?;
    assert_eq!(
        (al.len(), al[0].native_code.as_str(), al[0].line),
        (1, "shellcheck", 20)
    );

    let zz = parse_zizmor_sarif(
        r#"{"runs":[{"results":[{"ruleId":"template-injection","level":"error","message":{"text":"code injection"},"locations":[{"physicalLocation":{"artifactLocation":{"uri":".github/workflows/ci.yml"},"region":{"startLine":14,"startColumn":11}}}]}]}]}"#,
        Path::new("/ws"),
    )?;
    assert_eq!(
        (zz.len(), zz[0].native_code.as_str(), zz[0].line),
        (1, "template-injection", 14)
    );
    Ok(())
}
