//! Python ecosystem CST analyzers (`GF080`): Django 5.2/6.0, `NiceGUI`, `PyTorch`/`Keras`/`Sklearn`,
//! and `LangChain`/`LangGraph`.

pub mod django;
pub mod llm_agents;
pub mod ml_dl;
pub mod nicegui;

use gf_core::{DomainKind, UastDocument};
use tree_sitter::Node;

/// Runs all four Python ecosystem CST analyzers on `root` without performing I/O.
pub fn extract_py_ecosystem_rules(
    root: Node<'_>,
    source: &str,
    domain: DomainKind,
    doc: &mut UastDocument,
) {
    let in_code = domain == DomainKind::Code;
    django::extract_django_rules(root, source, in_code, doc);
    nicegui::extract_nicegui_rules(root, source, &mut doc.lang_rule_findings);
    ml_dl::extract_ml_dl_rules(root, source, in_code, &mut doc.lang_rule_findings);
    llm_agents::extract_llm_agent_rules(root, source, in_code, &mut doc.lang_rule_findings);
}
