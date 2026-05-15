//! Shared type metadata extracted from a [`clang::Type`].
//!
//! [`TypeInfo`] is the single source of truth for type classification
//! (pointer, array, const, terminal primitive, underlying record) used by
//! [`Field`](crate::Field), [`Param`](crate::Param), and
//! [`Typedef`](crate::Typedef). The data-only subset is exposed as
//! [`TypeProperties`], which can be embedded and serialized into any
//! type-shaped JSON object via `#[serde(flatten)]` — that's how all three
//! parent types stay shape-compatible for API consumers.

use clang::{Type, TypeKind};
use serde::Serialize;

use crate::ext::UnderlyingType;

/* ────────────────────────────────── Types ───────────────────────────────── */

/// Serializable type metadata. Carries the same information for any type
/// in any context — fields, params, typedefs — so API consumers always
/// see the same shape.
///
/// All boolean flags describe the type *with qualifiers and pointers*,
/// not the canonical leaf. `underlying_type` is the **terminal primitive**
/// at the bottom of the canonical chain (e.g. `void`, `int`, `char`).
/// `underlying_record` is the **record/enum declaration name** after
/// stripping one level of pointer/array indirection (e.g. for
/// `LIST_ENTRY *`, this is `_LIST_ENTRY`). The two are mutually exclusive
/// in practice: pointer/primitive typedefs set the former, record
/// typedefs the latter.
#[derive(Debug, Clone, Default, Serialize)]
#[allow(clippy::struct_excessive_bools)] // Each bool represents a distinct type property.
pub struct TypeProperties {
    pub is_const: bool,
    pub is_volatile: bool,
    pub is_restrict: bool,
    pub is_pointer: bool,
    /// How many levels of pointer indirection (e.g. `PVOID**` = 2,
    /// `HANDLE` = 1, plain `DWORD` = 0). Always emitted in JSON so
    /// consumers can pair it with `is_pointer` without an
    /// "absent = 0" special case.
    pub pointer_depth: usize,
    pub is_function_pointer: bool,
    pub is_array: bool,
    /// The number of elements in a fixed-size array, if applicable.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub array_size: Option<usize>,
    /// Terminal primitive at the bottom of the canonical chain.
    /// e.g. `void`, `int`, `char`, `unsigned long`, `bool`. `None` when
    /// the chain bottoms at a record (struct/union/enum) — consult
    /// `underlying_record` for that case.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub underlying_type: Option<String>,
    /// Record/enum declaration name after stripping one level of pointer
    /// or array indirection. e.g. for `LIST_ENTRY *`, this is
    /// `_LIST_ENTRY`. `None` when there's no named record at the bottom
    /// of the chain (the type bottoms at a primitive or anonymous).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub underlying_record: Option<String>,
}

impl TypeProperties {
    /// Compute properties from a clang [`Type`].
    ///
    /// Walks the canonical form to determine pointer/array/function-pointer
    /// classification, then walks **all** the way down (through every
    /// pointer/array layer) to find the terminal primitive when there is
    /// one. The first-step pointee is consulted separately for the
    /// `underlying_record` field.
    #[must_use]
    pub fn from_type(type_: &Type<'_>) -> Self {
        let canonical = type_.get_canonical_type();
        let is_const = type_.is_const_qualified();
        let is_volatile = type_.is_volatile_qualified();
        let is_restrict = type_.is_restrict_qualified();
        let is_pointer = canonical.get_pointee_type().is_some();
        let pointer_depth = count_pointer_depth(&canonical);
        let is_function_pointer = is_func_ptr(&canonical);
        let is_array = is_array_kind(canonical.get_kind());
        let array_size = if is_array { canonical.get_size() } else { None };

        // Old semantics — useful for "what struct does this pointer
        // ultimately point at?". Strip one level of pointer/array.
        let after_one_strip = type_.get_underlying_type();
        let underlying_record = after_one_strip.get_declaration().and_then(|d| d.get_name());

        // Walk all the way to the terminal scalar leaf. Returns None when
        // the chain bottoms at a record (or anonymous / unhandled kind).
        let underlying_type = terminal_primitive_name(&canonical);

        Self {
            is_const,
            is_volatile,
            is_restrict,
            is_pointer,
            pointer_depth,
            is_function_pointer,
            is_array,
            array_size,
            underlying_type,
            underlying_record,
        }
    }

    /// Clear `underlying_record` and `underlying_type` if either matches
    /// the given display name.
    ///
    /// Used by [`Field`](crate::Field) and [`Param`](crate::Param) to
    /// avoid redundant output when the rendered type name already says
    /// `_LIST_ENTRY` and the record name would just repeat it, or when
    /// the displayed type is itself the terminal primitive (e.g. a
    /// plain `int` field shouldn't also emit `"underlying_type": "int"`
    /// in JSON).
    pub fn suppress_underlying_record_if_matches(&mut self, display_name: Option<&str>) {
        if let Some(d) = display_name {
            if self.underlying_record.as_deref() == Some(d) {
                self.underlying_record = None;
            }
            if self.underlying_type.as_deref() == Some(d) {
                self.underlying_type = None;
            }
        }
    }
}

/// Extracted type metadata from a [`clang::Type`], with the raw type
/// preserved for further introspection.
///
/// Embeds [`TypeProperties`] via `#[serde(flatten)]` so the serialized
/// shape is identical to that of [`Typedef`](crate::Typedef). Field /
/// Param consumers and Typedef consumers see the same metadata vocabulary.
#[derive(Debug, Serialize)]
pub struct TypeInfo<'a> {
    /// The raw clang type. Available for further introspection but
    /// skipped during serialization.
    #[serde(skip)]
    type_: Type<'a>,
    #[serde(flatten)]
    pub properties: TypeProperties,
}

