//! `sqlparser` statement stream parser handling `PostgreSQL` `DROP INDEX CONCURRENTLY`
//! and `REINDEX [CONCURRENTLY]` alongside standard AST statements.

use gf_core::{Span, SqlDialectKind, SyntaxErrorNode};
use sqlparser::ast::{Spanned, Statement};
use sqlparser::dialect::Dialect;
use sqlparser::keywords::Keyword;
use sqlparser::parser::Parser;
use sqlparser::tokenizer::{Token, TokenWithSpan, Tokenizer};

use super::dialect_detect::{
    extract_sqlparser_error_loc, make_sqlparser_dialect, span_from_sql_line, span_from_sql_range,
};
use super::token_chunks::{chunk_span, is_non_trivia, kw_of, split_statement_chunks};

/// Normalized statement produced from a `.sql` source file.
pub enum ParsedSqlStmt {
    /// Standard AST statement parsed by `sqlparser`.
    Ast {
        /// Parsed SQL AST statement.
        stmt: Box<Statement>,
        /// 1-based source line span of the statement.
        span: Span,
    },
    /// `PostgreSQL` `DROP INDEX CONCURRENTLY ...` (validated via `sqlparser` after stripping `CONCURRENTLY`).
    DropIndexConcurrently {
        /// 1-based source line span.
        span: Span,
    },
    /// `PostgreSQL` `REINDEX [...] [CONCURRENTLY] <target>`.
    Reindex {
        /// Whether `CONCURRENTLY` was specified.
        concurrently: bool,
        /// 1-based source line span.
        span: Span,
    },
}

fn strip_drop_index_concurrently(chunk: &[TokenWithSpan]) -> Option<Vec<TokenWithSpan>> {
    let non_ws: Vec<usize> = chunk
        .iter()
        .enumerate()
        .filter_map(|(i, t)| is_non_trivia(&t.token).then_some(i))
        .collect();
    let &[i0, i1, i2, ..] = non_ws.as_slice() else {
        return None;
    };
    if kw_of(&chunk[i0].token) == Some(Keyword::DROP)
        && kw_of(&chunk[i1].token) == Some(Keyword::INDEX)
        && kw_of(&chunk[i2].token) == Some(Keyword::CONCURRENTLY)
    {
        Some(
            chunk
                .iter()
                .enumerate()
                .filter(|&(idx, _)| idx != i2)
                .map(|(_, tok)| tok.clone())
                .collect(),
        )
    } else {
        None
    }
}

fn try_parse_pg_reindex(chunk: &[TokenWithSpan]) -> Option<Result<ParsedSqlStmt, SyntaxErrorNode>> {
    let words: Vec<&TokenWithSpan> = chunk.iter().filter(|t| is_non_trivia(&t.token)).collect();
    let first = *words.first()?;
    if !matches!(&first.token, Token::Word(w) if w.value.eq_ignore_ascii_case("REINDEX")) {
        return None;
    }
    let span = chunk_span(chunk);
    if words.len() < 2 {
        return Some(Err(SyntaxErrorNode {
            span,
            message: String::from("incomplete `REINDEX` statement: expected target kind and name"),
        }));
    }
    let concurrently = words
        .iter()
        .any(|t| kw_of(&t.token) == Some(Keyword::CONCURRENTLY));
    Some(Ok(ParsedSqlStmt::Reindex { concurrently, span }))
}

fn push_parser_error(err_msg: &str, fallback: Span, out: &mut Vec<SyntaxErrorNode>) {
    let (line, col) = extract_sqlparser_error_loc(err_msg);
    let span = if err_msg.contains("Line:") {
        span_from_sql_line(u64::from(line))
    } else {
        fallback
    };
    out.push(SyntaxErrorNode {
        span,
        message: format!("col {col}: {err_msg}"),
    });
}

struct StmtAcc {
    statements: Vec<ParsedSqlStmt>,
    errors: Vec<SyntaxErrorNode>,
}

fn try_parse_pg_special_chunk(
    chunk: &[TokenWithSpan],
    dialect: &dyn Dialect,
    fallback: Span,
    out: &mut StmtAcc,
) -> bool {
    if let Some(res) = try_parse_pg_reindex(chunk) {
        match res {
            Ok(reidx) => out.statements.push(reidx),
            Err(err) => out.errors.push(err),
        }
        return true;
    }
    let Some(stripped) = strip_drop_index_concurrently(chunk) else {
        return false;
    };
    match Parser::new(dialect)
        .with_tokens_with_locations(stripped)
        .parse_statements()
    {
        Ok(_) => out
            .statements
            .push(ParsedSqlStmt::DropIndexConcurrently { span: fallback }),
        Err(e) => push_parser_error(&e.to_string(), fallback, &mut out.errors),
    }
    true
}

fn parse_single_chunk(
    chunk: Vec<TokenWithSpan>,
    dialect_kind: SqlDialectKind,
    dialect: &dyn Dialect,
    out: &mut StmtAcc,
) {
    let fallback = chunk_span(&chunk);
    if dialect_kind == SqlDialectKind::Postgres
        && try_parse_pg_special_chunk(&chunk, dialect, fallback, out)
    {
        return;
    }
    match Parser::new(dialect)
        .with_tokens_with_locations(chunk)
        .parse_statements()
    {
        Ok(parsed) => {
            for stmt in parsed {
                let sp = stmt.span();
                out.statements.push(ParsedSqlStmt::Ast {
                    stmt: Box::new(stmt),
                    span: span_from_sql_range(sp.start.line, sp.end.line),
                });
            }
        }
        Err(err) => push_parser_error(&err.to_string(), fallback, &mut out.errors),
    }
}

/// Tokenizes and parses `source` under `dialect_kind` into sequential [`ParsedSqlStmt`]s and syntax errors.
#[must_use]
pub(super) fn parse_sql_statements(
    source: &str,
    dialect_kind: SqlDialectKind,
) -> (Vec<ParsedSqlStmt>, Vec<SyntaxErrorNode>) {
    let dialect = make_sqlparser_dialect(dialect_kind);
    let mut out = StmtAcc {
        statements: Vec::new(),
        errors: Vec::new(),
    };
    let tokens = match Tokenizer::new(dialect.as_ref(), source).tokenize_with_location() {
        Ok(toks) => toks,
        Err(err) => {
            push_parser_error(&err.to_string(), Span::FIRST, &mut out.errors);
            return (out.statements, out.errors);
        }
    };
    for chunk in split_statement_chunks(tokens) {
        parse_single_chunk(chunk, dialect_kind, dialect.as_ref(), &mut out);
    }
    (out.statements, out.errors)
}
