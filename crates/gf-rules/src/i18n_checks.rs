//! Pure `#![no_std]` workspace `Texte` / `i18n` synchronization and hardcoded UI checks (`GF062`).

use alloc::collections::BTreeSet;
use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;

use gf_core::{
    Config, Diagnostic, DomainKind, FeatureId, ProjectContext, Span, TexteDocument, UastDocument,
    UiCheckKind, Validation, classify_path,
};

use crate::domain_checks::domain_diag;

fn collect_locale_docs<'a>(
    docs: &'a [UastDocument],
    config: &Config,
) -> Vec<(&'a UastDocument, &'a TexteDocument)> {
    docs.iter()
        .filter(|d| classify_path(&d.path, &config.domains) == DomainKind::Texte)
        .filter_map(|d| d.texte.as_ref().map(|t| (d, t)))
        .collect()
}

fn check_untranslated_jsx_strings(
    docs: &[UastDocument],
    config: &Config,
    out: &mut Vec<Diagnostic>,
) {
    for doc in docs {
        if classify_path(&doc.path, &config.domains) != DomainKind::Code {
            continue;
        }
        for ui in &doc.ui_checks {
            if ui.kind == UiCheckKind::UntranslatedText {
                out.push(domain_diag(
                    "GF062",
                    format!("untranslated UI string in `Code`: {}", ui.detail),
                    "Extract user-facing strings into `locales/*.json` (`Texte` domain) and render them via `t(\"...\")`.",
                    (&doc.path, ui.span),
                ));
            }
        }
    }
}

fn check_locale_symmetry(
    locale_docs: &[(&UastDocument, &TexteDocument)],
    all_keys: &BTreeSet<&str>,
    out: &mut Vec<Diagnostic>,
) {
    if locale_docs.len() < 2 {
        return;
    }
    for &(doc, texte) in locale_docs {
        for &key in all_keys {
            if !texte.keys.contains(key) {
                out.push(domain_diag(
                    "GF062",
                    format!(
                        "locale `{}` (`{}`) is missing translation key `{key}`",
                        texte.locale, doc.path
                    ),
                    "Keep translation key sets 100% symmetric across every locale dictionary in `Texte`.",
                    (&doc.path, Span::FIRST),
                ));
            }
        }
    }
}

fn collect_and_verify_used_keys<'a>(
    docs: &'a [UastDocument],
    all_keys: &BTreeSet<&str>,
    config: &Config,
    out: &mut Vec<Diagnostic>,
) -> (BTreeSet<&'a str>, bool) {
    let mut used_keys = BTreeSet::new();
    let mut has_code_files = false;

    for doc in docs {
        let is_code = classify_path(&doc.path, &config.domains) == DomainKind::Code;
        has_code_files |= is_code;
        for key in &doc.translation_keys_used {
            used_keys.insert(key.as_str());
            if is_code && !all_keys.contains(key.as_str()) {
                out.push(domain_diag(
                    "GF062",
                    format!(
                        "`t(\"{key}\")` in `{}` is missing from `Texte` dictionaries",
                        doc.path
                    ),
                    "Define every referenced translation key in all `locales/` dictionaries.",
                    (&doc.path, Span::FIRST),
                ));
            }
        }
    }
    (used_keys, has_code_files)
}

fn check_code_and_dead_keys(
    docs: &[UastDocument],
    locale_docs: &[(&UastDocument, &TexteDocument)],
    all_keys: &BTreeSet<&str>,
    sink: (&Config, &mut Vec<Diagnostic>),
) {
    let (config, out) = sink;
    let (used_keys, has_code_files) = collect_and_verify_used_keys(docs, all_keys, config, out);
    if !has_code_files {
        return;
    }
    for &key in all_keys {
        if !used_keys.contains(key)
            && let Some((owner_doc, _)) = locale_docs.iter().find(|(_, t)| t.keys.contains(key))
        {
            out.push(domain_diag(
                "GF062",
                format!(
                    "dead translation key `{key}` in `{}` is never used in `Code`",
                    owner_doc.path
                ),
                "Remove unused translation keys or wire them into UI components via `t(\"...\")`.",
                (&owner_doc.path, Span::FIRST),
            ));
        }
    }
}

/// Evaluates cross-file `Texte` / `i18n` symmetry, key coverage, and JSX string purity (`GF062`).
pub fn check_workspace_i18n(
    docs: &[UastDocument],
    ctx: &ProjectContext,
    config: &Config,
) -> Validation<(), Diagnostic> {
    let locale_docs = collect_locale_docs(docs, config);
    let i18n_active = ctx.features.contains(&FeatureId::I18n) || !locale_docs.is_empty();
    if !i18n_active {
        return Validation::Valid(());
    }

    let all_keys: BTreeSet<&str> = locale_docs
        .iter()
        .flat_map(|(_, t)| t.keys.iter().map(String::as_str))
        .collect();

    let mut diags = Vec::new();
    check_untranslated_jsx_strings(docs, config, &mut diags);
    check_locale_symmetry(&locale_docs, &all_keys, &mut diags);
    check_code_and_dead_keys(docs, &locale_docs, &all_keys, (config, &mut diags));
    Validation::<(), Diagnostic>::from_errors(diags)
}
