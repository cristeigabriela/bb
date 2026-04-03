#![allow(clippy::doc_markdown)] // "SQLite" is a proper noun, not code.
#![allow(clippy::cast_sign_loss, clippy::cast_possible_wrap)] // SQL values cross i64/u64 boundary.
//! SQL evaluation and SQLite export for Benowin Blanc.
//!
//! Provides a generic SQL `WHERE` clause evaluator that works with any row type
//! via a column resolver closure, plus SQLite export utilities.

mod eval;
mod export;
mod value;

pub use eval::Evaluator;
pub use export::export_json_to_sqlite;
pub use value::SqlValue;

/// Re-export the parsed expression type for callers that need to cache it.
pub use sqlparser::ast::Expr;

/// Parse a `WHERE` clause string into an AST expression.
///
/// Wraps the clause in `SELECT 1 WHERE ...` so `sqlparser` can handle it.
///
/// # Errors
///
/// Returns an error string if the SQL fails to parse.
pub fn parse_where(clause: &str) -> Result<Expr, String> {
    eval::parse_where(clause)
}
