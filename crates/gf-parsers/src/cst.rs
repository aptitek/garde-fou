//! Shared safe `Tree-sitter` CST utilities (spans, slices, line metrics, tuple arity, callables).

mod arity;

use gf_core::{CallableDecl, CallableKind, Span, SyntaxErrorNode};
use tree_sitter::{Language as TsLanguage, Node, Parser, Tree};

use crate::complexity::{ComplexityRules, ComputedMetrics, compute_metrics};
use crate::fingerprint::{BodyFingerprint, fingerprint_body};
pub use crate::types::{ParseError, node_text};
pub use arity::{effective_param_arity, single_param_arity};

/// Inputs needed to assemble a normalized [`CallableDecl`] with complexity and Type-2 hash.
pub struct CallableSpec<'a> {
    /// Extracted identifier of the callable.
    pub name: String,
    /// Function, method, or closure category.
    pub kind: CallableKind,
    /// Full CST node defining the callable's source span.
    pub decl_node: Node<'a>,
    /// Optional body CST node analyzed for complexity and Type-2 fingerprinting.
    pub body_node: Option<Node<'a>>,
    /// Deployed parameter count (unpacking tuple-packed parameters, excluding `self` / `cls`).
    pub param_count: u32,
}

/// Assembles a [`CallableDecl`] with complexity metrics and Type-2/Type-3 structural fingerprints.
pub fn assemble_callable(
    spec: CallableSpec<'_>,
    source: &str,
    rules: &ComplexityRules,
) -> CallableDecl {
    let span = node_span(spec.decl_node);
    let (metrics, fp) = spec.body_node.map_or(
        (
            ComputedMetrics {
                cyclomatic: 1,
                cognitive: 0,
                max_nesting: 0,
            },
            BodyFingerprint {
                structural_hash: None,
                shingle_sketch: None,
                ast_node_count: 0,
            },
        ),
        |body| {
            (
                compute_metrics(body, source, rules),
                fingerprint_body(body, source),
            )
        },
    );

    CallableDecl {
        name: spec.name,
        kind: spec.kind,
        span,
        effective_lines: span_effective_lines(span, source),
        param_count: spec.param_count,
        cyclomatic_complexity: metrics.cyclomatic,
        cognitive_complexity: metrics.cognitive,
        max_nesting_depth: metrics.max_nesting,
        structural_hash: fp.structural_hash,
        shingle_sketch: fp.shingle_sketch,
        ast_node_count: fp.ast_node_count,
    }
}

/// Runs a `Tree-sitter` parser configured with `ts_lang` on `source`.
pub fn parse_with_language(ts_lang: &TsLanguage, source: &str) -> Result<Tree, ParseError> {
    let mut parser = Parser::new();
    parser
        .set_language(ts_lang)
        .map_err(|_| ParseError::LanguageSetup)?;
    parser.parse(source, None).ok_or(ParseError::EngineFailure)
}

/// Converts a 0-indexed `Tree-sitter` node range into a 1-indexed inclusive [`Span`].
pub fn node_span(node: Node<'_>) -> Span {
    let start_pos = node.start_position();
    let end_pos = node.end_position();
    let start_row = start_pos.row;
    let end_row = if end_pos.column == 0 && end_pos.row > start_row {
        end_pos.row.saturating_sub(1)
    } else {
        end_pos.row
    };

    let start_line = u32::try_from(start_row)
        .unwrap_or(u32::MAX)
        .saturating_add(1);
    let end_line = u32::try_from(end_row)
        .unwrap_or(u32::MAX)
        .saturating_add(1)
        .max(start_line);

    Span::new(start_line, end_line).unwrap_or(Span::FIRST)
}

/// Extracts the text of a named child field, or `fallback` when absent.
pub fn field_name_string(node: Node<'_>, field: &str, fallback: &str, source: &str) -> String {
    node.child_by_field_name(field).map_or_else(
        || String::from(fallback),
        |n| String::from(node_text(n, source)),
    )
}

