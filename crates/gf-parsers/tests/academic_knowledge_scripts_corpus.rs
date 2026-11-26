//! Symmetric corpus and property-based tests for Jupyter (`.ipynb`), Quarto (`_quarto.yml`,
//! `.qmd`), Obsidian (`OFM`, `.canvas`), and Scripts (PEP 723, `SH-CURL-PIPE-UNSAFE`).

use gf_core::{Config, ProjectContext, RepoPath, WorkspaceArchetype};
use gf_parsers::langs::notebook::ipynb_bridge::neutralize_ipython_source;
use gf_parsers::parse_source;
use gf_rules::{evaluate_document, evaluate_workspace_sorted};
use proptest::prelude::*;

type TestRes = Result<(), Box<dyn std::error::Error>>;

fn eval_single(
    rel: &str,
    src: &str,
    arch: WorkspaceArchetype,
) -> Result<Vec<gf_core::Diagnostic>, Box<dyn std::error::Error>> {
    let path = RepoPath::new(rel)?;
    let doc = parse_source(&path, src)?;
    let mut cfg = Config::strict_defaults();
    cfg.project.archetype = gf_core::ArchetypeSetting::Explicit(arch);
    Ok(evaluate_document(&doc, &cfg))
}

#[test]
fn jupyter_clean_notebook_with_magics_and_print_has_zero_violations() -> TestRes {
    let clean_nb = r##"{
  "cells": [
    {
      "cell_type": "markdown",
      "metadata": {},
      "source": ["# Lab 1\n", "\n", "Intro text.\n"]
    },
    {
      "cell_type": "code",
      "execution_count": 1,
      "metadata": {},
      "outputs": [],
      "source": ["%matplotlib inline\n", "x = 42\n", "print(x)\n"]
    },
    {
      "cell_type": "code",
      "execution_count": 2,
      "metadata": {},
      "outputs": [],
      "source": "def double(n: int) -> int:\n    return n * 2\nprint(double(x))\n"
    }
  ],
  "metadata": {},
  "nbformat": 4,
  "nbformat_minor": 5
}"##;
    let diags = eval_single("TP1/lab.ipynb", clean_nb, WorkspaceArchetype::Academic)?;
    assert_eq!(diags, Vec::new());
    Ok(())
}

fn build_bad_notebook_json(secret: &str) -> String {
    use std::fmt::Write as _;
    let mut long_body = String::new();
    for i in 1..=55 {
        let _ = write!(long_body, "x_{i} = {i}\\n");
    }
    format!(
        r#"{{"cells":[{{"cell_type":"code","execution_count":3,"metadata":{{}},"outputs":[{{"output_type":"error","ename":"ValueError","evalue":"boom","traceback":[]}},{{"output_type":"stream","text":["token={secret}\n"]}}],"source":["def f(a=[]):\n","    return a\n"]}},{{"cell_type":"code","execution_count":1,"metadata":{{}},"outputs":[],"source":["%%bash\n","rm $UNQUOTED\n"]}},{{"cell_type":"code","execution_count":2,"metadata":{{}},"outputs":[],"source":["%%sql\n","SELECT * FROM users;\n"]}},{{"cell_type":"code","execution_count":4,"metadata":{{}},"outputs":[],"source":"{long_body}"}}],"metadata":{{}},"nbformat":4,"nbformat_minor":5}}"#
    )
}

#[test]
fn jupyter_detects_order_errors_secrets_python_bash_sql_and_oversized() -> TestRes {
    let secret = ["ghp_", "ABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789"].concat();
    let bad_nb = build_bad_notebook_json(&secret);
    let diags = eval_single("TP1/bad.ipynb", &bad_nb, WorkspaceArchetype::Academic)?;
    let msgs: Vec<&str> = diags.iter().map(|d| d.message.as_str()).collect();
    assert!(msgs.iter().any(|m| m.contains("NB-OUT-OF-ORDER-EXECUTION")));
    assert!(
        msgs.iter()
            .any(|m| m.contains("NB-COMMITTED-ERROR-TRACEBACK"))
    );
    assert!(
        msgs.iter()
            .any(|m| m.contains("NB-SECRET-IN-OUTPUT") && !m.contains(&secret))
    );
    assert!(
        msgs.iter()
            .any(|m| m.contains("PY-B006") && m.contains("[cell #1]"))
    );
    assert!(
        msgs.iter()
            .any(|m| m.contains("SH-SC2086") && m.contains("[cell #2]"))
    );
    assert!(
        msgs.iter()
            .any(|m| m.contains("SQL-SELECT-STAR") && m.contains("[cell #3]"))
    );
    assert!(
        msgs.iter()
            .any(|m| m.contains("NB-OVERSIZED-CELL") && m.contains("[cell #4]"))
    );
    Ok(())
}

