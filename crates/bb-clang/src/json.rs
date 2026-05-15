//! JSON serialization trait for bb-clang types.

use std::collections::HashSet;

use serde_json::Value;

use crate::constant::Constant;
use crate::enum_::Enum;
use crate::function::{Function, Param};
use crate::struct_::Field;
use crate::struct_::Struct;
use crate::union_::Union;

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

impl ToJson for Function<'_> {
    fn to_json(&self) -> Value {
        serde_json::to_value(self).unwrap()
    }
}

impl ToJson for Param<'_> {
    fn to_json(&self) -> Value {
        serde_json::to_value(self).unwrap()
    }
}

impl ToJson for Struct<'_> {
    /// Basic serialization. Adds `referenced_types` as a name-string
    /// array of every *named* referenced record (struct or union).
    /// Anonymous nested records have no name to put in a string list
    /// — they're only emitted by [`Self::to_json_full`].
    fn to_json(&self) -> Value {
        let mut val = serde_json::to_value(self).unwrap();
        let obj = val.as_object_mut().unwrap();
        obj.insert(
            "referenced_types".to_string(),
            serde_json::to_value(self.referenced_type_names()).unwrap(),
        );
        val
    }

    /// Full serialization. Emits the struct itself under `"type"` and
    /// a `referenced_types` array of full objects — both structs and
    /// unions, named or anonymous, distinguished per-entry by `"kind"`.
    fn to_json_full(&self) -> Value {
        let (structs, unions) = self.extract_nested_records(MAX_DEPTH);
        let referenced: Vec<Value> = structs
            .iter()
            .map(|s| serde_json::to_value(s).unwrap())
            .chain(unions.iter().map(|u| serde_json::to_value(u).unwrap()))
            .collect();
        serde_json::json!({
            "type": serde_json::to_value(self).unwrap(),
            "referenced_types": referenced,
        })
    }
}

impl ToJson for Union<'_> {
    fn to_json(&self) -> Value {
        let mut val = serde_json::to_value(self).unwrap();
        let obj = val.as_object_mut().unwrap();
        obj.insert(
            "referenced_types".to_string(),
            serde_json::to_value(self.referenced_type_names()).unwrap(),
        );
        val
    }

    fn to_json_full(&self) -> Value {
        let (structs, unions) = self.extract_nested_records(MAX_DEPTH);
        let referenced: Vec<Value> = structs
            .iter()
            .map(|s| serde_json::to_value(s).unwrap())
            .chain(unions.iter().map(|u| serde_json::to_value(u).unwrap()))
            .collect();
        serde_json::json!({
            "type": serde_json::to_value(self).unwrap(),
            "referenced_types": referenced,
        })
    }
}

/// Universally handle references.
impl<T: ToJson> ToJson for &T {
    fn to_json(&self) -> Value {
        (*self).to_json()
    }

    fn to_json_full(&self) -> Value {
        (*self).to_json_full()
    }
}

/* ───────────────────────────────── Slices ───────────────────────────────── */

