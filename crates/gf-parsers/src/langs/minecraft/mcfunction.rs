//! Minecraft 1.21+ Brigadier `.mcfunction` syntax, macro, selector, and folder analyzer (`MCFUNC-*`).

use gf_core::{ImportDecl, LangRuleFinding, McFunctionViolation, RepoPath, Span, UastDocument};

const UNSAFE_ADMIN_CMDS: &[&str] = &["stop", "op ", "deop ", "ban ", "ban-ip ", "pardon "];

fn push_mcfunc(doc: &mut UastDocument, v: McFunctionViolation, ln: u32, msg: &str) {
    let sp = Span::new(ln.max(1), ln.max(1)).unwrap_or(Span::FIRST);
    doc.lang_rule_findings
        .push(LangRuleFinding::new(v, sp, msg));
}

fn has_unbounded_at_e(trimmed: &str) -> bool {
    let Some(pos) = trimmed.find("@e") else {
        return false;
    };
    let rest = &trimmed[pos + 2..];
    if !rest.starts_with('[') {
        return rest
            .bytes()
            .next()
            .is_none_or(|b| b.is_ascii_whitespace() || b == b',');
    }
    let end = rest.find(']').unwrap_or(rest.len());
    let filter = &rest[1..end];
    !["type=", "tag=", "distance=", "limit="]
        .iter()
        .any(|k| filter.contains(k))
}

fn check_macro_line(trimmed: &str, ln: u32, doc: &mut UastDocument) {
    let is_macro = trimmed.starts_with('$');
    let has_param = trimmed.contains("$(");
    if is_macro && !has_param {
        push_mcfunc(
            doc,
            McFunctionViolation::InvalidMacroSyntax,
            ln,
            "`$` macro line must contain at least one `$(param)` substitution",
        );
    } else if !is_macro && has_param {
        push_mcfunc(
            doc,
            McFunctionViolation::InvalidMacroSyntax,
            ln,
            "`$(param)` macro substitution requires a leading `$` on the command line",
        );
    }
}

fn record_function_call_import(trimmed: &str, ln: u32, doc: &mut UastDocument) {
    let body = trimmed.strip_prefix('$').unwrap_or(trimmed);
    let Some(rest) = body.strip_prefix("function ") else {
        return;
    };
    let target = rest.split_whitespace().next().unwrap_or("");
    if target.contains(':') && !target.contains("$(") {
        let sp = Span::new(ln, ln).unwrap_or(Span::FIRST);
        doc.imports.push(ImportDecl {
            module_source: String::from(target),
            symbols: Vec::new(),
            is_type_only: false,
            span: sp,
        });
    }
}

fn check_mcfunc_line(trimmed: &str, ln: u32, doc: &mut UastDocument) {
    if trimmed.starts_with('/') {
        push_mcfunc(
            doc,
            McFunctionViolation::LeadingSlashCommand,
            ln,
            "remove leading `/` from `.mcfunction` command",
        );
    }
    check_macro_line(trimmed, ln, doc);
    if has_unbounded_at_e(trimmed) {
        push_mcfunc(
            doc,
            McFunctionViolation::UnboundedEntitySelector,
            ln,
            "unbounded `@e` selector without `type=`/`tag=`/`distance=`/`limit=` risks server TPS lag",
        );
    }
    let cmd = trimmed.trim_start_matches(['/', '$']);
    if UNSAFE_ADMIN_CMDS
        .iter()
        .any(|c| cmd == c.trim() || cmd.starts_with(c))
    {
        push_mcfunc(
            doc,
            McFunctionViolation::UnsafeAdminCommand,
            ln,
            "unsafe server-admin command (`stop`/`op`/`deop`/`ban`) inside datapack function",
        );
    }
    record_function_call_import(trimmed, ln, doc);
}

/// Analyzes a Minecraft 1.21+ `.mcfunction` file for Brigadier syntax, macros, lag selectors, and 1.21 paths.
pub fn analyze_mcfunction(path: &RepoPath, source: &str, doc: &mut UastDocument) {
    if path.as_str().contains("/functions/") {
        push_mcfunc(
            doc,
            McFunctionViolation::LegacyPluralFolder121,
            1,
            "legacy plural `/functions/` folder; Minecraft 1.21+ (`pack_format >= 48`) requires singular `/function/`",
        );
    }
    for (idx, line) in source.lines().enumerate() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        check_mcfunc_line(trimmed, u32::try_from(idx + 1).unwrap_or(1), doc);
    }
}
