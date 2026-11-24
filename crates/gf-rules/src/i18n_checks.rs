//! Pure `#![no_std]` workspace `Texte` / `i18n` synchronization and hardcoded UI checks (`GF062`).

use alloc::collections::BTreeSet;
use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;

use gf_core::{
    Config, Diagnostic, DomainKind, FeatureId, ProjectContext, Span, TexteDocument, UastDocument,
    UiCheckKind, Validation, classify_path,
};

use crate::checks::{SourceLoc, rule_error as domain_diag};

#[derive(Clone, Copy)]
struct LocaleEntry<'a> {
    doc: &'a UastDocument,
    texte: &'a TexteDocument,
}

struct I18nSink<'a> {
    config: &'a Config,
    diags: &'a mut Vec<Diagnostic>,
}

struct UsedKeysReport<'a> {
    used_keys: BTreeSet<&'a str>,
    has_code_files: bool,
}

fn collect_locale_docs<'a>(docs: &'a [UastDocument], config: &Config) -> Vec<LocaleEntry<'a>> {
    docs.iter()
        .filter(|d| classify_path(&d.path, &config.domains) == DomainKind::Texte)
        .filter_map(|doc| doc.texte.as_ref().map(|texte| LocaleEntry { doc, texte }))
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
                    SourceLoc::new(&doc.path, ui.span),
                ));
            }
        }
    }
}

fn check_locale_symmetry(
    locale_docs: &[LocaleEntry<'_>],
    all_keys: &BTreeSet<&str>,
    out: &mut Vec<Diagnostic>,
) {
    if locale_docs.len() < 2 {
        return;
    }
    for entry in locale_docs {
        for &key in all_keys {
            if !entry.texte.keys.contains(key) {
                out.push(domain_diag(
                    "GF062",
                    format!(
                        "locale `{}` (`{}`) is missing translation key `{key}`",
                        entry.texte.locale, entry.doc.path
                    ),
                    "Keep translation key sets 100% symmetric across every locale dictionary in `Texte`.",
                    SourceLoc::new(&entry.doc.path, Span::FIRST),
                ));
            }
        }
    }
}

fn collect_and_verify_used_keys<'a>(
    docs: &'a [UastDocument],
    all_keys: &BTreeSet<&str>,
    sink: &mut I18nSink<'_>,
) -> UsedKeysReport<'a> {
    let mut used_keys = BTreeSet::new();
    let mut has_code_files = false;

    for doc in docs {
        let is_code = classify_path(&doc.path, &sink.config.domains) == DomainKind::Code;
        has_code_files |= is_code;
        for key in &doc.translation_keys_used {
            used_keys.insert(key.as_str());
            if is_code && !all_keys.contains(key.as_str()) {
                sink.diags.push(domain_diag(
                    "GF062",
                    format!(
                        "`t(\"{key}\")` in `{}` is missing from `Texte` dictionaries",
                        doc.path
                    ),
                    "Define every referenced translation key in all `locales/` dictionaries.",
                    SourceLoc::new(&doc.path, Span::FIRST),
                ));
            }
        }
    }
    UsedKeysReport {
        used_keys,
        has_code_files,
    }
}

fn check_code_and_dead_keys(
    docs: &[UastDocument],
    locale_docs: &[LocaleEntry<'_>],
    all_keys: &BTreeSet<&str>,
    sink: &mut I18nSink<'_>,
) {
    let report = collect_and_verify_used_keys(docs, all_keys, sink);
    if !report.has_code_files {
        return;
    }
    for &key in all_keys {
        if !report.used_keys.contains(key)
            && let Some(owner) = locale_docs.iter().find(|e| e.texte.keys.contains(key))
        {
            sink.diags.push(domain_diag(
                "GF062",
                format!(
                    "dead translation key `{key}` in `{}` is never used in `Code`",
                    owner.doc.path
                ),
                "Remove unused translation keys or wire them into UI components via `t(\"...\")`.",
                SourceLoc::new(&owner.doc.path, Span::FIRST),
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
        .flat_map(|e| e.texte.keys.iter().map(String::as_str))
        .collect();

    let mut diags = Vec::new();
    check_untranslated_jsx_strings(docs, config, &mut diags);
    check_locale_symmetry(&locale_docs, &all_keys, &mut diags);
    let mut sink = I18nSink {
        config,
        diags: &mut diags,
    };
    check_code_and_dead_keys(docs, &locale_docs, &all_keys, &mut sink);
    Validation::<(), Diagnostic>::from_errors(diags)
}
