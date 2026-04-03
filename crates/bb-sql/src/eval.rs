//! Generic SQL `WHERE` clause evaluator.
//!
//! The [`Evaluator`] is parameterised over a row type `T` and a column resolver
//! function that maps column names to [`SqlValue`]s. This lets each CLI crate
//! define its own column schema while sharing all parsing and evaluation logic.

use sqlparser::ast::{BinaryOperator, Expr, UnaryOperator, Value};
use sqlparser::dialect::GenericDialect;
use sqlparser::parser::Parser;

use crate::value::SqlValue;

/* ────────────────────────────── Parsing ───────────────────────────────── */

/// Parse a `WHERE` clause string into an AST expression.
pub fn parse_where(clause: &str) -> Result<Expr, String> {
    let sql = format!("SELECT 1 WHERE {clause}");
    let dialect = GenericDialect {};
    let statements =
        Parser::parse_sql(&dialect, &sql).map_err(|e| format!("SQL parse error: {e}"))?;

    let Some(sqlparser::ast::Statement::Query(query)) = statements.into_iter().next() else {
        return Err("expected a query statement".into());
    };

    let Some(select) = query.body.as_select() else {
        return Err("expected a SELECT body".into());
    };

    select
        .selection
        .clone()
        .ok_or_else(|| "empty WHERE clause".into())
}

/* ────────────────────────────── Evaluator ─────────────────────────────── */

/// Column resolver function type: maps a lowercased column name and a row to a [`SqlValue`].
type Resolver<T> = Box<dyn Fn(&str, &T) -> SqlValue>;

/// A generic SQL `WHERE` evaluator bound to a column resolver.
pub struct Evaluator<T> {
    resolver: Resolver<T>,
}

impl<T> Evaluator<T> {
    /// Create a new evaluator with the given column resolver.
    ///
    /// The resolver maps a column name (already lowercased) and a row to a [`SqlValue`].
    pub fn new(resolver: impl Fn(&str, &T) -> SqlValue + 'static) -> Self {
        Self {
            resolver: Box::new(resolver),
        }
    }

    /// Evaluate a parsed `WHERE` expression against a row, returning `true` if it passes.
    #[must_use]
    pub fn eval_where(&self, expr: &Expr, row: &T) -> bool {
        match self.eval_expr(expr, row) {
            SqlValue::Bool(b) => b,
            _ => false,
        }
    }

    fn eval_expr(&self, expr: &Expr, row: &T) -> SqlValue {
        match expr {
            // Column reference.
            Expr::Identifier(ident) => self.resolve(&ident.value, row),

            // Compound identifier like table.column — just use the last part.
            Expr::CompoundIdentifier(parts) => {
                if let Some(last) = parts.last() {
                    self.resolve(&last.value, row)
                } else {
                    SqlValue::Null
                }
            }

            // Literals.
            Expr::Value(val) => match &val.value {
                Value::Number(n, _) => n.parse::<i64>().map_or(SqlValue::Null, SqlValue::Int),
                Value::SingleQuotedString(s) | Value::DoubleQuotedString(s) => {
                    SqlValue::Str(s.clone())
                }
                Value::Boolean(b) => SqlValue::Bool(*b),
                Value::Null => SqlValue::Null,
                _ => SqlValue::Null,
            },

            // Binary operators.
            Expr::BinaryOp { left, op, right } => {
                let lhs = self.eval_expr(left, row);
                let rhs = self.eval_expr(right, row);
                eval_binop(&lhs, op, &rhs)
            }

            // Unary NOT.
            Expr::UnaryOp {
                op: UnaryOperator::Not,
                expr: inner,
            } => SqlValue::Bool(!self.eval_expr(inner, row).as_bool()),

            // LIKE.
            Expr::Like {
                expr: inner,
                pattern,
                negated,
                ..
            } => {
                let val = self.eval_expr(inner, row);
                let pat = self.eval_expr(pattern, row);
                let matches = match (val.as_str(), pat.as_str()) {
                    (Some(v), Some(p)) => sql_like(v, p),
                    _ => false,
                };
                SqlValue::Bool(if *negated { !matches } else { matches })
            }

            // IN list.
            Expr::InList {
                expr: inner,
                list,
                negated,
            } => {
                let val = self.eval_expr(inner, row);
                let found = list.iter().any(|item| {
                    let item_val = self.eval_expr(item, row);
                    vals_eq(&val, &item_val)
                });
                SqlValue::Bool(if *negated { !found } else { found })
            }

            // BETWEEN.
            Expr::Between {
                expr: inner,
                low,
                high,
                negated,
            } => {
                let val = self.eval_expr(inner, row);
                let lo = self.eval_expr(low, row);
                let hi = self.eval_expr(high, row);
                let between = match (val.as_int(), lo.as_int(), hi.as_int()) {
                    (Some(v), Some(l), Some(h)) => v >= l && v <= h,
                    _ => false,
                };
                SqlValue::Bool(if *negated { !between } else { between })
            }

            // IS NULL / IS NOT NULL.
            Expr::IsNull(inner) => {
                SqlValue::Bool(matches!(self.eval_expr(inner, row), SqlValue::Null))
            }
            Expr::IsNotNull(inner) => {
                SqlValue::Bool(!matches!(self.eval_expr(inner, row), SqlValue::Null))
            }

            // Nested parens.
            Expr::Nested(inner) => self.eval_expr(inner, row),

            _ => SqlValue::Null,
        }
    }

