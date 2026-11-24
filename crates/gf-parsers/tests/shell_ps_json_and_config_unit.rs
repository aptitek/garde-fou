//! Tier 2 configuration hardening (`.shellcheckrc`, `.hadolint.yaml`, `PSScriptAnalyzerSettings.psd1`)
//! and Tier 3 JSON parser unit tests (including PowerShell 5.1 `ConvertTo-Json` quirks).

use std::path::Path;

use gf_core::{Config, RepoPath};
use gf_parsers::{
    parse_hadolint_json, parse_psscriptanalyzer_json, parse_shellcheck_json, parse_source,
};
use gf_rules::evaluate_document;

type Res = Result<(), Box<dyn std::error::Error>>;

fn eval_local_lines(rel_path: &str, src: &str) -> Result<Vec<u32>, Box<dyn std::error::Error>> {
    let doc = parse_source(&RepoPath::new(rel_path)?, src)?;
    Ok(evaluate_document(&doc, &Config::strict_defaults())
        .into_iter()
        .map(|d| d.span.start_line())
        .collect())
}

#[test]
fn tier2_configs_and_tier3_json_parsers_handle_ps51_quirks() -> Res {
    assert_eq!(
        eval_local_lines(".shellcheckrc", "disable=all\ndisable=SC1000-SC9999\n")?,
        vec![1, 2]
    );
    assert_eq!(
        eval_local_lines(".shellcheckrc", "# clean\nshell=sh\n")?,
        Vec::<u32>::new()
    );

    assert_eq!(
        eval_local_lines(
            ".hadolint.yaml",
            "ignored:\n  - DL4006\n  - SC2086\ntrustedRegistries: [\"*\"]\n"
        )?,
        vec![2, 3, 4]
    );

    assert_eq!(
        eval_local_lines(
            "PSScriptAnalyzerSettings.psd1",
            "@{\n  <# ExcludeRules = @('*') in comment is ignored #>\n  ExcludeRules = @('PSAvoidUsingInvokeExpression')\n}\n",
        )?,
        vec![3]
    );

    let root = Path::new("/repo");
    assert_eq!(parse_psscriptanalyzer_json("", root)?, Vec::new());
    assert_eq!(parse_psscriptanalyzer_json("null", root)?, Vec::new());
    let single = r#"{"RuleName":"PSAvoidGlobalVars","ScriptPath":"/repo/scripts/a.ps1","Line":9,"Column":3,"Message":"Avoid global","Severity":"Warning"}"#;
    let one = parse_psscriptanalyzer_json(single, root)?;
    assert_eq!(one.len(), 1);
    assert_eq!(one[0].path.as_str(), "scripts/a.ps1");
    assert_eq!(one[0].line, 9);

    let many = format!(
        "[{single}, {{\"RuleName\":\"InfoRule\",\"ScriptPath\":\"scripts/a.ps1\",\"Line\":2,\"Column\":1,\"Message\":\"info\",\"Severity\":0}}]"
    );
    assert_eq!(parse_psscriptanalyzer_json(&many, root)?.len(), 1);

    let sc_json = r#"{"comments":[{"file":"/repo/scripts/x.sh","line":4,"column":8,"level":"warning","code":2086,"message":"Double quote"},{"file":"scripts/x.sh","line":5,"column":1,"level":"style","code":2148,"message":"style"}]}"#;
    let sc_res = parse_shellcheck_json(sc_json, root)?;
    assert_eq!(sc_res.len(), 1);
    assert_eq!(sc_res[0].native_code, "SC2086");

    let hl_json = r#"[{"file":"/repo/Dockerfile","line":12,"column":1,"level":"warning","code":"DL4006","message":"Set pipefail"}]"#;
    let hl_res = parse_hadolint_json(hl_json, root)?;
    assert_eq!(hl_res.len(), 1);
    assert_eq!(hl_res[0].native_code, "DL4006");
    Ok(())
}
