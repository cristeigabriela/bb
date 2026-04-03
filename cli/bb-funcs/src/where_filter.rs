//! SQL `WHERE`-clause filtering for functions.
//!
//! Uses [`bb_sql::Evaluator`] with a Function-specific column resolver.
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
use bb_sql::{Evaluator, SqlValue};

pub use bb_sql::parse_where;

/* ──────────────────────── Column resolution ────────────────────────────── */

fn resolve_column(name: &str, f: &Function) -> SqlValue {
    match name {
        "name" => SqlValue::Str(f.get_name().to_string()),
        "return_type" => SqlValue::Str(f.get_return_type_name().to_string()),
        "params" => SqlValue::Int(f.get_params().len() as i64),
        "stack_size" => SqlValue::Int(stack_param_bytes(f) as i64),
        "arch" => SqlValue::Str(format_arch(f.get_arch()).to_string()),
        "calling_convention" | "callconv" => {
            SqlValue::Str(format_callconv(f.get_calling_convention()).to_string())
        }
        "is_exported" | "exported" => SqlValue::Bool(f.is_dllimport()),
        "has_body" => SqlValue::Bool(f.has_body()),
        "header" => {
            let h = f
                .get_location()
                .and_then(|l| l.file.clone())
                .unwrap_or_default();
            SqlValue::Str(h)
        }
        _ => SqlValue::Null,
    }
}

fn stack_param_bytes(f: &Function) -> usize {
    f.get_params()
        .iter()
        .filter(|p| p.is_stack())
        .map(bb_clang::Param::size)
        .sum()
}

/* ──────────────────────── Public interface ────────────────────────────── */

/// Evaluate a `WHERE` expression against a function, returning `true` if it passes.
#[must_use]
pub fn eval_where(expr: &bb_sql::Expr, f: &Function) -> bool {
    let evaluator = Evaluator::new(resolve_column);
    evaluator.eval_where(expr, f)
}
