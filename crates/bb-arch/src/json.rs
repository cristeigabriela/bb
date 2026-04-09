//! JSON serialization trait for bb-arch types.

use serde_json::{Value, json};

use crate::display::register_name;
use crate::location::{MemoryOperand, ParamLocation, ReturnLocation};

/* ────────────────────────────────── Trait ───────────────────────────────── */

/// Convert a bb-arch type to a [`serde_json::Value`].
pub trait ToJson {
    /// Structured JSON serialization.
    fn to_json(&self) -> Value;
}

/* ────────────────────────────────── Impls ───────────────────────────────── */

impl ToJson for MemoryOperand {
    fn to_json(&self) -> Value {
        match self {
            Self::Reg(r) => json!({
                "kind": "reg",
                "register": register_name(r),
            }),
            Self::RegImm { base, offset } => json!({
                "kind": "stack",
                "base": register_name(base),
                "offset": offset,
            }),
        }
    }
}

impl ToJson for ParamLocation {
    fn to_json(&self) -> Value {
        match self {
            Self::Direct { locations, size } => {
                let mut obj = match locations.first() {
                    Some(op) => op.to_json(),
                    None => json!({ "kind": "?" }),
                };
                if let Some(map) = obj.as_object_mut() {
                    map.insert("size".into(), json!(size));
                }
                obj
            }
            Self::Indirect { pointer, size } => json!({
                "kind": "indirect",
                "pointer": pointer.to_json(),
                "size": size,
            }),
        }
    }
}

impl ToJson for ReturnLocation {
    fn to_json(&self) -> Value {
        match self {
            Self::Void => json!({ "kind": "void" }),
            Self::Register(r) => json!({
                "kind": "reg",
                "register": register_name(r),
            }),
            Self::Indirect => json!({ "kind": "indirect" }),
        }
    }
}
