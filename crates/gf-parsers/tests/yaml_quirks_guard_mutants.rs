//! Mutation-killing boundary tests for YAML 1.2 stream events, anchors/merges, scalar quirks,
//! comment pragmas (`GF070`), and Tier 2 tool configuration files (`GF081`).

use std::fmt::Write;

use gf_core::{LangRuleViolation, RepoPath, UastDocument, YamlViolation};
use gf_parsers::langs::yaml::events_tree::parse_yaml_stream;
use gf_parsers::parse_source;

type TestResult = Result<(), Box<dyn std::error::Error>>;

fn parse_doc(rel_path: &str, src: &str) -> Result<UastDocument, Box<dyn std::error::Error>> {
    Ok(parse_source(&RepoPath::new(rel_path)?, src)?)
}

fn has_yaml(doc: &UastDocument, v: YamlViolation) -> bool {
    doc.lang_rule_findings
        .iter()
        .any(|f| f.violation == LangRuleViolation::Yaml(v))
}

#[test]
fn stream_events_anchor_id_and_block_ranges_boundaries() {
    let plain = parse_yaml_stream("alpha: 1\nbeta: \"line\"\n");
    assert_eq!(plain.docs.len(), 1);
    assert_eq!(plain.docs[0].anchors.len(), 0);
    assert_eq!(plain.docs[0].block_body_ranges.len(), 0);

    let empty_block = parse_yaml_stream("alpha: |\nbeta: 2\n");
    assert_eq!(empty_block.docs[0].block_body_ranges.len(), 0);

    let block = parse_yaml_stream("alpha: |\n  first\n  second\nbeta: 2\n");
    assert_eq!(block.docs[0].block_body_ranges, vec![(2, 3)]);

    let anchored = parse_yaml_stream("base: &b\n  k: 1\nuse: *b\n");
    assert_eq!(anchored.docs[0].anchors.len(), 1);
}

#[test]
fn anchor_depth_16_vs_17_and_node_budget_2048_vs_2049() -> TestResult {
    let build_chain = |depth: usize| -> Result<String, std::fmt::Error> {
        let mut s = String::from("a0: &n0 leaf\n");
        for i in 1..=depth {
            writeln!(s, "a{i}: &n{i} [*n{}]", i - 1)?;
        }
        Ok(s)
    };
    assert!(!has_yaml(
        &parse_doc("cfg/d16.yml", &build_chain(16)?)?,
        YamlViolation::AnchorBomb
    ));
    assert!(has_yaml(
        &parse_doc("cfg/d17.yml", &build_chain(17)?)?,
        YamlViolation::AnchorBomb
    ));

    let mut plain_large = String::from("items:\n");
    for i in 0..2100 {
        writeln!(plain_large, "  - item_{i}")?;
    }
    assert!(!has_yaml(
        &parse_doc("cfg/p.yml", &plain_large)?,
        YamlViolation::AnchorBomb
    ));

    let build_aliases = |count: usize| -> String {
        let mut s = String::from("seed: &s v\nrefs:\n");
        for _ in 0..count {
            s.push_str("  - *s\n");
        }
        s
    };
    assert!(!has_yaml(
        &parse_doc("cfg/n2048.yml", &build_aliases(2048))?,
        YamlViolation::AnchorBomb
    ));
    assert!(has_yaml(
        &parse_doc("cfg/n2049.yml", &build_aliases(2049))?,
        YamlViolation::AnchorBomb
    ));
    Ok(())
}

#[test]
fn duplicate_merge_key_and_alias_depth_dedup() -> TestResult {
    let d1 = parse_doc(
        "cfg/dm.yml",
        "a: &a { k1: 1 }\nb: &b { k2: 2 }\nobj:\n  <<: *a\n  <<: *b\n",
    )?;
    assert_eq!(d1.lang_rule_findings.len(), 1);
    assert!(has_yaml(&d1, YamlViolation::DuplicateKey));

    let d2 = parse_doc(
        "cfg/ad.yml",
        "bad: &bad\n  k: 1\n  k: 2\nu1: *bad\nu2: *bad\n",
    )?;
    assert_eq!(d2.lang_rule_findings.len(), 1);

    let d3 = parse_doc(
        "cfg/adm.yml",
        "a: &a { k1: 1 }\nb: &b { k2: 2 }\nm: &m\n  <<: *a\n  <<: *b\nu1: *m\nu2: *m\n",
    )?;
    assert_eq!(d3.lang_rule_findings.len(), 1);
    Ok(())
}

