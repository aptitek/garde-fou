//! Pure Safe Rust Symfony 7.x and Doctrine ORM 3 / DBAL 4 rule analyzer (`SYMFONY-*`).

use gf_core::{LangRuleFinding, RepoPath, Span, SymfonyViolation, UastDocument};

fn push_sf(doc: &mut UastDocument, v: SymfonyViolation, ln: u32, msg: &str) {
    let sp = Span::new(ln.max(1), ln.max(1)).unwrap_or(Span::FIRST);
    doc.lang_rule_findings
        .push(LangRuleFinding::new(v, sp, msg));
}

fn is_dynamic_query_line(trimmed: &str) -> bool {
    let calls = [
        "createQuery(",
        "executeQuery(",
        "executeStatement(",
        "->where(",
        "->andWhere(",
    ];
    let has_call = calls.iter().any(|c| trimmed.contains(c));
    has_call && (trimmed.contains(" . $") || trimmed.contains(".$") || trimmed.contains("\"$"))
}

fn is_mutating_route(trimmed: &str) -> bool {
    trimmed.contains("#[Route(")
        && [
            "'POST'",
            "\"POST\"",
            "'PUT'",
            "\"PUT\"",
            "'DELETE'",
            "\"DELETE\"",
            "'PATCH'",
            "\"PATCH\"",
        ]
        .iter()
        .any(|m| trimmed.contains(m))
}

fn check_route_auth(lines: &[&str], idx: usize, ln: u32, doc: &mut UastDocument) {
    let window_start = idx.saturating_sub(3);
    let window_end = (idx + 8).min(lines.len());
    let protected = lines[window_start..window_end].iter().any(|l| {
        l.contains("#[IsGranted(")
            || l.contains("denyAccessUnlessGranted(")
            || l.contains("isGranted(")
    });
    if !protected {
        push_sf(
            doc,
            SymfonyViolation::MissingAccessControl,
            ln,
            "mutating Symfony `#[Route]` lacks `#[IsGranted]` or `denyAccessUnlessGranted()`",
        );
    }
}

fn check_legacy_annotation(trimmed: &str, ln: u32, doc: &mut UastDocument) {
    if trimmed.contains("@Route(") || trimmed.contains("@ORM\\") {
        push_sf(
            doc,
            SymfonyViolation::LegacyAnnotation,
            ln,
            "legacy `@Route`/`@ORM\\` docblock annotation; migrate to PHP 8 attributes",
        );
    }
}

fn check_line_patterns(lines: &[&str], idx: usize, ln: u32, doc: &mut UastDocument) {
    let trimmed = lines[idx].trim();
    if trimmed.contains("->container->get(") || trimmed.contains("$container->get(") {
        push_sf(
            doc,
            SymfonyViolation::ServiceLocatorAntiPattern,
            ln,
            "Symfony service locator `$container->get()`; inject service via constructor DI",
        );
    }
    if is_dynamic_query_line(trimmed) {
        push_sf(
            doc,
            SymfonyViolation::RawSqlOrDqlInjection,
            ln,
            "concatenated/interpolated variable in DQL/SQL query; use `setParameter()`",
        );
    }
    if trimmed.contains("->findAll()") {
        push_sf(
            doc,
            SymfonyViolation::UnboundedFindAll,
            ln,
            "unbounded Doctrine `->findAll()`; use paginated QueryBuilder",
        );
    }
    if is_mutating_route(trimmed) {
        check_route_auth(lines, idx, ln, doc);
    }
}

fn is_loop_header(trimmed: &str) -> bool {
    ["foreach ", "foreach(", "for ", "for(", "while ", "while("]
        .iter()
        .any(|p| trimmed.starts_with(p))
}

fn check_final_entities(lines: &[&str], doc: &mut UastDocument) {
    let mut saw_entity = false;
    for (idx, line) in lines.iter().enumerate() {
        let trimmed = line.trim();
        if trimmed.contains("#[ORM\\Entity") || trimmed.contains("#[Entity") {
            saw_entity = true;
        } else if saw_entity && trimmed.starts_with("final class ") {
            let ln = u32::try_from(idx + 1).unwrap_or(1);
            push_sf(
                doc,
                SymfonyViolation::FinalDoctrineEntity,
                ln,
                "`final class` with `#[ORM\\Entity]` breaks Doctrine lazy proxies",
            );
            saw_entity = false;
        }
    }
}

fn check_flush_in_loops(lines: &[&str], doc: &mut UastDocument) {
    let (mut loop_depth, mut brace_in_loop) = (0i32, 0i32);
    for (idx, line) in lines.iter().enumerate() {
        let trimmed = line.trim();
        if is_loop_header(trimmed) {
            loop_depth += 1;
        }
        if loop_depth == 0 {
            continue;
        }
        if trimmed.contains("->flush(") {
            let ln = u32::try_from(idx + 1).unwrap_or(1);
            push_sf(
                doc,
                SymfonyViolation::FlushInsideLoop,
                ln,
                "Doctrine `->flush()` inside loop causes N+1 database round-trips",
            );
        }
        brace_in_loop += i32::from(trimmed.contains('{')) - i32::from(trimmed.contains('}'));
        if brace_in_loop <= 0 && trimmed.contains('}') {
            loop_depth = loop_depth.saturating_sub(1);
            brace_in_loop = 0;
        }
    }
}

/// Runs Symfony 7 & Doctrine ORM 3 static checks on PHP source.
pub fn analyze_symfony_doctrine(path: &RepoPath, source: &str, doc: &mut UastDocument) {
    if path.file_name().starts_with("Version") && path.as_str().contains("migrations") {
        return;
    }
    let lines: Vec<&str> = source.lines().collect();
    for (idx, line) in lines.iter().enumerate() {
        let ln = u32::try_from(idx + 1).unwrap_or(1);
        let trimmed = line.trim();
        check_legacy_annotation(trimmed, ln, doc);
        if !trimmed.starts_with("//") && !trimmed.starts_with('*') {
            check_line_patterns(&lines, idx, ln, doc);
        }
    }
    check_final_entities(&lines, doc);
    check_flush_in_loops(&lines, doc);
}
