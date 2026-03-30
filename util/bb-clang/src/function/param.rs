//! Parameter declaration representation.

use bb_arch::{Arch, ParamLocation};
use clang::{Entity, EntityKind, Type};
use serde::Serialize;

use super::callconv::CallConv;
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

/* ─────────────────────────────── Conversions ────────────────────────────── */

impl<'a> TryFrom<Entity<'a>> for Param<'a> {
    type Error = ParamError;

    fn try_from(entity: Entity<'a>) -> Result<Self, Self::Error> {
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

        // Compute ABI location from context: arch from TU, calling convention
        // and positional index from the parent function declaration.
        let abi_location = compute_abi_location(&entity, &semantic_parent)?;

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

/* ──────────────────────── ABI location from context ────────────────────── */

/// Derive the ABI location for a `ParmDecl` by inspecting its parent
/// function's type (for calling convention and sibling param types)
/// and the translation unit's target (for architecture).
fn compute_abi_location(
    entity: &Entity<'_>,
    parent: &Entity<'_>,
) -> Result<ParamLocation, ParamError> {
    // Architecture from the translation unit.
    let target = entity.get_translation_unit().get_target();
    let arch = Arch::from_triple(&target.triple).map_err(|_| ParamError::NoAbiLocation)?;

    // Calling convention from the parent function's type.
    let parent_type = parent.get_type().ok_or(ParamError::NoAbiLocation)?;
    let callconv = CallConv::try_from(&parent_type).map_err(|_| ParamError::NoAbiLocation)?;

    // Collect all sibling ParmDecl types to determine positional assignment,
    // and find our own index among them.
    let siblings: Vec<Entity<'_>> = parent
        .get_children()
        .into_iter()
        .filter(|e| matches!(e.get_kind(), EntityKind::ParmDecl))
        .collect();

    let sibling_types: Vec<clang::Type<'_>> =
        siblings.iter().filter_map(|e| e.get_type()).collect();

    let all_locations = callconv.assign_params(arch, &sibling_types);

    // Find our index among the siblings.
    let index = siblings
        .iter()
        .position(|e| e == entity)
        .ok_or(ParamError::NoAbiLocation)?;

    all_locations
        .into_iter()
        .nth(index)
        .ok_or(ParamError::NoAbiLocation)
}