impl ToJson for [Constant<'_>] {
    fn to_json(&self) -> Value {
        slice_to_json(self)
    }

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

impl ToJson for [Enum<'_>] {
    fn to_json(&self) -> Value {
        slice_to_json(self)
    }

    fn to_json_full(&self) -> Value {
        slice_to_json_full(self)
    }
}

impl ToJson for [Field<'_>] {
    fn to_json(&self) -> Value {
        slice_to_json(self)
    }

    fn to_json_full(&self) -> Value {
        slice_to_json_full(self)
    }
}

impl ToJson for [Function<'_>] {
    fn to_json(&self) -> Value {
        slice_to_json(self)
    }

    fn to_json_full(&self) -> Value {
        slice_to_json_full(self)
    }
}

impl ToJson for [Param<'_>] {
    fn to_json(&self) -> Value {
        slice_to_json(self)
    }

    fn to_json_full(&self) -> Value {
        slice_to_json_full(self)
    }
}

/// `to_json` produces an array of structs.
/// `to_json_full` produces `{ "types": [...], "referenced_types": [...] }`
/// where `referenced_types` mixes structs and unions discovered through
/// any field of the slice's records (recursively up to [`MAX_DEPTH`],
/// deduplicated by composite identity). Each entry carries `"kind"` so
/// the consumer can dispatch.
impl ToJson for [Struct<'_>] {
    fn to_json(&self) -> Value {
        slice_to_json(self)
    }

    fn to_json_full(&self) -> Value {
        let mut seen: HashSet<String> = self.iter().map(|s| s.identity()).collect();

        let types: Vec<Value> = self
            .iter()
            .map(|s| serde_json::to_value(s).unwrap())
            .collect();

        let mut referenced: Vec<Value> = Vec::new();
        for s in self {
            let (nested_structs, nested_unions) = s.extract_nested_records(MAX_DEPTH);
            for ns in nested_structs {
                if seen.insert(ns.identity()) {
                    referenced.push(serde_json::to_value(&ns).unwrap());
                }
            }
            for nu in nested_unions {
                if seen.insert(nu.identity()) {
                    referenced.push(serde_json::to_value(&nu).unwrap());
                }
            }
        }

        serde_json::json!({
            "types": types,
            "referenced_types": referenced,
        })
    }
}

/// Universally handle slices of references.
impl<T: ToJson> ToJson for [&T] {
    fn to_json(&self) -> Value {
        slice_to_json(self)
    }

    fn to_json_full(&self) -> Value {
        slice_to_json_full(self)
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

/* ───────────────────────────────── Helpers ──────────────────────────────── */

fn slice_to_json<T: ToJson>(slice: &[T]) -> Value {
    Value::Array(slice.iter().map(T::to_json).collect())
}

fn slice_to_json_full<T: ToJson>(slice: &[T]) -> Value {
    Value::Array(slice.iter().map(T::to_json_full).collect())
}

/* ─────────────────────── Mixed struct + union helper ───────────────────── */

/// Full JSON shape for a heterogeneous set of records (some structs,
/// some unions). Used by `bb-types` and any other consumer that wants
/// to dump struct and union queries together.
///
/// Returns:
/// ```jsonc
/// {
///   "types":            [ /* structs first, then unions; each carries "kind" */ ],
///   "referenced_types": [ /* every record reachable via fields, dedup'd by composite identity */ ]
/// }
/// ```
///
/// Each entry in `referenced_types` carries `"kind"` ("struct" /
/// "union") so consumers can dispatch. Anonymous nested entries
/// additionally carry `enclosing_record` + `field_path` for
/// cross-reference from a parent's anon-typed field.
#[must_use]
pub fn records_to_json_full(structs: &[Struct<'_>], unions: &[Union<'_>]) -> Value {
    let mut types: Vec<Value> = structs
        .iter()
        .map(|s| serde_json::to_value(s).expect("Struct serializes"))
        .collect();
    types.extend(
        unions
            .iter()
            .map(|u| serde_json::to_value(u).expect("Union serializes")),
    );

    let mut seen: HashSet<String> = structs
        .iter()
        .map(Struct::identity)
        .chain(unions.iter().map(Union::identity))
        .collect();
    let mut referenced: Vec<Value> = Vec::new();
    for s in structs {
        let (ns, nu) = s.extract_nested_records(MAX_DEPTH);
        extend_referenced_records(ns, nu, &mut seen, &mut referenced);
    }
    for u in unions {
        let (ns, nu) = u.extract_nested_records(MAX_DEPTH);
        extend_referenced_records(ns, nu, &mut seen, &mut referenced);
    }

    serde_json::json!({
        "types": types,
        "referenced_types": referenced,
    })
}

fn extend_referenced_records(
    structs: Vec<Struct<'_>>,
    unions: Vec<Union<'_>>,
    seen: &mut HashSet<String>,
    out: &mut Vec<Value>,
) {
    for r in structs {
        if seen.insert(r.identity()) {
            out.push(serde_json::to_value(&r).expect("Struct serializes"));
        }
    }
    for r in unions {
        if seen.insert(r.identity()) {
            out.push(serde_json::to_value(&r).expect("Union serializes"));
        }
    }
}

/* ──────────────────────────── Component helpers ─────────────────────────── */

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

pub fn collect_component_constants(
    c: &Constant,
    seen: &mut HashSet<String>,
    result: &mut Vec<Value>,
) {
    for comp in c.get_component_constants() {
        collect_component_constants(comp, seen, result);
        if seen.insert(comp.get_name().to_string()) {
            result.push(comp.to_json());
        }
    }
}
