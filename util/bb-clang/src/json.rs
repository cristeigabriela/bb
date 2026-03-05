//! JSON serialization trait for bb-clang types.

use std::collections::HashSet;

use serde_json::Value;

use crate::constant::Constant;
use crate::enum_::Enum;
use crate::struct_::Field;
use crate::struct_::Struct;

/// Maximum nesting depth for full struct expansion.
const MAX_DEPTH: usize = 8;

/* ────────────────────────────────── Trait ───────────────────────────────── */

/// Convert a bb-clang type to a [`serde_json::Value`].
pub trait ToJson {
    /// Basic JSON serialization.
    fn to_json(&self) -> Value;

    /// Full JSON serialization with maximum detail.
    /// Identical to [`to_json`](ToJson::to_json) unless overridden.
    fn to_json_full(&self) -> Value {
        self.to_json()
    }
}

/* ───────────────────────────────── Scalars ──────────────────────────────── */

impl ToJson for Constant<'_> {
    fn to_json(&self) -> Value {
        serde_json::to_value(self).unwrap()
    }

    /// Like [`to_json`](ToJson::to_json) but with a `referred_components` field
    /// containing the fully serialized component constants.
    fn to_json_full(&self) -> Value {
        let mut val = self.to_json();
        val.as_object_mut().unwrap().insert(
            "referred_components".to_string(),
            serde_json::json!(build_referred_components(
                std::iter::once(self.get_name().to_string()),
                std::slice::from_ref(self).iter(),
            )),
        );
        val
    }
}

impl ToJson for Enum<'_> {
    fn to_json(&self) -> Value {
        serde_json::to_value(self).unwrap()
    }
}

impl ToJson for Field<'_> {
    fn to_json(&self) -> Value {
        serde_json::to_value(self).unwrap()
    }
}

impl ToJson for Struct<'_> {
    /// Serializes the struct with `referenced_types` as a list of type names.
    fn to_json(&self) -> Value {
        let mut val = serde_json::to_value(self).unwrap();
        val.as_object_mut().unwrap().insert(
            "referenced_types".to_string(),
            serde_json::to_value(self.referenced_type_names()).unwrap(),
        );
        val
    }

    /// Returns `{ "type": {..}, "referenced_types": [{..}, ..] }` where
    /// `referenced_types` contains fully expanded nested types up to
    /// [`MAX_DEPTH`], each with their own `referenced_types` name list.
    fn to_json_full(&self) -> Value {
        let mut seen = HashSet::new();
        seen.insert(self.get_name().to_string());

        let referenced: Vec<Value> = self
            .extract_nested_types(MAX_DEPTH)
            .into_iter()
            .filter(|n| seen.insert(n.get_name().to_string()))
            .map(|n| serde_json::to_value(n).unwrap())
            .collect();

        serde_json::json!({
            "type": serde_json::to_value(self).unwrap(),
            "referenced_types": referenced,
        })
    }
}

impl<T: ToJson> ToJson for &T {
    fn to_json(&self) -> Value {
        (*self).to_json()
    }

    fn to_json_full(&self) -> Value {
        (*self).to_json_full()
    }
}

/* ───────────────────────────────── Slices ───────────────────────────────── */

/// Helper: map each element's `to_json` into a JSON array.
fn slice_to_json<T: ToJson>(slice: &[T]) -> Value {
    Value::Array(slice.iter().map(T::to_json).collect())
}

impl ToJson for [Constant<'_>] {
    fn to_json(&self) -> Value {
        slice_to_json(self)
    }

    /// Returns `{ "constants": [...], "referred_components": [...] }`.
    ///
    /// `referred_components` is a deduplicated, depth-first list of all
    /// [`Constant`]s that are transitively referenced as components by the
    /// constants in this slice. Constants already present in the slice are
    /// excluded from `referred_components`.
    fn to_json_full(&self) -> Value {
        let mut seen: HashSet<String> = self.iter().map(|c| c.get_name().to_string()).collect();
        let mut referred = Vec::new();
        for c in self {
            collect_component_constants(c, &mut seen, &mut referred);
        }
        serde_json::json!({
            "constants": self.to_json(),
            "referred_components": referred,
        })
    }
}


impl ToJson for [&Constant<'_>] {
    fn to_json(&self) -> Value {
        slice_to_json(self)
    }
}

impl ToJson for [Enum<'_>] {
    fn to_json(&self) -> Value {
        slice_to_json(self)
    }
}

impl ToJson for [&Enum<'_>] {
    fn to_json(&self) -> Value {
        slice_to_json(self)
    }
}

impl ToJson for [Field<'_>] {
    fn to_json(&self) -> Value {
        slice_to_json(self)
    }
}

impl ToJson for [&Field<'_>] {
    fn to_json(&self) -> Value {
        slice_to_json(self)
    }
}

impl ToJson for [&Struct<'_>] {
    fn to_json(&self) -> Value {
        slice_to_json(self)
    }
}

/// `to_json` produces an array of structs.
/// `to_json_full` produces `{ "types": [...], "referenced_types": [...] }`
/// with all nested types across the slice expanded up to [`MAX_DEPTH`]
/// and deduplicated.
impl ToJson for [Struct<'_>] {
    fn to_json(&self) -> Value {
        slice_to_json(self)
    }

    fn to_json_full(&self) -> Value {
        let mut seen: HashSet<String> = self.iter().map(|s| s.get_name().to_string()).collect();

        let types: Vec<Value> = self
            .iter()
            .map(|s| serde_json::to_value(s).unwrap())
            .collect();

        let mut referenced = Vec::new();
        for s in self {
            for nested in s.extract_nested_types(MAX_DEPTH) {
                if seen.insert(nested.get_name().to_string()) {
                    referenced.push(serde_json::to_value(&nested).unwrap());
                }
            }
        }

        serde_json::json!({
            "types": types,
            "referenced_types": referenced,
        })
    }
}

/* ────────────────────────────────── Vecs ────────────────────────────────── */

impl<T: ToJson> ToJson for Vec<T>
where
    [T]: ToJson,
{
    fn to_json(&self) -> Value {
        self.as_slice().to_json()
    }

    fn to_json_full(&self) -> Value {
        self.as_slice().to_json_full()
    }
}

/* ──────────────────────────── Component helpers ─────────────────────────── */

/// Collect the `referred_components` array for a `to_json_full` result.
///
/// Seeds a `seen` set from `primary_names`, then transitively collects all
/// component constants referenced by `constants` (depth-first, deduplicated).
/// Returns an empty `Vec` when there are no referred components.
pub fn build_referred_components<'a>(
    primary_names: impl IntoIterator<Item = String>,
    constants: impl IntoIterator<Item = &'a Constant<'a>>,
) -> Vec<Value> {
    let mut seen: HashSet<String> = primary_names.into_iter().collect();
    let mut referred = Vec::new();
    for c in constants {
        collect_component_constants(c, &mut seen, &mut referred);
    }
    referred
}

/// Recursively collect all [`Constant`]s transitively referenced as components,
/// depth-first (dependencies before dependents), deduplicating via `seen`.
///
/// Constants whose names are already in `seen` (i.e. already in the main
/// result set or already collected) are skipped.
pub fn collect_component_constants(
    c: &Constant,
    seen: &mut HashSet<String>,
    result: &mut Vec<Value>,
) {
    for comp in c.get_component_constants() {
        // Recurse into this component's own dependencies first.
        collect_component_constants(comp, seen, result);
        if seen.insert(comp.get_name().to_string()) {
            result.push(comp.to_json());
        }
    }
}
