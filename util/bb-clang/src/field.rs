//! Field type representation.

use crate::error::ParseError;
use crate::struct_::Struct;
use crate::traits::{AnonymousType, UnderlyingType};
use clang::{Entity, EntityKind, Type};
use serde::Serialize;

#[derive(Debug, Serialize)]
pub struct Field<'a> {
    #[serde(skip)]
    entity: Entity<'a>,
    #[serde(skip)]
    #[allow(unused)]
    semantic_parent: Entity<'a>,
    name: String,
    #[serde(skip)]
    type_: Type<'a>,
    #[serde(rename = "type")]
    type_name: Option<String>,
    offset: usize,
    #[serde(rename = "offset_bytes")]
    offset_bytes: usize,
    size: usize,
    alignment: usize,
}

impl<'a> Field<'a> {
    #[must_use]
    pub const fn get_entity(&self) -> &Entity<'a> {
        &self.entity
    }
    #[allow(unused)]
    #[must_use]
    pub const fn get_semantic_parent(&self) -> &Entity<'a> {
        &self.semantic_parent
    }
    #[must_use]
    pub fn get_name(&self) -> &str {
        &self.name
    }
    #[must_use]
    pub const fn get_type(&self) -> &Type<'a> {
        &self.type_
    }
    #[must_use]
    pub const fn get_type_name(&self) -> &Option<String> {
        &self.type_name
    }
    #[must_use]
    pub fn get_canonical_type(&self) -> Type<'a> {
        self.type_.get_canonical_type()
    }
    #[must_use]
    pub const fn get_offset(&self) -> usize {
        self.offset
    }
    #[must_use]
    pub const fn get_offset_bytes(&self) -> usize {
        self.offset / 8
    }
    #[must_use]
    pub const fn get_size(&self) -> usize {
        self.size
    }
    #[must_use]
    pub const fn get_alignment(&self) -> usize {
        self.alignment
    }

    /// Returns the underlying type of this field, resolving pointers and arrays.
    ///
    /// For pointer types like `PLIST_ENTRY`, this returns the pointee type (`LIST_ENTRY`).
    /// For array types, this returns the element type. Otherwise returns the canonical type.
    #[must_use]
    pub fn get_underlying_type(&self) -> Type<'a> {
        self.get_type().get_underlying_type()
    }

    /// Returns true if this field's underlying type has child fields that can be expanded.
    #[must_use]
    pub fn has_children(&self) -> bool {
        Some(self.get_underlying_type())
            .and_then(|t| t.get_fields())
            .is_some_and(|fields| !fields.is_empty())
    }

    #[must_use]
    pub fn get_child_fields(&self) -> Vec<Self> {
        Some(self.get_underlying_type())
            .and_then(|t| t.get_declaration())
            .map(|decl| collect_fields(&decl))
            .unwrap_or_default()
    }

    #[must_use]
    pub fn get_child_struct(&self) -> Option<Struct<'a>> {
        let underlying = self.get_underlying_type();
        let decl = underlying.get_declaration()?;
        Struct::try_from(decl).ok()
    }
}

/// Generate [`Field`] from child-parent reference tuple, where the child is a field declaration.
impl<'a> TryFrom<(Entity<'a>, &Entity<'a>)> for Field<'a> {
    type Error = ParseError;

    fn try_from((entity, parent): (Entity<'a>, &Entity<'a>)) -> Result<Self, Self::Error> {
        if entity.get_kind() != EntityKind::FieldDecl {
            return Err(ParseError::NotFieldDecl);
        }

        let type_ = entity.get_type().ok_or(ParseError::NoType)?;
        let name = entity.get_name().ok_or(ParseError::NoName)?;
        let semantic_parent = entity.get_semantic_parent().ok_or(ParseError::NoType)?;
        let anonymous_type = type_.is_anonymous().unwrap_or(false);
        let type_name = (!anonymous_type).then(|| type_.get_display_name());

        let parent_type = parent.get_type().ok_or(ParseError::NoType)?;
        let offset = parent_type
            .get_offsetof(&name)
            .map_err(|_| ParseError::NoOffset)?;
        let size = type_.get_sizeof().map_err(|_| ParseError::NoSize)?;
        let alignment = type_.get_alignof().map_err(|_| ParseError::NoAlignment)?;

        Ok(Self {
            entity,
            semantic_parent,
            name,
            type_,
            type_name,
            offset,
            offset_bytes: offset / 8,
            size,
            alignment,
        })
    }
}

/// Collects all field declarations from a struct/class entity.
pub fn collect_fields<'a>(entity: &Entity<'a>) -> Vec<Field<'a>> {
    use clang::EntityVisitResult;

    let mut fields = Vec::new();
    entity.visit_children(|child, _| {
        if child.get_kind() == EntityKind::FieldDecl {
            if let Ok(field) = Field::try_from((child, entity)) {
                fields.push(field);
            }
        }
        EntityVisitResult::Continue
    });
    fields
}
