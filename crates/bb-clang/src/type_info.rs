//! Shared type metadata extracted from a [`clang::Type`].
//!
//! [`TypeInfo`] is the single source of truth for type classification
//! (pointer, array, const, underlying type) used by both [`Field`](crate::Field)
//! and [`Param`](crate::Param).

use clang::{Type, TypeKind};
use serde::Serialize;

use crate::ext::UnderlyingType;

/* ─────────────────────────────── Helpers ──────────────────────────────── */

/// Count pointer indirection depth. `int**` = 2, `int*` = 1, `int` = 0.
fn count_pointer_depth(canonical: &Type) -> usize {
    let mut depth = 0;
    let mut t = *canonical;
    while let Some(pointee) = t.get_pointee_type() {
        depth += 1;
        t = pointee.get_canonical_type();
    }
    depth
}

/// Check if the canonical type is a function pointer (pointer to FunctionProto/FunctionNoProto).
fn is_func_ptr(canonical: &Type) -> bool {
    canonical.get_pointee_type().is_some_and(|pointee| {
        matches!(
            pointee.get_canonical_type().get_kind(),
            TypeKind::FunctionPrototype | TypeKind::FunctionNoPrototype
        )
    })
}

/// Serde helper: skip serializing when value is zero.
const fn is_zero(v: &usize) -> bool {
    *v == 0
}

/* ────────────────────────────────── Type ───────────────────────────────── */

/// Extracted type metadata from a [`clang::Type`].
///
/// Holds both the raw clang type (for further introspection) and the
/// serializable properties that describe the type's nature.
#[derive(Debug, Serialize)]
pub struct TypeInfo<'a> {
    /// The raw clang type. Available for further introspection but
    /// skipped during serialization.
    #[serde(skip)]
    type_: Type<'a>,
    /// The resolved underlying type name after stripping pointers and arrays.
    /// Only present when it differs from the display name.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub underlying_type: Option<String>,
    pub is_const: bool,
    pub is_volatile: bool,
    pub is_restrict: bool,
    pub is_pointer: bool,
    /// How many levels of pointer indirection (e.g. `PVOID**` = 2, `HANDLE` = 1 if ptr typedef, plain `DWORD` = 0).
    #[serde(skip_serializing_if = "is_zero")]
    pub pointer_depth: usize,
    pub is_function_pointer: bool,
    pub is_array: bool,
    /// The number of elements in a fixed-size array, if applicable.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub array_size: Option<usize>,
}

impl<'a> From<Type<'a>> for TypeInfo<'a> {
    /// Extract type metadata from a clang type.
    ///
    /// The `underlying_type` field is always populated when a declaration
    /// name is available. Use [`suppress_underlying_if_matches`](Self::suppress_underlying_if_matches)
    /// to clear it when it matches the display name.
    fn from(type_: Type<'a>) -> Self {
        let canonical = type_.get_canonical_type();
        let is_const = type_.is_const_qualified();
        let is_volatile = type_.is_volatile_qualified();
        let is_restrict = type_.is_restrict_qualified();
        let is_pointer = canonical.get_pointee_type().is_some();
        let pointer_depth = count_pointer_depth(&canonical);
        let is_function_pointer = is_func_ptr(&canonical);
        let is_array = matches!(
            canonical.get_kind(),
            TypeKind::ConstantArray | TypeKind::IncompleteArray | TypeKind::VariableArray
        );
        let array_size = if is_array { canonical.get_size() } else { None };

        let underlying = type_.get_underlying_type();
        let underlying_type = underlying.get_declaration().and_then(|d| d.get_name());

        Self {
            type_,
            underlying_type,
            is_const,
            is_volatile,
            is_restrict,
            is_pointer,
            pointer_depth,
            is_function_pointer,
            is_array,
            array_size,
        }
    }
}

impl<'a> TypeInfo<'a> {
    /// Clear `underlying_type` if it matches the given display name.
    ///
    /// Used by [`Field`](crate::Field) and [`Param`](crate::Param) to avoid
    /// redundant output when the underlying type is the same as the display type.
    pub fn suppress_underlying_if_matches(&mut self, display_name: Option<&str>) {
        if let Some(ref u) = self.underlying_type {
            if display_name.is_some_and(|d| d == u) {
                self.underlying_type = None;
            }
        }
    }

    /// The raw clang type.
    #[must_use]
    pub const fn get_type(&self) -> &Type<'a> {
        &self.type_
    }

    /// The canonical (fully resolved typedef) form of this type.
    #[must_use]
    pub fn get_canonical_type(&self) -> Type<'a> {
        self.type_.get_canonical_type()
    }

    /// The underlying type after resolving pointers and arrays.
    #[must_use]
    pub fn get_underlying_type(&self) -> Type<'a> {
        self.type_.get_underlying_type()
    }
}
