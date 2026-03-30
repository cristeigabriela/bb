//! Parameter declaration representation.

use bb_arch::ParamLocation;
use clang::{Entity, EntityKind, Type};
use serde::Serialize;

use crate::{SourceLocation, clang_ext::UnderlyingType, error::ParamError};

/* ────────────────────────────────── Types ───────────────────────────────── */

#[derive(Debug, Serialize)]
pub struct Param<'a> {
    #[serde(skip)]
    entity: Entity<'a>,
    #[serde(skip)]
    #[allow(unused)]
    semantic_parent: Entity<'a>,
    name: Option<String>,
    #[serde(skip)]
    type_: Type<'a>,
    #[serde(rename = "type")]
    type_name: String,
    location: Option<SourceLocation>,
    abi_location: ParamLocation,
}

impl<'a> Param<'a> {
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
    pub fn get_name(&self) -> Option<&str> {
        self.name.as_deref()
    }
    #[must_use]
    pub const fn get_type(&self) -> &Type<'a> {
        &self.type_
    }
    #[must_use]
    pub fn get_type_name(&self) -> &str {
        &self.type_name
    }
    #[must_use]
    pub fn get_canonical_type(&self) -> Type<'a> {
        self.type_.get_canonical_type()
    }
    #[must_use]
    pub const fn get_location(&self) -> Option<&SourceLocation> {
        self.location.as_ref()
    }
    #[must_use]
    pub const fn get_abi_location(&self) -> &ParamLocation {
        &self.abi_location
    }

    /// Returns the underlying type of this field, resolving pointers and arrays.
    #[allow(unused)]
    #[must_use]
    pub fn get_underlying_type(&self) -> Type<'a> {
        self.get_type().get_underlying_type()
    }
}

/* ─────────────────────────────── Construction ──────────────────────────── */

impl<'a> Param<'a> {
    /// Construct a `Param` from an entity and its computed ABI location.
    pub fn new(entity: Entity<'a>, abi_location: ParamLocation) -> Result<Self, ParamError> {
        let kind = entity.get_kind();
        if !matches!(kind, EntityKind::ParmDecl) {
            return Err(ParamError::NotParam(kind));
        }

        let semantic_parent = entity
            .get_semantic_parent()
            .ok_or(ParamError::NoSemanticParent)?;
        let name = entity.get_name();
        let type_ = entity.get_type().ok_or(ParamError::NoType)?;
        let type_name = type_.get_display_name();
        let location = SourceLocation::from_entity(&entity);

        Ok(Self {
            entity,
            semantic_parent,
            name,
            type_,
            type_name,
            location,
            abi_location,
        })
    }
}
