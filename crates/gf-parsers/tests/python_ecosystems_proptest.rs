//! Property-based tests (`proptest`) for Django, `NiceGUI`, `PyTorch`/`Keras`, and
//! `LangChain`/`LangGraph` CST analyzers: UTF-8 anti-panic and exact `+k` line covariance.

use gf_core::RepoPath;
use gf_parsers::parse_source;
use proptest::prelude::*;

proptest! {
    #[test]
    fn python_ecosystem_analyzers_never_panic_on_arbitrary_utf8(raw in "\\PC{0,180}") {
        for p in ["src/models.py", "src/settings.py", "src/ui.py", "src/train.py", "src/agent.py"] {
            if let Ok(rp) = RepoPath::new(p)
                && let Ok(doc) = parse_source(&rp, &raw)
            {
                prop_assert!(doc.effective_lines <= doc.total_lines);
            }
        }
    }

    #[test]
    fn python_ecosystem_line_numbers_covary_exactly_with_leading_comments(k in 0_u32..10_u32) {
        let prefix = "# pad\n".repeat(usize::try_from(k).unwrap_or(0));
        let dj = format!("{prefix}from django.utils.html import format_html\nformat_html(f\"{{x}}\")\n");
        let ml = format!("{prefix}import torch\ntorch.load(\"m.pt\", weights_only=False)\n");
        let llm = format!("{prefix}from langchain_core.prompts import PromptTemplate\nload_prompt(\"a.yaml\")\n");
        if let (Ok(dp), Ok(mp), Ok(lp)) = (
            RepoPath::new("src/v.py"),
            RepoPath::new("src/m.py"),
            RepoPath::new("src/l.py"),
        ) && let (Ok(dd), Ok(md), Ok(ld)) = (
            parse_source(&dp, &dj),
            parse_source(&mp, &ml),
            parse_source(&lp, &llm),
        ) {
            prop_assert!(dd.lang_rule_findings.iter().any(|f| f.code() == "DJ-RAW-SQL-OR-UNSAFE-HTML" && f.span.start_line() == k + 2));
            prop_assert!(md.lang_rule_findings.iter().any(|f| f.code() == "ML-UNSAFE-MODEL-DESERIALIZATION" && f.span.start_line() == k + 2));
            prop_assert!(ld.lang_rule_findings.iter().any(|f| f.code() == "LLM-RCE-AND-CVE-VECTORS" && f.span.start_line() == k + 2));
        }
    }
}

#[test]
fn python_ecosystem_proptest_sanity() -> Result<(), Box<dyn std::error::Error>> {
    let doc = parse_source(
        &RepoPath::new("src/ok.py")?,
        "def f(x: int) -> int:\n    return x\n",
    )?;
    assert_eq!(doc.syntax_errors.len(), 0);
    Ok(())
}
