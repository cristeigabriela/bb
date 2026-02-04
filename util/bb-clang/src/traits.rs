//! Expansions to clang library, as I believe are suited for this project.

use clang::{EntityKind, Type};

/// Check if the declaration of the canonical type in question is anonymous.
///
/// In the case of an anonymous declaration, [`Type::get_display_name`] will return
/// a string similar to `struct (unnamed ...`.
///
/// For similar usage, see: <https://github.com/rust-lang/rust-bindgen/issues/2488>
pub trait AnonymousType {
    fn is_anonymous(&self) -> Option<bool>;
}

impl AnonymousType for Type<'_> {
    fn is_anonymous(&self) -> Option<bool> {
        self.get_canonical_type()
            .get_declaration()
            .map(|x| x.is_anonymous())
    }
}

/// Get the name of a canonical type's declaration's kind.
pub trait DeclarationKind {
    fn get_declaration_kind_name(&self) -> Option<String>;
}

impl DeclarationKind for Type<'_> {
    /// Implements:
    ///
    /// - `class`
    /// - `struct`
    /// - `union`
    ///
    /// These are really the only ones needed for viewing Windows types, given
    /// this is only really used in combination with [`AnonymousType`].
    fn get_declaration_kind_name(&self) -> Option<String> {
        let kind = self.get_canonical_type().get_declaration()?.get_kind();
        match kind {
            EntityKind::StructDecl => Some("struct".into()),
            EntityKind::ClassDecl => Some("class".into()),
            EntityKind::UnionDecl => Some("union".into()),
            _ => None,
        }
    }
}

/// Get the underlying type of a canonical type.
pub trait UnderlyingType {
    fn get_underlying_type(&self) -> Self;
}

impl UnderlyingType for Type<'_> {
    /// Resolves the underlying type for pointers and arrays.
    ///
    /// For pointer types (e.g., `PLIST_ENTRY`), returns the pointee's canonical type.
    /// For array types, returns the element's canonical type.
    /// Otherwise, returns the canonical type unchanged.
    ///
    /// This is used to "see through" pointer indirection when expanding nested types,
    /// so that `PLIST_ENTRY` (pointer to `LIST_ENTRY`) expands `LIST_ENTRY`'s fields.
    fn get_underlying_type(&self) -> Self {
        let canonical_type = self.get_canonical_type();
        if let Some(underlying_type) = canonical_type.get_pointee_type() {
            underlying_type.get_canonical_type()
        } else if let Some(underlying_type) = canonical_type.get_element_type() {
            underlying_type.get_canonical_type()
        } else {
            canonical_type
        }
    }
}