    fn resolve(&self, name: &str, row: &T) -> SqlValue {
        (self.resolver)(&name.to_lowercase(), row)
    }
}

/* ──────────────────────── Shared helpers ──────────────────────────────── */

fn eval_binop(lhs: &SqlValue, op: &BinaryOperator, rhs: &SqlValue) -> SqlValue {
    match op {
        BinaryOperator::And => SqlValue::Bool(lhs.as_bool() && rhs.as_bool()),
        BinaryOperator::Or => SqlValue::Bool(lhs.as_bool() || rhs.as_bool()),

        BinaryOperator::Eq => SqlValue::Bool(vals_eq(lhs, rhs)),
        BinaryOperator::NotEq => SqlValue::Bool(!vals_eq(lhs, rhs)),

        BinaryOperator::Lt => {
            SqlValue::Bool(vals_cmp(lhs, rhs).is_some_and(std::cmp::Ordering::is_lt))
        }
        BinaryOperator::LtEq => SqlValue::Bool(vals_cmp(lhs, rhs).is_some_and(|c| !c.is_gt())),
        BinaryOperator::Gt => {
            SqlValue::Bool(vals_cmp(lhs, rhs).is_some_and(std::cmp::Ordering::is_gt))
        }
        BinaryOperator::GtEq => SqlValue::Bool(vals_cmp(lhs, rhs).is_some_and(|c| !c.is_lt())),

        _ => SqlValue::Null,
    }
}

fn vals_eq(a: &SqlValue, b: &SqlValue) -> bool {
    match (a, b) {
        (SqlValue::Int(x), SqlValue::Int(y)) => x == y,
        (SqlValue::Str(x), SqlValue::Str(y)) => x.eq_ignore_ascii_case(y),
        (SqlValue::Bool(x), SqlValue::Bool(y)) => x == y,
        _ => false,
    }
}

fn vals_cmp(a: &SqlValue, b: &SqlValue) -> Option<std::cmp::Ordering> {
    match (a.as_int(), b.as_int()) {
        (Some(x), Some(y)) => Some(x.cmp(&y)),
        _ => None,
    }
}

/// SQL LIKE pattern matching (`%` = any, `_` = single char).
fn sql_like(input: &str, pattern: &str) -> bool {
    let input = input.to_lowercase();
    let pattern = pattern.to_lowercase();
    like_match(input.as_bytes(), pattern.as_bytes())
}

fn like_match(input: &[u8], pattern: &[u8]) -> bool {
    if pattern.is_empty() {
        return input.is_empty();
    }
    match pattern[0] {
        b'%' => {
            for i in 0..=input.len() {
                if like_match(&input[i..], &pattern[1..]) {
                    return true;
                }
            }
            false
        }
        b'_' => !input.is_empty() && like_match(&input[1..], &pattern[1..]),
        c => {
            !input.is_empty()
                && input[0].eq_ignore_ascii_case(&c)
                && like_match(&input[1..], &pattern[1..])
        }
    }
}

