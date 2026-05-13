//! IRQL constraint type + parsing + numeric comparison.
//!
//! The upstream sparse parser pre-normalizes free-form frontmatter strings
//! like `"<= DISPATCH_LEVEL"` or `"PASSIVE_LEVEL"` into the structured
//! [`IrqlConstraint`] shape. This module owns that struct, plus a
//! `parse_constraint` for filter-input strings (used by `bb-funcs --irql`)
//! and a [`IrqlConstraint::matches`] helper that resolves levels to numeric
//! values via a caller-supplied constant lookup and applies the constraint
//! operator.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

/// Recognized IRQL level identifiers.
///
/// `ANY` is a wildcard used by the upstream parser. `DEVICE_LEVEL` and
/// `DIRQL` are documentation placeholders — they don't have a single
/// concrete numeric value and won't resolve via [`IrqlConstraint::resolve_level`].
pub const KNOWN_LEVELS: &[&str] = &[
    "PASSIVE_LEVEL",
    "APC_LEVEL",
    "DISPATCH_LEVEL",
    "DPC_LEVEL",
    "DEVICE_LEVEL",
    "DIRQL",
    "HIGH_LEVEL",
    "IPI_LEVEL",
    "ANY",
];

/// Recognized comparison operators on IRQL constraints.
pub const KNOWN_OPS: &[&str] = &["<", "<=", "=", "==", ">=", ">"];

/// Normalized IRQL constraint.
///
/// `level` is one of [`KNOWN_LEVELS`]. `op` is one of [`KNOWN_OPS`] or
/// `None` when the documentation didn't provide an operator (exact match).
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
pub struct IrqlConstraint {
    pub level: String,
    #[serde(default)]
    pub op: Option<String>,
}

impl IrqlConstraint {
    /// Resolve `self.level` to its numeric `#define` value via the given
    /// constant lookup (typically built from a macro-preprocessed kernel-
    /// mode translation unit).
    ///
    /// Returns `None` for `ANY`, `DEVICE_LEVEL`, and `DIRQL` (which don't
    /// have a single concrete value), or when the constant isn't in the
    /// lookup table.
    #[must_use]
    pub fn resolve_level(&self, lookup: &HashMap<String, u64>) -> Option<u64> {
        match self.level.as_str() {
            "ANY" | "DEVICE_LEVEL" | "DIRQL" => None,
            other => lookup.get(other).copied(),
        }
    }

    /// Does this constraint match a filter applied at `filter.level` with
    /// `filter.op`? Symbolic-match cases short-circuit; otherwise both
    /// sides are resolved to numeric values via `lookup` and compared
    /// using `filter.op` (or `=` when `op` is `None`).
    ///
    /// Returns `None` when a side can't be resolved numerically and the
    /// comparison is therefore undefined (the caller decides whether to
    /// keep or drop the entry).
    #[must_use]
    pub fn matches(
        &self,
        filter: &IrqlConstraint,
        lookup: &HashMap<String, u64>,
    ) -> Option<bool> {
        // `ANY` filter matches everything.
        if filter.level == "ANY" {
            return Some(true);
        }
        // Exact symbolic equality covers same-level same-op cases (incl.
        // both ops being None).
        if self.level == filter.level && self.op == filter.op {
            return Some(true);
        }

        let a = self.resolve_level(lookup)?;
        let b = filter.resolve_level(lookup)?;
        let op = filter.op.as_deref().unwrap_or("=");
        Some(match op {
            "=" | "==" => a == b,
            "<" => a < b,
            "<=" => a <= b,
            ">" => a > b,
            ">=" => a >= b,
            _ => false,
        })
    }
}

/* ─────────────────────────────── Parsing ────────────────────────────────── */

/// Parser errors for [`parse_constraint`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ParseError {
    Empty,
    UnknownLevel(String),
    BadSyntax(String),
}

impl std::fmt::Display for ParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Empty => write!(f, "empty IRQL constraint"),
            Self::UnknownLevel(l) => write!(
                f,
                "unknown IRQL level '{l}' (expected one of: {})",
                KNOWN_LEVELS.join(", ")
            ),
            Self::BadSyntax(s) => write!(f, "bad IRQL syntax: {s:?}"),
        }
    }
}

impl std::error::Error for ParseError {}