const BAD_QMD_FIXTURE: &str = "\
---
title: \"Test\"
---

::: {.callout-warning}
Unclosed div referencing @fig-missing and {{< include _missing.qmd >}}.

```{python}
#| bad: [unclosed
def bad(x=[]):
    return x
```

```{{python}}
# Passive display block should not fail
x = 1 +
```
";

#[test]
fn quarto_validates_divs_crossrefs_yaml_options_cells_and_config_refs() -> TestRes {
    let qmd_doc = parse_source(&RepoPath::new("chap1.qmd")?, BAD_QMD_FIXTURE)?;
    let cfg_doc = parse_source(
        &RepoPath::new("_quarto.yml")?,
        "book:\n  chapters:\n    - chap1.qmd\n    - missing.qmd\n",
    )?;
    let mut cfg = Config::strict_defaults();
    cfg.project.archetype = gf_core::ArchetypeSetting::Explicit(WorkspaceArchetype::Academic);
    let mut diags = evaluate_document(&qmd_doc, &cfg);
    let ctx = ProjectContext {
        archetype: WorkspaceArchetype::Academic,
        ..ProjectContext::default()
    };
    diags.extend(evaluate_workspace_sorted(&[qmd_doc, cfg_doc], &ctx, &cfg));
    let msgs: Vec<&str> = diags.iter().map(|d| d.message.as_str()).collect();
    for code in [
        "QUARTO-UNCLOSED-DIV",
        "QUARTO-BROKEN-CROSSREF",
        "QUARTO-INVALID-CELL-OPTION",
        "PY-B006",
        "QUARTO-BROKEN-SHORTCODE-TARGET",
        "QUARTO-MISSING-REFERENCED-FILE",
    ] {
        assert!(msgs.iter().any(|m| m.contains(code)), "missing {code}");
    }
    assert!(!diags.iter().any(|d| d.rule_id == "GF000"));
    Ok(())
}

#[test]
fn obsidian_wikilinks_comments_and_json_canvas_refs_are_verified() -> TestRes {
    let note_a = RepoPath::new("Alpha.md")?;
    let note_b = RepoPath::new("Beta.md")?;
    let canvas_p = RepoPath::new("board.canvas")?;
    let doc_a = parse_source(
        &note_a,
        "# Alpha\n\nLink [[Beta#Section|Alias]] and table [[Beta\\|B]] and %% [[Ignored]] %% and [[Ghost]].\n",
    )?;
    let doc_b = parse_source(&note_b, "## Section\n\nParagraph text. ^blk-1\n")?;
    let doc_c = parse_source(
        &canvas_p,
        r#"{"nodes":[{"id":"n1","type":"file","file":"missing.md","x":0,"y":0,"width":100,"height":100}],"edges":[{"id":"e1","fromNode":"n1","toNode":"n999"}]}"#,
    )?;

    let mut cfg = Config::strict_defaults();
    cfg.project.archetype = gf_core::ArchetypeSetting::Explicit(WorkspaceArchetype::Knowledge);
    let mut diags = evaluate_document(&doc_c, &cfg);
    let ctx = ProjectContext {
        archetype: WorkspaceArchetype::Knowledge,
        ..ProjectContext::default()
    };
    diags.extend(evaluate_workspace_sorted(
        &[doc_a, doc_b, doc_c],
        &ctx,
        &cfg,
    ));
    let obs_links: Vec<&str> = diags
        .iter()
        .filter(|d| d.message.contains("OBS-BROKEN-WIKILINK-OR-EMBED"))
        .map(|d| d.message.as_str())
        .collect();
    assert_eq!(obs_links.len(), 1);
    assert!(obs_links[0].contains("Ghost"));
    assert_eq!(
        diags
            .iter()
            .filter(|d| d.message.contains("OBS-CANVAS-BROKEN-REF"))
            .count(),
        2
    );
    Ok(())
}

proptest! {
    #[test]
    fn ipython_neutralizer_preserves_exact_line_count(
        lines in prop::collection::vec(
            prop_oneof![
                Just(String::from("%matplotlib inline")),
                Just(String::from("%%time")),
                Just(String::from("!echo hello")),
                Just(String::from("x = 1")),
                Just(String::from("obj?")),
            ],
            1..20
        )
    ) {
        let src = lines.join("\n");
        let (neutralized, _) = neutralize_ipython_source(&src);
        prop_assert_eq!(neutralized.lines().count(), lines.len());
    }
}
