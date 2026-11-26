//! Pure `#![no_std]` single-file 6-domain evaluators (`GF060`, `GF061`, `GF063`,
//! local `GF062`/`GF064`, and static UI `a11y` `GF066`).

use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;

use gf_core::{
    Config, ContenusIssueKind, Diagnostic, DomainKind, FormeFindingKind, Language, Severity, Span,
    StyleLeakKind, UastDocument, UiCheckKind, Validation, classify_path, is_forme_token_path,
    is_root_governance_doc,
};

use crate::checks::{SourceLoc, rule_error as domain_diag};
use crate::format_workspace_checks::emit_svg_hazards;

fn check_code_style_and_ui(doc: &UastDocument, domain: DomainKind, out: &mut Vec<Diagnostic>) {
    if doc.language == Language::Css && domain != DomainKind::Forme {
        out.push(domain_diag(
            "GF060",
            format!("stylesheet `{}` must live inside `Forme` domain (`styles/`, `tokens/`)", doc.path),
            "Move `.css`/`.scss` files into `styles/` or `tokens/` (`forme_globs`) instead of mixing styles into `Code`.",
            SourceLoc::new(&doc.path, Span::FIRST),
        ));
    }
    if domain != DomainKind::Code {
        return;
    }
    for leak in &doc.style_leaks {
        let loc = SourceLoc::new(&doc.path, leak.span);
        match leak.kind {
            StyleLeakKind::CssInJs
            | StyleLeakKind::InlineStyleAttr
            | StyleLeakKind::StyleElement
            | StyleLeakKind::RawColorLiteral
            | StyleLeakKind::RawCssUnit => out.push(domain_diag(
                "GF060",
                format!("`Forme` style leak in `Code`: {}", leak.detail),
                "Remove CSS-in-JS, inline `style=`, and raw color/unit literals from `Code`; reference semantic CSS classes or Design Tokens in `Forme`.",
                loc,
            )),
            StyleLeakKind::DataUri | StyleLeakKind::InlineSvg => out.push(domain_diag(
                "GF064",
                format!("`Asset` leak in `Code`: {}", leak.detail),
                "Extract inline `<svg>` and `data:` payloads into standalone files under `assets/` (`asset_globs`).",
                loc,
            )),
        }
    }
    for ui in &doc.ui_checks {
        if matches!(
            ui.kind,
            UiCheckKind::NonInteractiveClick
                | UiCheckKind::ImgMissingAlt
                | UiCheckKind::ButtonMissingType
                | UiCheckKind::ButtonMissingLabel
        ) {
            out.push(domain_diag(
                "GF066",
                format!("static a11y violation: {}", ui.detail),
                "Use semantic `<button type=\"button\">` for interactive controls, provide `alt` on `<img>`, and ensure icon buttons have `aria-label`.",
                SourceLoc::new(&doc.path, ui.span),
            ));
        }
    }
}

fn emit_forme_finding(
    finding: &gf_core::FormeFinding,
    is_token_file: bool,
    loc: SourceLoc<'_>,
) -> Option<Diagnostic> {
    match finding.kind {
        FormeFindingKind::RawColor
        | FormeFindingKind::RawPxUnit
        | FormeFindingKind::ImportantOverride
            if !is_token_file =>
        {
            Some(domain_diag(
                "GF061",
                format!("design-token violation in `Forme`: {}", finding.detail),
                "Define primitive colors and sizes exclusively inside `tokens/` (`forme_token_globs`) and consume `var(--token)` in stylesheets without `!important`.",
                loc,
            ))
        }
        FormeFindingKind::TranslatedContentText => Some(domain_diag(
            "GF061",
            format!("`Texte` leak in `Forme`: {}", finding.detail),
            "Never embed human-readable strings in CSS `content:`; render localized strings via `Texte` dictionaries.",
            loc,
        )),
        FormeFindingKind::DataUri => Some(domain_diag(
            "GF061",
            format!("`Asset` leak in `Forme`: {}", finding.detail),
            "Move embedded `url(data:...)` payloads to dedicated files in `assets/` (`asset_globs`).",
            loc,
        )),
        FormeFindingKind::CssSpec(v) if !(is_token_file && v.allowed_in_token_file()) => {
            Some(domain_diag(
                "GF061",
                format!("[{}] {}", v.code(), finding.detail),
                v.remediation(),
                loc,
            ))
        }
        _ => None,
    }
}