impl<'a> TypeInfo<'a> {
    /// Clear `underlying_record` if it matches the given display name.
    ///
    /// Delegates to [`TypeProperties::suppress_underlying_record_if_matches`].
    /// Kept on `TypeInfo` for source-compatibility with prior callers.
    pub fn suppress_underlying_if_matches(&mut self, display_name: Option<&str>) {
        self.properties
            .suppress_underlying_record_if_matches(display_name);
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

impl<'a> std::ops::Deref for TypeInfo<'a> {
    type Target = TypeProperties;
    fn deref(&self) -> &TypeProperties {
        &self.properties
    }
}

/* ─────────────────────────────── Conversions ────────────────────────────── */

impl<'a> From<Type<'a>> for TypeInfo<'a> {
    fn from(type_: Type<'a>) -> Self {
        let properties = TypeProperties::from_type(&type_);
        Self { type_, properties }
    }
}

/* ───────────────────────────────── Helpers ──────────────────────────────── */

/// Count pointer indirection depth.
///
/// - `int**` = 2
/// - `int*` = 1
/// - `int` = 0
fn count_pointer_depth(canonical: &Type) -> usize {
    let mut depth = 0;
    let mut t = *canonical;
    while let Some(pointee) = t.get_pointee_type() {
        depth += 1;
        t = pointee.get_canonical_type();
    }
    depth
}

/// Check if the canonical type is a function pointer (pointer to
/// [`TypeKind::FunctionPrototype`] or [`TypeKind::FunctionNoPrototype`]).
fn is_func_ptr(canonical: &Type) -> bool {
    canonical.get_pointee_type().is_some_and(|pointee| {
        matches!(
            pointee.get_canonical_type().get_kind(),
            TypeKind::FunctionPrototype | TypeKind::FunctionNoPrototype
        )
    })
}

/// Walk every pointer / array / typedef layer and return the canonical
/// display name of the terminal scalar if the leaf is a builtin.
///
/// Returns `None` when the leaf is a record/enum (use `underlying_record`
/// instead) or a function type (use `is_function_pointer`).
fn terminal_primitive_name(canonical: &Type<'_>) -> Option<String> {
    // Cap detects runaway recursion on malformed types. Real chains
    // are very shallow (≤ ~3 pointer/array layers); 64 is generous.
    // Truncation emits an `eprintln!` so it doesn't silently produce
    // wrong metadata.
    const MAX_DEPTH: usize = 64;
    let mut current = canonical.get_canonical_type();
    let mut depth = 0_usize;
    loop {
        if let Some(pointee) = current.get_pointee_type() {
            current = pointee.get_canonical_type();
        } else if let Some(element) = current.get_element_type() {
            current = element.get_canonical_type();
        } else {
            break;
        }
        depth += 1;
        if depth >= MAX_DEPTH {
            eprintln!(
                "bb-clang: terminal_primitive_name walked {MAX_DEPTH}+ pointer/array \
                 layers without bottoming out — abandoning"
            );
            return None;
        }
    }
    if is_primitive_kind(current.get_kind()) {
        Some(current.get_display_name())
    } else {
        None
    }
}

/// Whether a [`TypeKind`] is a builtin scalar — single source of truth
/// for "what counts as a primitive" across the crate.
///
/// Used by `TypeProperties::from_type` to decide whether the terminal
/// leaf of a canonical chain qualifies for the `underlying_type` slot,
/// and by [`crate::typedef::TypedefIndex`] to classify a typedef whose
/// canonical type is one of these. Centralising it here means future
/// additions (new clang `TypeKind`s, exotic scalars) only need to
/// change one place.
#[must_use]
pub(crate) const fn is_primitive_kind(k: TypeKind) -> bool {
    matches!(
        k,
        TypeKind::Void
            | TypeKind::Bool
            | TypeKind::CharS
            | TypeKind::CharU
            | TypeKind::SChar
            | TypeKind::UChar
            | TypeKind::WChar
            | TypeKind::Char16
            | TypeKind::Char32
            | TypeKind::Short
            | TypeKind::UShort
            | TypeKind::Int
            | TypeKind::UInt
            | TypeKind::Long
            | TypeKind::ULong
            | TypeKind::LongLong
            | TypeKind::ULongLong
            | TypeKind::Int128
            | TypeKind::UInt128
            | TypeKind::Half
            | TypeKind::Float
            | TypeKind::Double
            | TypeKind::LongDouble
            | TypeKind::Float128
            | TypeKind::Nullptr
    )
}

/// Whether a [`TypeKind`] is one of the three array flavors clang
/// surfaces. Same centralisation rationale as [`is_primitive_kind`].
#[must_use]
pub(crate) const fn is_array_kind(k: TypeKind) -> bool {
    matches!(
        k,
        TypeKind::ConstantArray | TypeKind::IncompleteArray | TypeKind::VariableArray
    )
}

/// Whether a [`Type`] is a function pointer (pointer-to-function).
///
/// Walks one pointee level — `int (*)(...)` is a pointer; the pointee
/// is the function prototype itself. Exposed at crate scope so the
/// typedef classifier can call it without re-implementing the dance.
#[must_use]
pub(crate) fn is_function_pointer(ty: &Type<'_>) -> bool {
    is_func_ptr(&ty.get_canonical_type())
}
