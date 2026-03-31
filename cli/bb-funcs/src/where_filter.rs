//! SQL WHERE-clause filtering for functions.
//!
//! Parses a SQL WHERE expression via `sqlparser` and evaluates it against
//! [`Function`] fields. Simple CLI flags (--name, --return, etc.) are
//! converted into WHERE AST nodes and AND'd together.
//!
//! ## Supported columns
//!
//! | Column               | Type   | Example                  |
//! |----------------------|--------|--------------------------|
//! | `name`               | string | `'CreateFileW'`          |
//! | `return_type`        | string | `'HANDLE'`               |
//! | `params`             | int    | `7`                      |
//! | `stack_size`         | int    | `12`                     |
//! | `arch`               | string | `'x64'`                  |
//! | `calling_convention` | string | `'cdecl'`                |
//! | `is_exported`        | bool   | `true`                   |
//! | `has_body`           | bool   | `false`                  |
//! | `header`             | string | `'fileapi.h'`            |

use bb_clang::Function;
use bb_clang::display::{format_arch, format_callconv};
use sqlparser::ast::{BinaryOperator, Expr, UnaryOperator, Value};
use sqlparser::dialect::GenericDialect;
use sqlparser::parser::Parser;

/// Parse a WHERE clause string into an AST expression.
pub fn parse_where(clause: &str) -> Result<Expr, String> {
    // Wrap in a dummy SELECT to make sqlparser happy.
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

/// Evaluate a WHERE expression against a function, returning true if it passes.
#[must_use]
pub fn eval_where(expr: &Expr, f: &Function) -> bool {
    match eval_expr(expr, f) {
        Val::Bool(b) => b,
        _ => false,
    }
}

/* ────────────────────────── Value representation ───────────────────────── */

#[derive(Debug, Clone)]
enum Val {
    Bool(bool),
    Int(i64),
    Str(String),
    Null,
}

impl Val {
    fn as_bool(&self) -> bool {
        match self {
            Self::Bool(b) => *b,
            Self::Int(n) => *n != 0,
            Self::Str(s) => !s.is_empty(),
            Self::Null => false,
        }
    }

    fn as_int(&self) -> Option<i64> {
        match self {
            Self::Int(n) => Some(*n),
            Self::Bool(b) => Some(i64::from(*b)),
            _ => None,
        }
    }

    fn as_str(&self) -> Option<&str> {
        match self {
            Self::Str(s) => Some(s),
            _ => None,
        }
    }
}

/* ──────────────────────── Column resolution ────────────────────────────── */

fn resolve_column(name: &str, f: &Function) -> Val {
    match name.to_lowercase().as_str() {
        "name" => Val::Str(f.get_name().to_string()),
        "return_type" => Val::Str(f.get_return_type_name().to_string()),
        "params" => Val::Int(f.get_params().len() as i64),
        "stack_size" => Val::Int(stack_param_bytes(f) as i64),
        "arch" => Val::Str(format_arch(f.get_arch()).to_string()),
        "calling_convention" | "callconv" => {
            Val::Str(format_callconv(f.get_calling_convention()).to_string())
        }
        "is_exported" | "exported" => Val::Bool(f.is_dllimport()),
        "has_body" => Val::Bool(f.has_body()),
        "header" => {
            let h = f
                .get_location()
                .and_then(|l| l.file.clone())
                .unwrap_or_default();
            Val::Str(h)
        }
        _ => Val::Null,
    }
}

fn stack_param_bytes(f: &Function) -> usize {
    f.get_params()
        .iter()
        .filter(|p| p.is_stack())
        .map(|p| p.size())
        .sum()
}

/* ──────────────────────── Expression evaluator ─────────────────────────── */

fn eval_expr(expr: &Expr, f: &Function) -> Val {
    match expr {
        // Column reference.
        Expr::Identifier(ident) => resolve_column(&ident.value, f),

        // Compound identifier like table.column — just use the last part.
        Expr::CompoundIdentifier(parts) => {
            if let Some(last) = parts.last() {
                resolve_column(&last.value, f)
            } else {
                Val::Null
            }
        }

        // Literals.
        Expr::Value(val) => match &val.value {
            Value::Number(n, _) => n.parse::<i64>().map_or(Val::Null, Val::Int),
            Value::SingleQuotedString(s) | Value::DoubleQuotedString(s) => Val::Str(s.clone()),
            Value::Boolean(b) => Val::Bool(*b),
            Value::Null => Val::Null,
            _ => Val::Null,
        },

        // Binary operators.
        Expr::BinaryOp { left, op, right } => {
            let lhs = eval_expr(left, f);
            let rhs = eval_expr(right, f);
            eval_binop(&lhs, op, &rhs)
        }

        // Unary NOT.
        Expr::UnaryOp {
            op: UnaryOperator::Not,
            expr: inner,
        } => Val::Bool(!eval_expr(inner, f).as_bool()),

        // LIKE.
        Expr::Like {
            expr: inner,
            pattern,
            negated,
            ..
        } => {
            let val = eval_expr(inner, f);
            let pat = eval_expr(pattern, f);
            let matches = match (val.as_str(), pat.as_str()) {
                (Some(v), Some(p)) => sql_like(v, p),
                _ => false,
            };
            Val::Bool(if *negated { !matches } else { matches })
        }

        // IN list.
        Expr::InList {
            expr: inner,
            list,
            negated,
        } => {
            let val = eval_expr(inner, f);
            let found = list.iter().any(|item| {
                let item_val = eval_expr(item, f);
                vals_eq(&val, &item_val)
            });
            Val::Bool(if *negated { !found } else { found })
        }

        // BETWEEN.
        Expr::Between {
            expr: inner,
            low,
            high,
            negated,
        } => {
            let val = eval_expr(inner, f);
            let lo = eval_expr(low, f);
            let hi = eval_expr(high, f);
            let between = match (val.as_int(), lo.as_int(), hi.as_int()) {
                (Some(v), Some(l), Some(h)) => v >= l && v <= h,
                _ => false,
            };
            Val::Bool(if *negated { !between } else { between })
        }

        // IS NULL / IS NOT NULL.
        Expr::IsNull(inner) => Val::Bool(matches!(eval_expr(inner, f), Val::Null)),
        Expr::IsNotNull(inner) => Val::Bool(!matches!(eval_expr(inner, f), Val::Null)),

        // Nested parens.
        Expr::Nested(inner) => eval_expr(inner, f),

        _ => Val::Null,
    }
}

fn eval_binop(lhs: &Val, op: &BinaryOperator, rhs: &Val) -> Val {
    match op {
        BinaryOperator::And => Val::Bool(lhs.as_bool() && rhs.as_bool()),
        BinaryOperator::Or => Val::Bool(lhs.as_bool() || rhs.as_bool()),

        BinaryOperator::Eq => Val::Bool(vals_eq(lhs, rhs)),
        BinaryOperator::NotEq => Val::Bool(!vals_eq(lhs, rhs)),

        BinaryOperator::Lt => Val::Bool(vals_cmp(lhs, rhs).is_some_and(|c| c.is_lt())),
        BinaryOperator::LtEq => Val::Bool(vals_cmp(lhs, rhs).is_some_and(|c| !c.is_gt())),
        BinaryOperator::Gt => Val::Bool(vals_cmp(lhs, rhs).is_some_and(|c| c.is_gt())),
        BinaryOperator::GtEq => Val::Bool(vals_cmp(lhs, rhs).is_some_and(|c| !c.is_lt())),

        _ => Val::Null,
    }
}

fn vals_eq(a: &Val, b: &Val) -> bool {
    match (a, b) {
        (Val::Int(x), Val::Int(y)) => x == y,
        (Val::Str(x), Val::Str(y)) => x.eq_ignore_ascii_case(y),
        (Val::Bool(x), Val::Bool(y)) => x == y,
        _ => false,
    }
}

fn vals_cmp(a: &Val, b: &Val) -> Option<std::cmp::Ordering> {
    match (a.as_int(), b.as_int()) {
        (Some(x), Some(y)) => Some(x.cmp(&y)),
        _ => None,
    }
}

/// SQL LIKE pattern matching (% = any, _ = single char).
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
            // % matches zero or more characters.
            for i in 0..=input.len() {
                if like_match(&input[i..], &pattern[1..]) {
                    return true;
                }
            }
            false
        }
        b'_' => {
            // _ matches exactly one character.
            !input.is_empty() && like_match(&input[1..], &pattern[1..])
        }
        c => {
            !input.is_empty()
                && input[0].eq_ignore_ascii_case(&c)
                && like_match(&input[1..], &pattern[1..])
        }
    }
}
