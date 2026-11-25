//! Universal multi-dialect SQL AST semantic rules (`GF080`):
//! `SQL-NULL-COMPARISON`, `SQL-NOT-IN-SUBQUERY`, `SQL-UNBOUNDED-MUTATION`, and `SQL-SELECT-STAR`.

use core::ops::ControlFlow;

use gf_core::{LangRuleFinding, Span, SqlRole, SqlViolation};
use sqlparser::ast::{
    BinaryOperator, Expr, InTarget, Query, SelectItem, SetExpr, Spanned, Statement, UnaryOperator,
    Value, Visit, Visitor,
};

use super::dialect_detect::span_from_sql_range;

fn expr_span(expr: &Expr) -> Span {
    let sp = expr.span();
    span_from_sql_range(sp.start.line, sp.end.line)
}

fn unwrap_nested(mut expr: &Expr) -> &Expr {
    while let Expr::Nested(inner) = expr {
        expr = inner;
    }
    expr
}

fn is_null_literal(expr: &Expr) -> bool {
    matches!(unwrap_nested(expr), Expr::Value(v) if matches!(v.value, Value::Null))
}

fn literal_eq_key(expr: &Expr) -> Option<(&'static str, String)> {
    match &unwrap_nested(expr) {
        Expr::Value(v) => match &v.value {
            Value::Number(n, _) => Some(("num", n.clone())),
            Value::SingleQuotedString(s) | Value::DoubleQuotedString(s) => Some(("str", s.clone())),
            Value::Boolean(b) => Some(("bool", b.to_string())),
            _ => None,
        },
        _ => None,
    }
}

fn is_trivial_where_tautology(expr: &Expr) -> bool {
    match unwrap_nested(expr) {
        Expr::Value(v) => matches!(v.value, Value::Boolean(true)),
        Expr::UnaryOp {
            op: UnaryOperator::Not,
            expr: inner,
        } => {
            matches!(unwrap_nested(inner), Expr::Value(v) if matches!(v.value, Value::Boolean(false)))
        }
        Expr::BinaryOp {
            left,
            op: BinaryOperator::Eq,
            right,
        } => {
            if let (Some(lk), Some(rk)) = (literal_eq_key(left), literal_eq_key(right)) {
                lk == rk
            } else {
                false
            }
        }
        Expr::BinaryOp {
            left,
            op: BinaryOperator::Or,
            right,
        } => is_trivial_where_tautology(left) || is_trivial_where_tautology(right),
        Expr::BinaryOp {
            left,
            op: BinaryOperator::And,
            right,
        } => is_trivial_where_tautology(left) && is_trivial_where_tautology(right),
        _ => false,
    }
}

struct ExprSemanticVisitor<'a> {
    findings: &'a mut Vec<LangRuleFinding>,
}

impl Visitor for ExprSemanticVisitor<'_> {
    type Break = ();

    fn pre_visit_expr(&mut self, expr: &Expr) -> ControlFlow<Self::Break> {
        match expr {
            Expr::BinaryOp {
                left,
                op: op @ (BinaryOperator::Eq | BinaryOperator::NotEq),
                right,
            } if is_null_literal(left) || is_null_literal(right) => {
                self.findings.push(LangRuleFinding::new(
                    SqlViolation::NullComparison,
                    expr_span(expr),
                    format!(
                        "comparison `{op} NULL` always evaluates to `UNKNOWN` in SQL three-valued logic"
                    ),
                ));
            }
            Expr::In {
                target: InTarget::Subquery(_),
                negated: true,
                ..
            } => {
                self.findings.push(LangRuleFinding::new(
                    SqlViolation::NotInSubquery,
                    expr_span(expr),
                    "`NOT IN (SELECT ...)` returns zero rows whenever the subquery yields a single `NULL`",
                ));
            }
            _ => {}
        }
        ControlFlow::Continue(())
    }
}

fn check_unbounded_mutation(stmt: &Statement, stmt_span: Span, out: &mut Vec<LangRuleFinding>) {
    let (kind_label, selection) = match stmt {
        Statement::Update(u) => ("UPDATE", u.selection.as_ref()),
        Statement::Delete(d) => ("DELETE", d.selection.as_ref()),
        _ => return,
    };
    match selection {
        None => out.push(LangRuleFinding::new(
            SqlViolation::UnboundedMutation,
            stmt_span,
            format!("unbounded `{kind_label}` statement without a `WHERE` clause mutates every row"),
        )),
        Some(pred) if is_trivial_where_tautology(pred) => out.push(LangRuleFinding::new(
            SqlViolation::UnboundedMutation,
            expr_span(pred),
            format!("`{kind_label}` statement guarded by trivial tautology `WHERE {pred}` mutates every row"),
        )),
        Some(_) => {}
    }
}

fn check_set_expr_projection_star(
    body: &SetExpr,
    context_label: &str,
    out: &mut Vec<LangRuleFinding>,
) {
    match body {
        SetExpr::Select(sel) => {
            for item in &sel.projection {
                if matches!(
                    item,
                    SelectItem::Wildcard(_) | SelectItem::QualifiedWildcard(_, _)
                ) {
                    let sp = item.span();
                    out.push(LangRuleFinding::new(
                        SqlViolation::SelectStar,
                        span_from_sql_range(sp.start.line, sp.end.line),
                        format!(
                            "`SELECT {item}` wildcard projection in {context_label} couples callers to physical column order"
                        ),
                    ));
                }
            }
        }
        SetExpr::Query(sub) => check_query_final_projection_star(sub, context_label, out),
        SetExpr::SetOperation { left, right, .. } => {
            check_set_expr_projection_star(left, context_label, out);
            check_set_expr_projection_star(right, context_label, out);
        }
        _ => {}
    }
}

fn check_query_final_projection_star(
    query: &Query,
    context_label: &str,
    out: &mut Vec<LangRuleFinding>,
) {
    check_set_expr_projection_star(&query.body, context_label, out);
}

/// Evaluates universal SQL AST semantic rules on a single [`Statement`].
pub fn check_universal_sql_statement(
    stmt: &Statement,
    stmt_span: Span,
    role: &SqlRole,
    out: &mut Vec<LangRuleFinding>,
) {
    let mut visitor = ExprSemanticVisitor { findings: out };
    if stmt.visit(&mut visitor).is_break() {
        return;
    }

    check_unbounded_mutation(stmt, stmt_span, out);

    match stmt {
        Statement::CreateView(cv) => {
            let label = if cv.materialized {
                "`CREATE MATERIALIZED VIEW`"
            } else {
                "`CREATE VIEW`"
            };
            check_query_final_projection_star(&cv.query, label, out);
        }
        Statement::Query(q) if matches!(role, SqlRole::QueryOrSchema) => {
            check_query_final_projection_star(q, "application query", out);
        }
        _ => {}
    }
}
