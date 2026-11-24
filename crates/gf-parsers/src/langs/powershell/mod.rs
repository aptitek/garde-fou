//! Pure `tree-sitter-powershell` CST extractor for `.ps1`/`.psm1` scripts and `Dockerfile RUN` layers.

mod metrics;
pub(super) mod psd1_config;
mod rules;
mod strict_header;

use gf_core::{LangRuleFinding, Language, RepoPath, UastDocument};

use crate::anti_cheat::populate_cst_fortress;
use crate::cst::{collect_syntax_errors, line_metrics, parse_with_language};
use crate::types::{LanguageExtractor, ParseError};
use metrics::collect_powershell_uast_metrics;
use rules::check_powershell_rules;
use strict_header::{check_ps_strict_header, requires_ps_strict_header};

/// Pure Safe-Rust extractor for `.ps1`, `.psm1`, and `.psd1` PowerShell files.
#[derive(Clone, Copy, Debug, Default)]
pub struct PowerShellExtractor;

impl LanguageExtractor for PowerShellExtractor {
    fn extract(&self, path: &RepoPath, source: &str) -> Result<UastDocument, ParseError> {
        let ts_lang = tree_sitter_powershell::LANGUAGE.into();
        let tree = parse_with_language(&ts_lang, source)?;
        let root = tree.root_node();

        let (total_lines, effective_lines) = line_metrics(source);
        let mut doc = UastDocument::new(
            path.clone(),
            Language::PowerShell,
            total_lines,
            effective_lines,
        )?;
        doc.syntax_errors = collect_syntax_errors(root);

        if requires_ps_strict_header(path) {
            check_ps_strict_header(root, source, &mut doc.lang_rule_findings);
        }

        let mut uast_children = Vec::new();
        collect_powershell_uast_metrics(root, source, &mut doc, &mut uast_children);
        check_powershell_rules(root, source, &mut doc.lang_rule_findings);

        populate_cst_fortress(root, source, &mut doc);
        doc.root.children = uast_children;
        Ok(doc)
    }
}

/// Analyzes an embedded `Dockerfile RUN` PowerShell snippet (without requiring file-level strict header).
///
/// # Errors
///
/// Returns [`ParseError`] if configuring `tree-sitter-powershell` fails.
pub(super) fn analyze_dockerfile_powershell_run(
    script: &str,
) -> Result<Vec<LangRuleFinding>, ParseError> {
    let ts_lang = tree_sitter_powershell::LANGUAGE.into();
    let tree = parse_with_language(&ts_lang, script)?;
    let root = tree.root_node();
    let mut findings = Vec::new();
    check_powershell_rules(root, script, &mut findings);
    Ok(findings)
}