/// Counts direct children of `parent` whose `kind()` is contained in `kinds`.
pub fn count_children_of_kind(parent: Option<Node<'_>>, kinds: &[&str]) -> u32 {
    let Some(node) = parent else {
        return 0;
    };
    let mut cursor = node.walk();
    let matched = node
        .children(&mut cursor)
        .filter(|c| kinds.contains(&c.kind()))
        .count();
    u32::try_from(matched).unwrap_or(u32::MAX)
}

/// Counts direct `named_children` of `parent` whose `kind()` is contained in `kinds`.
pub fn count_named_kinds(parent: Option<Node<'_>>, kinds: &[&str]) -> u32 {
    let Some(node) = parent else {
        return 0;
    };
    let mut cursor = node.walk();
    let matched = node
        .named_children(&mut cursor)
        .filter(|c| kinds.contains(&c.kind()))
        .count();
    u32::try_from(matched).unwrap_or(u32::MAX)
}

/// Computes `(total_lines, effective_non_blank_lines)` for `source`.
pub fn line_metrics(source: &str) -> (u32, u32) {
    let total = u32::try_from(source.lines().count()).unwrap_or(u32::MAX);
    let effective =
        u32::try_from(source.lines().filter(|l| !l.trim().is_empty()).count()).unwrap_or(u32::MAX);
    (total, effective)
}

fn span_effective_lines(span: Span, source: &str) -> u32 {
    let start_idx = usize::try_from(span.start_line().saturating_sub(1)).unwrap_or(0);
    let count = usize::try_from(span.line_count()).unwrap_or(0);
    let non_blank = source
        .lines()
        .skip(start_idx)
        .take(count)
        .filter(|line| !line.trim().is_empty())
        .count();
    u32::try_from(non_blank).unwrap_or(u32::MAX).max(1)
}

/// Collects all top-level `ERROR` and `MISSING` CST nodes when `root.has_error()` is true.
pub fn collect_syntax_errors(root: Node<'_>) -> Vec<SyntaxErrorNode> {
    if !root.has_error() {
        return Vec::new();
    }
    let mut errors = Vec::new();
    walk_errors(root, &mut errors);
    errors
}

fn walk_errors(node: Node<'_>, out: &mut Vec<SyntaxErrorNode>) {
    if node.is_error() {
        out.push(SyntaxErrorNode {
            message: String::from("malformed syntax node (`ERROR`)"),
            span: node_span(node),
        });
        return;
    }
    if node.is_missing() {
        out.push(SyntaxErrorNode {
            message: format!("missing expected token `{}`", node.kind()),
            span: node_span(node),
        });
        return;
    }
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        walk_errors(child, out);
    }
}

/// Parses `source` with `ts_lang` and initializes a [`UastDocument`] with line metrics,
/// syntax errors, and referenced symbols.
///
/// # Errors
///
/// Returns [`ParseError`] if Tree-sitter parsing or span initialization fails.
pub fn parse_and_init_document(
    ts_lang: &TsLanguage,
    path: &gf_core::RepoPath,
    lang: gf_core::Language,
    source: &str,
) -> Result<(Tree, gf_core::UastDocument), ParseError> {
    let tree = parse_with_language(ts_lang, source)?;
    let (total_lines, effective_lines) = line_metrics(source);
    let mut doc = gf_core::UastDocument::new(path.clone(), lang, total_lines, effective_lines)?;
    let root = tree.root_node();
    doc.syntax_errors = collect_syntax_errors(root);
    doc.referenced_symbols = crate::fingerprint::collect_referenced_symbols(root, source);
    Ok((tree, doc))
}

/// Builds a [`gf_core::TestCaseSummary`] by counting assertion `markers` present in `body`.
#[must_use]
pub fn make_test_case(
    name: String,
    span: Span,
    body: &str,
    markers: &[&str],
) -> gf_core::TestCaseSummary {
    let count = markers.iter().filter(|m| body.contains(**m)).count();
    gf_core::TestCaseSummary {
        name,
        span,
        assertion_count: u32::try_from(count).unwrap_or(u32::MAX),
        has_tautology: false,
        has_weak_only_assertion: false,
    }
}