/* ────────────────────────────── Tests ─────────────────────────────────── */

#[cfg(test)]
mod tests {
    use super::*;

    /// A simple row type for testing.
    struct Row {
        name: String,
        count: i64,
        active: bool,
    }

    fn test_resolver(col: &str, row: &Row) -> SqlValue {
        match col {
            "name" => SqlValue::Str(row.name.clone()),
            "count" => SqlValue::Int(row.count),
            "active" => SqlValue::Bool(row.active),
            _ => SqlValue::Null,
        }
    }

    fn eval(clause: &str, row: &Row) -> bool {
        let expr = parse_where(clause).expect("valid SQL");
        let evaluator = Evaluator::new(test_resolver);
        evaluator.eval_where(&expr, row)
    }

    fn row(name: &str, count: i64, active: bool) -> Row {
        Row {
            name: name.to_string(),
            count,
            active,
        }
    }

    #[test]
    fn eq_string() {
        assert!(eval("name = 'foo'", &row("foo", 0, false)));
        assert!(!eval("name = 'bar'", &row("foo", 0, false)));
    }

    #[test]
    fn eq_string_case_insensitive() {
        assert!(eval("name = 'FOO'", &row("foo", 0, false)));
    }

    #[test]
    fn eq_int() {
        assert!(eval("count = 5", &row("x", 5, false)));
        assert!(!eval("count = 5", &row("x", 6, false)));
    }

    #[test]
    fn gt_lt() {
        assert!(eval("count > 3", &row("x", 5, false)));
        assert!(!eval("count > 3", &row("x", 2, false)));
        assert!(eval("count < 10", &row("x", 5, false)));
        assert!(!eval("count < 10", &row("x", 15, false)));
    }

    #[test]
    fn between() {
        assert!(eval("count BETWEEN 3 AND 7", &row("x", 5, false)));
        assert!(!eval("count BETWEEN 3 AND 7", &row("x", 8, false)));
    }

    #[test]
    fn like_pattern() {
        assert!(eval("name LIKE 'fo%'", &row("foobar", 0, false)));
        assert!(!eval("name LIKE 'ba%'", &row("foobar", 0, false)));
        assert!(eval("name LIKE '%bar'", &row("foobar", 0, false)));
        assert!(eval("name LIKE 'f__bar'", &row("foobar", 0, false)));
    }

    #[test]
    fn in_list() {
        assert!(eval("name IN ('foo', 'bar', 'baz')", &row("bar", 0, false)));
        assert!(!eval("name IN ('foo', 'baz')", &row("bar", 0, false)));
    }

    #[test]
    fn and_or() {
        assert!(eval("count > 3 AND name = 'foo'", &row("foo", 5, false)));
        assert!(!eval("count > 3 AND name = 'bar'", &row("foo", 5, false)));
        assert!(eval("count > 100 OR name = 'foo'", &row("foo", 5, false)));
    }

    #[test]
    fn not() {
        assert!(eval("NOT count = 5", &row("x", 3, false)));
        assert!(!eval("NOT count = 5", &row("x", 5, false)));
    }

    #[test]
    fn is_null() {
        // "missing_col" resolves to SqlValue::Null
        assert!(eval("missing_col IS NULL", &row("x", 0, false)));
        assert!(!eval("name IS NULL", &row("x", 0, false)));
    }

    #[test]
    fn bool_column() {
        assert!(eval("active = true", &row("x", 0, true)));
        assert!(!eval("active = true", &row("x", 0, false)));
    }

    #[test]
    fn parse_invalid_sql() {
        assert!(parse_where("this is not sql !!!").is_err());
    }

    #[test]
    fn like_helper() {
        assert!(sql_like("CreateFileW", "%File%"));
        assert!(!sql_like("CloseHandle", "%File%"));
        assert!(sql_like("abc", "a_c"));
        assert!(!sql_like("abbc", "a_c"));
    }
}