#[test]
fn sexagesimal_and_scalar_quirk_boundaries() -> TestResult {
    for src in ["v: 1:5\n", "v: 1:5:9\n", "v: 1:59:59\n", "v: 22:00\n"] {
        assert!(
            has_yaml(
                &parse_doc("cfg/s.yml", src)?,
                YamlViolation::SexagesimalValue
            ),
            "{src}"
        );
    }
    for src in [
        "v: 1:60\n",
        "v: 1:59:60\n",
        "v: 1:20:30:40\n",
        "v: :20\n",
        "v: abc:20\n",
        "v: 12a:20\n",
        "v: 1:5a\n",
        "? [a, b]\n: 1\n",
    ] {
        assert!(
            !has_yaml(
                &parse_doc("cfg/ok.yml", src)?,
                YamlViolation::SexagesimalValue
            ),
            "{src}"
        );
    }
    assert!(has_yaml(
        &parse_doc("cfg/plain.yml", "on: [push]\n")?,
        YamlViolation::TruthyAmbiguity
    ));
    let nested_on = parse_doc(
        ".github/workflows/ci.yml",
        "on: { push: {} }\npermissions: { contents: read }\nenv: { on: true }\n",
    )?;
    assert!(has_yaml(&nested_on, YamlViolation::TruthyAmbiguity));
    Ok(())
}

#[test]
fn comment_scanner_quotes_escapes_and_block_range_edges() -> TestResult {
    let quoted_ok = concat!(
        "a: \"val # yamllint disable\"\n",
        "b: \"escaped \\\" # yamllint disable\"\n",
        "c: 'single \" # yamllint disable'\n",
        "d: \"double ' # yamllint disable\"\n",
        "e: token# yamllint disable\n",
        "f: 'it''s' # normal comment\n",
    );
    assert_eq!(
        parse_doc("cfg/quotes.yml", quoted_ok)?
            .bypass_findings
            .len(),
        0
    );

    let after_quotes = concat!(
        "a: 'trail\\' # yamllint disable\n",
        "b: 'has \" inner' # yamllint disable\n",
        "c: \"has ' inner\" # yamllint disable\n",
    );
    assert_eq!(
        parse_doc("cfg/after_quotes.yml", after_quotes)?
            .bypass_findings
            .len(),
        3
    );

    let around =
        "# yamllint disable\nscript: |\n  echo ok\n  # yamllint disable\n# dclint disable\n";
    let d = parse_doc("cfg/around.yml", around)?;
    assert_eq!(d.bypass_findings.len(), 2);
    assert_eq!(d.bypass_findings[0].span.start_line(), 1);
    assert_eq!(d.bypass_findings[1].span.start_line(), 5);
    Ok(())
}

#[test]
fn pragma_justification_length_and_single_rule_boundaries() -> TestResult {
    let valid = concat!(
        "a: 1 # yamllint disable-line rule:line-length -- 0123456789\n",
        "b: 2 # dclint disable-line require-quotes-in-ports -- 0123456789\n",
        "c: 3 # actionlint ignore -- 0123456789\n",
        "d: 4 # zizmor: ignore[unpinned-uses] -- 0123456789\n",
    );
    assert_eq!(parse_doc("cfg/ok.yml", valid)?.bypass_findings.len(), 0);

    for (idx, bad) in [
        "a: 1 # yamllint disable-line rule:line-length -- 012345678\n",
        "a: 1 # yamllint disable-line rule:all -- 0123456789\n",
        "a: 1 # yamllint disable-line rule:a,b -- 0123456789\n",
        "a: 1 # yamllint disable-line rule: -- 0123456789\n",
        "a: 1 # yamllint disable-line line-length -- 0123456789\n",
        "a: 1 # dclint disable-line require-quotes -- 012345678\n",
        "a: 1 # dclint disable-line rule-a,rule-b -- 0123456789\n",
        "a: 1 # dclint disable-line all -- 0123456789\n",
        "a: 1 # dclint disable-line -- 0123456789\n",
        "a: 1 # dclint disable\n",
        "a: 1 # actionlint ignore -- 012345678\n",
        "a: 1 # zizmor: ignore[unpinned-uses] -- 012345678\n",
        "a: 1 # zizmor: ignore[a,b] -- 0123456789\n",
        "a: 1 # zizmor: ignore[] -- 0123456789\n",
        "a: 1 # zizmor: ignore\n",
    ]
    .iter()
    .enumerate()
    {
        assert_eq!(
            parse_doc("cfg/bad.yml", bad)?.bypass_findings.len(),
            1,
            "case {idx}"
        );
    }
    Ok(())
}