/// Parse a filter-input string like `"<= DISPATCH_LEVEL"`, `"PASSIVE_LEVEL"`,
/// or `"<=DISPATCH_LEVEL"` into an [`IrqlConstraint`].
///
/// Whitespace is permissive. The level token is matched case-sensitively
/// against [`KNOWN_LEVELS`] (since real frontmatter uses uppercase). No-op
/// strings collapse to `op: None`.
pub fn parse_constraint(s: &str) -> Result<IrqlConstraint, ParseError> {
    let s = s.trim();
    if s.is_empty() {
        return Err(ParseError::Empty);
    }

    // Match the longest operator prefix first so "<=" beats "<".
    let mut op: Option<&str> = None;
    let mut rest = s;
    for candidate in ["<=", ">=", "==", "=", "<", ">"] {
        if let Some(after) = rest.strip_prefix(candidate) {
            op = Some(candidate);
            rest = after;
            break;
        }
    }

    let level = rest.trim();
    if level.is_empty() {
        return Err(ParseError::BadSyntax(s.into()));
    }
    if !KNOWN_LEVELS.iter().any(|&l| l == level) {
        return Err(ParseError::UnknownLevel(level.into()));
    }

    Ok(IrqlConstraint {
        level: level.to_string(),
        op: op.map(String::from),
    })
}

/* ────────────────────────────────── Tests ───────────────────────────────── */

#[cfg(test)]
mod tests {
    use super::*;

    fn lookup() -> HashMap<String, u64> {
        // Match the canonical x64 wdm.h values.
        HashMap::from([
            ("PASSIVE_LEVEL".into(), 0),
            ("APC_LEVEL".into(), 1),
            ("DISPATCH_LEVEL".into(), 2),
            ("DPC_LEVEL".into(), 2),
            ("HIGH_LEVEL".into(), 31),
            ("IPI_LEVEL".into(), 29),
        ])
    }

    #[test]
    fn parse_bare_level() {
        let c = parse_constraint("PASSIVE_LEVEL").unwrap();
        assert_eq!(c.level, "PASSIVE_LEVEL");
        assert_eq!(c.op, None);
    }

    #[test]
    fn parse_with_op() {
        let c = parse_constraint("<= DISPATCH_LEVEL").unwrap();
        assert_eq!(c.level, "DISPATCH_LEVEL");
        assert_eq!(c.op.as_deref(), Some("<="));
    }

    #[test]
    fn parse_no_space() {
        let c = parse_constraint("<=DISPATCH_LEVEL").unwrap();
        assert_eq!(c.op.as_deref(), Some("<="));
    }

    #[test]
    fn parse_rejects_unknown() {
        assert!(matches!(
            parse_constraint("MAGIC_LEVEL"),
            Err(ParseError::UnknownLevel(_))
        ));
    }

    #[test]
    fn parse_rejects_empty() {
        assert!(matches!(parse_constraint(""), Err(ParseError::Empty)));
        assert!(matches!(parse_constraint("<= "), Err(ParseError::BadSyntax(_))));
    }

    #[test]
    fn any_filter_matches_everything() {
        let any = IrqlConstraint { level: "ANY".into(), op: None };
        let c = IrqlConstraint { level: "DISPATCH_LEVEL".into(), op: None };
        assert_eq!(c.matches(&any, &lookup()), Some(true));
    }

    #[test]
    fn numeric_le_filter() {
        let filter = parse_constraint("<= DISPATCH_LEVEL").unwrap();
        let passive = IrqlConstraint { level: "PASSIVE_LEVEL".into(), op: None };
        let dispatch = IrqlConstraint { level: "DISPATCH_LEVEL".into(), op: None };
        let high = IrqlConstraint { level: "HIGH_LEVEL".into(), op: None };
        assert_eq!(passive.matches(&filter, &lookup()), Some(true));
        assert_eq!(dispatch.matches(&filter, &lookup()), Some(true));
        assert_eq!(high.matches(&filter, &lookup()), Some(false));
    }

    #[test]
    fn exact_filter_matches_by_value() {
        // DPC_LEVEL and DISPATCH_LEVEL share the numeric value 2.
        let filter = IrqlConstraint { level: "DPC_LEVEL".into(), op: None };
        let dispatch = IrqlConstraint { level: "DISPATCH_LEVEL".into(), op: None };
        assert_eq!(dispatch.matches(&filter, &lookup()), Some(true));
    }

    #[test]
    fn unresolvable_levels_return_none() {
        let filter = parse_constraint("<= DISPATCH_LEVEL").unwrap();
        let dirql = IrqlConstraint { level: "DIRQL".into(), op: None };
        let device = IrqlConstraint { level: "DEVICE_LEVEL".into(), op: None };
        assert_eq!(dirql.matches(&filter, &lookup()), None);
        assert_eq!(device.matches(&filter, &lookup()), None);
    }
}
