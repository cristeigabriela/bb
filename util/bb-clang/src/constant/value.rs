//! Constant value representation and lookup type.

use std::collections::HashMap;
use std::fmt;
use std::num::Wrapping;

use cexpr::expr::EvalResult;
use clang::EvaluationResult;
use serde::Serialize;

/* ────────────────────────────────── Types ───────────────────────────────── */

/// Lookup table mapping constant names to their resolved values.
pub type ConstLookup = HashMap<String, ConstValue>;

/// A structure that unifies the possible values of constants into one.
///
/// This representation might only be constructed from:
/// - [`EvaluationResult`] for [`clang::EntityKind::VarDecl`] entities;
/// - [`clang::Entity::get_enum_constant_value`] for [`clang::EntityKind::EnumConstantDecl`] entities;
/// - [`EvalResult`] for [`clang::EntityKind::MacroDefinition`] entities.
///
/// This structure also supports utilities such as future serialization if it's contents,
/// and hex/decimal formatting depending on the match arm.
#[derive(Debug, Clone, Copy)]
pub enum ConstValue {
    I64(i64),
    U64(u64),
    F64(f64),
}

impl Serialize for ConstValue {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        match self {
            Self::I64(v) => serializer.serialize_i64(*v),
            Self::U64(v) => serializer.serialize_u64(*v),
            Self::F64(v) => serializer.serialize_f64(*v),
        }
    }
}

impl ConstValue {
    /// Create from a clang [`EvaluationResult`], returning [`None`] for non-numeric results
    /// (strings, unexposed, etc.).
    pub(crate) fn from_eval(result: EvaluationResult) -> Option<Self> {
        match result {
            EvaluationResult::SignedInteger(v) => Some(Self::I64(v)),
            EvaluationResult::UnsignedInteger(v) => Some(Self::U64(v)),
            EvaluationResult::Float(v) => Some(Self::F64(v)),
            _ => None,
        }
    }

    /// Create from an enum constant's signed/unsigned value pair.
    ///
    /// Uses the signed representation when negative, unsigned otherwise.
    pub(crate) const fn from_enum_constant(signed: i64, unsigned: u64) -> Self {
        if signed < 0 {
            Self::I64(signed)
        } else {
            Self::U64(unsigned)
        }
    }

    /// Convert to `u64`, casting signed values. Returns `None` for floats.
    #[must_use]
    pub const fn as_u64(&self) -> Option<u64> {
        match self {
            Self::I64(v) => Some(*v as u64),
            Self::U64(v) => Some(*v),
            Self::F64(_) => None,
        }
    }

    /// Create from a [`cexpr::expr::EvalResult`].
    pub(crate) fn from_cexpr(result: EvalResult) -> Option<Self> {
        match result {
            EvalResult::Int(Wrapping(v)) if v < 0 => Some(Self::I64(v)),
            EvalResult::Int(Wrapping(v)) => Some(Self::U64(v as u64)),
            EvalResult::Float(v) => Some(Self::F64(v)),
            _ => None,
        }
    }
}

/* ──────────────────────────────── Displays ──────────────────────────────── */

/// Format a [`ConstValue`] for display.
///
/// Integers are shown in hex, floats in decimal.
impl fmt::Display for ConstValue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::I64(v) if *v < 0 => write!(f, "-0x{:X}", v.unsigned_abs()),
            Self::I64(v) => write!(f, "0x{v:X}"),
            Self::U64(v) => write!(f, "0x{v:X}"),
            Self::F64(v) => write!(f, "{v}"),
        }
    }
}
