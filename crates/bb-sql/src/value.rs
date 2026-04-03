//! SQL value representation.

use rusqlite::types::ToSqlOutput;

/* ──────────────────────────── Value type ──────────────────────────────── */

/// A dynamically-typed SQL value used during expression evaluation and export.
#[derive(Debug, Clone)]
pub enum SqlValue {
    Bool(bool),
    Int(i64),
    Str(String),
    Null,
}

impl SqlValue {
    /// Coerce to boolean (SQL truthiness).
    #[must_use]
    pub const fn as_bool(&self) -> bool {
        match self {
            Self::Bool(b) => *b,
            Self::Int(n) => *n != 0,
            Self::Str(s) => !s.is_empty(),
            Self::Null => false,
        }
    }

    /// Coerce to integer if possible.
    #[must_use]
    pub fn as_int(&self) -> Option<i64> {
        match self {
            Self::Int(n) => Some(*n),
            Self::Bool(b) => Some(i64::from(*b)),
            _ => None,
        }
    }

    /// Coerce to string reference if the value is a string.
    #[must_use]
    pub fn as_str(&self) -> Option<&str> {
        match self {
            Self::Str(s) => Some(s),
            _ => None,
        }
    }
}

/* ──────────────────────── rusqlite integration ───────────────────────── */

impl rusqlite::types::ToSql for SqlValue {
    fn to_sql(&self) -> rusqlite::Result<ToSqlOutput<'_>> {
        match self {
            Self::Bool(b) => Ok(ToSqlOutput::from(*b)),
            Self::Int(n) => Ok(ToSqlOutput::from(*n)),
            Self::Str(s) => Ok(ToSqlOutput::from(s.as_str())),
            Self::Null => Ok(ToSqlOutput::from(rusqlite::types::Null)),
        }
    }
}

/* ────────────────────────── serde_json conversion ────────────────────── */

impl From<&serde_json::Value> for SqlValue {
    fn from(v: &serde_json::Value) -> Self {
        match v {
            serde_json::Value::Bool(b) => Self::Bool(*b),
            serde_json::Value::Number(n) => {
                if let Some(i) = n.as_i64() {
                    Self::Int(i)
                } else if let Some(u) = n.as_u64() {
                    Self::Int(u as i64)
                } else {
                    Self::Null
                }
            }
            serde_json::Value::String(s) => Self::Str(s.clone()),
            serde_json::Value::Null => Self::Null,
            _ => Self::Null,
        }
    }
}
