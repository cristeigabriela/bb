//! Shared building blocks for record (struct + union) representation.
//!
//! [`RecordKind`] is the type-level discriminator that lives on every
//! record JSON entry — top-level [`Struct`](crate::Struct), top-level
//! [`Union`](crate::Union), or any anonymous nested record surfaced in a
//! struct's `referenced_structs` / `referenced_unions` slot. Consumers
//! switch on `kind` to dispatch reconstruction.
//!
//! [`AnonRef`] is the cross-reference an anonymous nested record gets
//! from the field that points at it. The pair
//! `(enclosing_record, field_path)` is the canonical identity for an
//! anonymous record across the JSON output: `enclosing_record` is always
//! a named ancestor (walked via `semantic_parent`), `field_path` is the
//! chain of field names from that named ancestor down to this record.
//! Anonymous fields along the way carry a synthetic name of the form
//! `<anonymous_N>` where `N` is the per-parent counter — see
//! [`crate::Field`] for how the counter is assigned.

use serde::Serialize;

/* ────────────────────────────────── Types ───────────────────────────────── */

/// Discriminates struct-shaped records from union-shaped records.
///
/// Serializes as `"struct"` / `"union"` in JSON. Top-level
/// [`Struct`](crate::Struct) entries always carry `Struct`;
/// [`Union`](crate::Union) entries always carry `Union`. Anonymous
/// nested records carry whichever they actually are — the same record
/// type can model either depending on the underlying clang decl.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RecordKind {
    Struct,
    Union,
}

/// Cross-reference to an anonymous record nested inside a named record.
///
/// Set on a [`Field`](crate::Field) when the field's underlying type is
/// an anonymous struct or union. The tuple `(kind, enclosing_record,
/// field_path)` is the lookup key for the matching entry in the parent
/// struct's `referenced_structs` or `referenced_unions` slot —
/// `kind` picks the slot, `(enclosing_record, field_path)` picks the
/// entry within it.
///
/// `field_path` uses synthetic `<anonymous_N>` names for any
/// intermediate anonymous fields along the way. For OVERLAPPED's nested
/// anonymous struct, the path is `["<anonymous_0>", "<anonymous_0>"]`
/// — the outer 0 indexes the union field within `_OVERLAPPED`, the
/// inner 0 indexes the struct field within that union.
#[derive(Debug, Clone, Serialize)]
pub struct AnonRef {
    pub kind: RecordKind,
    pub enclosing_record: String,
    pub field_path: Vec<String>,
}

impl AnonRef {
    /// Stable composite identity string. Used as a `HashSet` key when
    /// deduplicating anonymous records during nested-record extraction.
    #[must_use]
    pub fn identity(&self) -> String {
        format!("{}::{}", self.enclosing_record, self.field_path.join("::"))
    }
}
