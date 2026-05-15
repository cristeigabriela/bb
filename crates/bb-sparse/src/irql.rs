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
    /// `filter.op`?
    ///
    /// Each side describes a **range** of IRQLs at which the function is
    /// callable, not a single point:
    ///
    /// - bare `X` / `= X` / `== X` → `[X, X]` (callable only at X)
    /// - `<= X` → `[0, X]`     (callable at any IRQL up to and including X)
    /// - `<  X` → `[0, X-1]`
    /// - `>= X` → `[X, HIGH]`
    /// - `>  X` → `[X+1, HIGH]`
    ///
    /// The filter is interpreted as a constraint on the function's range:
    ///
    /// - filter `= Y`  → the function's range covers Y (`min ≤ Y ≤ max`)
    /// - filter `<= Y` → the function is callable **only** at IRQL ≤ Y (`max ≤ Y`)
    /// - filter `<  Y` → `max < Y`
    /// - filter `>= Y` → the function is callable **only** at IRQL ≥ Y (`min ≥ Y`)
    /// - filter `>  Y` → `min > Y`
    ///
    /// This is the semantics behind issue #23: a function documented
    /// `<= DISPATCH_LEVEL` is callable at PASSIVE through DISPATCH, so a
    /// filter of `> PASSIVE_LEVEL` (i.e. "callable only above PASSIVE")
    /// must NOT match it.
    ///
    /// Returns `None` when a side can't be resolved numerically (e.g.
    /// `DEVICE_LEVEL` or `DIRQL` have no single concrete value) — the
    /// caller decides whether to keep or drop the entry. Pure symbolic
    /// matches short-circuit so symbolic-only constraints still work.
    #[must_use]
    pub fn matches(&self, filter: &Self, lookup: &HashMap<String, u64>) -> Option<bool> {
        // `ANY` filter matches everything.
        if filter.level == "ANY" {
            return Some(true);
        }
        // Exact symbolic equality covers same-level same-op cases (incl.
        // both ops being None). Keeps DIRQL / DEVICE_LEVEL constraints
        // workable for exact-string filters.
        if self.level == filter.level && self.op == filter.op {
            return Some(true);
        }

        let (fn_min, fn_max) = self.as_range(lookup)?;
        let y = filter.resolve_level(lookup)?;
        let op = filter.op.as_deref().unwrap_or("=");
        Some(match op {
            "=" | "==" => fn_min <= y && y <= fn_max,
            "<" => fn_max < y,
            "<=" => fn_max <= y,
            ">" => fn_min > y,
            ">=" => fn_min >= y,
            _ => false,
        })
    }

    /// Resolve this constraint to a `[min, max]` numeric IRQL range
    /// according to its operator. See [`matches`](Self::matches) for the
    /// mapping. Returns `None` when the level can't be resolved (e.g.
    /// `DIRQL`).
    ///
    /// `HIGH_LEVEL` is used as the canonical upper bound; we don't
    /// special-case wraparound (operators in practice anchor at named
    /// levels, never at `HIGH_LEVEL + 1`).
    fn as_range(&self, lookup: &HashMap<String, u64>) -> Option<(u64, u64)> {
        let lvl = self.resolve_level(lookup)?;
        let high = lookup.get("HIGH_LEVEL").copied().unwrap_or(31);
        let op = self.op.as_deref().unwrap_or("=");
        Some(match op {
            "=" | "==" => (lvl, lvl),
            "<=" => (0, lvl),
            "<" => (0, lvl.saturating_sub(1)),
            ">=" => (lvl, high),
            ">" => (lvl.saturating_add(1).min(high), high),
            _ => (lvl, lvl),
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
    if !KNOWN_LEVELS.contains(&level) {
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
        assert!(matches!(
            parse_constraint("<= "),
            Err(ParseError::BadSyntax(_))
        ));
    }

    #[test]
    fn any_filter_matches_everything() {
        let any = IrqlConstraint {
            level: "ANY".into(),
            op: None,
        };
        let c = IrqlConstraint {
            level: "DISPATCH_LEVEL".into(),
            op: None,
        };
        assert_eq!(c.matches(&any, &lookup()), Some(true));
    }

    #[test]
    fn numeric_le_filter() {
        let filter = parse_constraint("<= DISPATCH_LEVEL").unwrap();
        let passive = IrqlConstraint {
            level: "PASSIVE_LEVEL".into(),
            op: None,
        };
        let dispatch = IrqlConstraint {
            level: "DISPATCH_LEVEL".into(),
            op: None,
        };
        let high = IrqlConstraint {
            level: "HIGH_LEVEL".into(),
            op: None,
        };
        assert_eq!(passive.matches(&filter, &lookup()), Some(true));
        assert_eq!(dispatch.matches(&filter, &lookup()), Some(true));
        assert_eq!(high.matches(&filter, &lookup()), Some(false));
    }

    #[test]
    fn exact_filter_matches_by_value() {
        // DPC_LEVEL and DISPATCH_LEVEL share the numeric value 2.
        let filter = IrqlConstraint {
            level: "DPC_LEVEL".into(),
            op: None,
        };
        let dispatch = IrqlConstraint {
            level: "DISPATCH_LEVEL".into(),
            op: None,
        };
        assert_eq!(dispatch.matches(&filter, &lookup()), Some(true));
    }

    #[test]
    fn unresolvable_levels_return_none() {
        let filter = parse_constraint("<= DISPATCH_LEVEL").unwrap();
        let dirql = IrqlConstraint {
            level: "DIRQL".into(),
            op: None,
        };
        let device = IrqlConstraint {
            level: "DEVICE_LEVEL".into(),
            op: None,
        };
        assert_eq!(dirql.matches(&filter, &lookup()), None);
        assert_eq!(device.matches(&filter, &lookup()), None);
    }

    /* ──────── Range semantics (issue #23 regressions) ──────── */

    /// `<= DISPATCH_LEVEL` covers PASSIVE..=DISPATCH (range [0, 2]).
    /// A filter of `> PASSIVE_LEVEL` asks for functions whose MIN is > 0.
    /// Since this function's min is 0, it must NOT match.
    /// This is the issue #23 repro: bb-funcs returned `RtlInitUTF8StringEx`
    /// (`<= DISPATCH_LEVEL`) for `> PASSIVE_LEVEL`.
    #[test]
    fn range_le_dispatch_does_not_match_gt_passive() {
        let func = IrqlConstraint {
            level: "DISPATCH_LEVEL".into(),
            op: Some("<=".into()),
        };
        let filter = parse_constraint("> PASSIVE_LEVEL").unwrap();
        assert_eq!(func.matches(&filter, &lookup()), Some(false));
    }

    /// `>= DISPATCH_LEVEL` covers DISPATCH..=HIGH (range [2, 31]).
    /// A filter of `> PASSIVE_LEVEL` asks min > 0. Min is 2, so MATCH.
    #[test]
    fn range_ge_dispatch_matches_gt_passive() {
        let func = IrqlConstraint {
            level: "DISPATCH_LEVEL".into(),
            op: Some(">=".into()),
        };
        let filter = parse_constraint("> PASSIVE_LEVEL").unwrap();
        assert_eq!(func.matches(&filter, &lookup()), Some(true));
    }

    /// A bare-level constraint represents a single-point range.
    /// `DISPATCH_LEVEL` (range [2, 2]) matches `> PASSIVE_LEVEL` (min 2 > 0).
    #[test]
    fn range_bare_dispatch_matches_gt_passive() {
        let func = IrqlConstraint {
            level: "DISPATCH_LEVEL".into(),
            op: None,
        };
        let filter = parse_constraint("> PASSIVE_LEVEL").unwrap();
        assert_eq!(func.matches(&filter, &lookup()), Some(true));
    }

    /// Filter `= DISPATCH_LEVEL` asks: does the function's range cover
    /// `DISPATCH_LEVEL`? `<= DISPATCH_LEVEL` (range [0, 2]) covers 2, so MATCH.
    #[test]
    fn range_eq_filter_checks_coverage() {
        let func_le = IrqlConstraint {
            level: "DISPATCH_LEVEL".into(),
            op: Some("<=".into()),
        };
        let filter = parse_constraint("= DISPATCH_LEVEL").unwrap();
        assert_eq!(func_le.matches(&filter, &lookup()), Some(true));

        // PASSIVE_LEVEL (range [0, 0]) does NOT cover DISPATCH_LEVEL (2).
        let func_passive = IrqlConstraint {
            level: "PASSIVE_LEVEL".into(),
            op: None,
        };
        assert_eq!(func_passive.matches(&filter, &lookup()), Some(false));
    }

    /// Filter `<= DISPATCH_LEVEL` asks: is the function's max ≤ 2?
    /// `>= APC_LEVEL` (range [1, 31]) has max 31 → NO MATCH.
    #[test]
    fn range_le_filter_rejects_wider_max() {
        let func = IrqlConstraint {
            level: "APC_LEVEL".into(),
            op: Some(">=".into()),
        };
        let filter = parse_constraint("<= DISPATCH_LEVEL").unwrap();
        assert_eq!(func.matches(&filter, &lookup()), Some(false));
    }

    /// Filter `>= APC_LEVEL` asks: is the function's min ≥ 1?
    /// `<= DISPATCH_LEVEL` (range [0, 2]) has min 0 → NO MATCH.
    /// This is the same family of bug as the `>` case in #23.
    #[test]
    fn range_ge_filter_rejects_lower_min() {
        let func = IrqlConstraint {
            level: "DISPATCH_LEVEL".into(),
            op: Some("<=".into()),
        };
        let filter = parse_constraint(">= APC_LEVEL").unwrap();
        assert_eq!(func.matches(&filter, &lookup()), Some(false));
    }

    /// A bare-level filter (`--irql APC_LEVEL`) is the same as
    /// `--irql "= APC_LEVEL"`: keep functions whose callable range
    /// covers `APC_LEVEL`.
    #[test]
    fn bare_filter_is_equivalent_to_eq() {
        let bare = IrqlConstraint {
            level: "APC_LEVEL".into(),
            op: None,
        };
        let eq = IrqlConstraint {
            level: "APC_LEVEL".into(),
            op: Some("=".into()),
        };

        // <= DISPATCH_LEVEL covers APC_LEVEL — match under both filter forms.
        let le_dispatch = IrqlConstraint {
            level: "DISPATCH_LEVEL".into(),
            op: Some("<=".into()),
        };
        assert_eq!(le_dispatch.matches(&bare, &lookup()), Some(true));
        assert_eq!(le_dispatch.matches(&eq, &lookup()), Some(true));

        // Bare PASSIVE_LEVEL does NOT cover APC_LEVEL — no match under either.
        let bare_passive = IrqlConstraint {
            level: "PASSIVE_LEVEL".into(),
            op: None,
        };
        assert_eq!(bare_passive.matches(&bare, &lookup()), Some(false));
        assert_eq!(bare_passive.matches(&eq, &lookup()), Some(false));

        // Bare APC_LEVEL exactly matches the bare filter (symbolic shortcut)
        // and also matches the `= APC_LEVEL` filter via coverage.
        let bare_apc = IrqlConstraint {
            level: "APC_LEVEL".into(),
            op: None,
        };
        assert_eq!(bare_apc.matches(&bare, &lookup()), Some(true));
        assert_eq!(bare_apc.matches(&eq, &lookup()), Some(true));
    }

    /// Strict-less filter `< APC_LEVEL` asks: is the function's max < 1?
    /// Bare `PASSIVE_LEVEL` (range [0, 0]) has max 0 → MATCH.
    /// `<= DISPATCH_LEVEL` (range [0, 2]) has max 2 → NO MATCH.
    #[test]
    fn range_lt_filter_max_strictly_below() {
        let passive = IrqlConstraint {
            level: "PASSIVE_LEVEL".into(),
            op: None,
        };
        let le_dispatch = IrqlConstraint {
            level: "DISPATCH_LEVEL".into(),
            op: Some("<=".into()),
        };
        let filter = parse_constraint("< APC_LEVEL").unwrap();
        assert_eq!(passive.matches(&filter, &lookup()), Some(true));
        assert_eq!(le_dispatch.matches(&filter, &lookup()), Some(false));
    }
}