fn check_forme_tokens(
    doc: &UastDocument,
    domain: DomainKind,
    config: &Config,
    out: &mut Vec<Diagnostic>,
) {
    if domain != DomainKind::Forme && doc.language != Language::Astro {
        return;
    }
    let Some(forme) = &doc.forme else {
        return;
    };
    let is_token_file = is_forme_token_path(&doc.path, &config.domains);
    for finding in &forme.findings {
        if let Some(d) = emit_forme_finding(
            finding,
            is_token_file,
            SourceLoc::new(&doc.path, finding.span),
        ) {
            out.push(d);
        }
    }
}

fn emit_contenus_issue(iss: &gf_core::ContenusIssue, loc: SourceLoc<'_>) -> Diagnostic {
    match iss.kind {
        ContenusIssueKind::DataUri => domain_diag(
            "GF064",
            format!("`Asset` leak in `Contenus`: {}", iss.detail),
            "Store images under `assets/` and link them by relative path instead of `data:` URIs.",
            loc,
        ),
        ContenusIssueKind::MarkdownSpec(v) => domain_diag(
            "GF063",
            format!("[{}] {}", v.code(), iss.detail),
            v.remediation(),
            loc,
        ),
        _ => domain_diag(
            "GF063",
            format!("impure `Contenus` Markdown: {}", iss.detail),
            "Keep Markdown/MDX strictly editorial: no raw HTML outside code blocks, no hooks/exports/CSS imports, sequential heading levels, and non-empty image `alt`.",
            loc,
        ),
    }
}

fn check_contenus_and_local_texte(
    doc: &UastDocument,
    domain: DomainKind,
    out: &mut Vec<Diagnostic>,
) {
    if let Some(texte) = &doc.texte {
        for iss in &texte.forbidden_values {
            out.push(domain_diag(
                "GF062",
                format!("impure `Texte` key `{}`: {}", iss.key, iss.detail),
                "Keep `i18n` dictionary values pure plain text (zero HTML tags, `style=`, or `data:` URIs).",
                SourceLoc::new(&doc.path, iss.span),
            ));
        }
    }
    if doc.language == Language::Markdown
        && !matches!(domain, DomainKind::Contenus | DomainKind::Tests)
        && !is_root_governance_doc(&doc.path)
    {
        out.push(domain_diag(
            "GF063",
            format!("editorial Markdown `{}` placed outside `Contenus` domain", doc.path),
            "Move editorial `.md`/`.mdx` files into `content/`, `courses/`, or `docs/` (`contenus_globs`).",
            SourceLoc::new(&doc.path, Span::FIRST),
        ));
    }
    if let Some(contenus) = &doc.contenus {
        for iss in &contenus.issues {
            out.push(emit_contenus_issue(
                iss,
                SourceLoc::new(&doc.path, iss.span),
            ));
        }
    }
}

fn check_local_asset(
    doc: &UastDocument,
    domain: DomainKind,
    config: &Config,
    out: &mut Vec<Diagnostic>,
) {
    let Some(asset) = &doc.asset else {
        return;
    };
    if domain != DomainKind::Asset {
        out.push(domain_diag(
            "GF064",
            format!("static asset `{}` placed outside `Asset` domain", doc.path),
            "Move static media, icons, and fonts into `assets/`, `public/`, or `static/` (`asset_globs`).",
            SourceLoc::new(&doc.path, Span::FIRST),
        ));
    }
    if asset.byte_size > config.domains.max_asset_bytes {
        let kb = u32::try_from(asset.byte_size / 1024)
            .unwrap_or(u32::MAX)
            .max(1);
        let max_kb = u32::try_from(config.domains.max_asset_bytes / 1024).unwrap_or(u32::MAX);
        out.push(Diagnostic {
            rule_id: String::from("GF064"),
            severity: Severity::Error,
            message: format!(
                "asset `{}` is {} bytes (max {})",
                doc.path, asset.byte_size, config.domains.max_asset_bytes
            ),
            remediation: String::from(
                "Compress/optimize the asset or raise `domains.max_asset_bytes` explicitly.",
            ),
            measured: kb,
            threshold: max_kb,
            path: doc.path.clone(),
            span: Span::FIRST,
        });
    }
    emit_svg_hazards(&doc.path, &asset.hazards, out);
}

/// Evaluates all single-file 6-domain segregation rules (`GF060`–`GF064`, `GF066`).
pub fn check_single_file_domains(
    doc: &UastDocument,
    config: &Config,
) -> Validation<(), Diagnostic> {
    let domain = classify_path(&doc.path, &config.domains);
    let mut diags = Vec::new();
    check_code_style_and_ui(doc, domain, &mut diags);
    check_forme_tokens(doc, domain, config, &mut diags);
    check_contenus_and_local_texte(doc, domain, &mut diags);
    check_local_asset(doc, domain, config, &mut diags);
    Validation::<(), Diagnostic>::from_errors(diags)
}
