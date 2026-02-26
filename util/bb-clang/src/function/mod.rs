//! Function declaration representation.

mod callconv;
mod param;

pub use param::Param;

use callconv::CallConv;
use clang::{Entity, EntityKind, Type};
use serde::Serialize;

use crate::{SourceLocation, error::FunctionError};

/* ────────────────────────────────── Types ───────────────────────────────── */

#[derive(Debug, Serialize)]
pub struct Function<'a> {
    #[serde(skip)]
    entity: Entity<'a>,
    name: String,
    #[serde(skip)]
    type_: Type<'a>,
    #[serde(skip)]
    return_type: Type<'a>,
    #[serde(rename = "return_type")]
    return_type_name: String,
    is_dllimport: bool,
    calling_convention: CallConv,
    params: Vec<Param<'a>>,
    has_body: bool,
    location: Option<SourceLocation>,
}

impl<'a> Function<'a> {
    #[must_use]
    pub const fn get_entity(&self) -> &Entity<'a> {
        &self.entity
    }
    #[must_use]
    pub fn get_name(&self) -> &str {
        &self.name
    }
    #[must_use]
    pub fn get_type(&self) -> &Type<'a> {
        &self.type_
    }
    #[must_use]
    pub fn get_return_type(&self) -> &Type<'a> {
        &self.return_type
    }
    #[must_use]
    pub fn get_return_type_name(&self) -> &str {
        &self.return_type_name
    }
    #[must_use]
    pub fn is_dllimport(&self) -> bool {
        self.is_dllimport
    }
    pub fn get_calling_convention(&self) -> &CallConv {
        &self.calling_convention
    }
    #[must_use]
    pub fn get_params(&self) -> &[Param<'a>] {
        &self.params
    }
    #[must_use]
    pub fn has_body(&self) -> bool {
        self.has_body
    }
    #[must_use]
    pub const fn get_location(&self) -> Option<&SourceLocation> {
        self.location.as_ref()
    }
}

/* ─────────────────────────────── Conversions ────────────────────────────── */

impl<'a> TryFrom<Entity<'a>> for Function<'a> {
    type Error = FunctionError;

    fn try_from(entity: Entity<'a>) -> Result<Self, Self::Error> {
        let kind = entity.get_kind();
        if !matches!(kind, EntityKind::FunctionDecl) {
            return Err(FunctionError::NotFunction(kind));
        }

        let mut return_type: Option<Type<'a>> = None;
        let mut params: Vec<Param<'a>> = Vec::new();
        let mut is_dllimport: bool = false;
        let mut has_body: bool = false;

        for entry in entity.get_children() {
            match entry.get_kind() {
                EntityKind::DllImport => is_dllimport = true,
                EntityKind::CompoundStmt => has_body = true,
                EntityKind::TypeRef if return_type.is_none() => {
                    return_type = entry.get_type();
                }
                EntityKind::ParmDecl => {
                    params.push(Param::try_from(entry)?);
                }
                _ => {}
            }
        }

        let name = entity.get_name().ok_or(FunctionError::NoName)?;
        let type_ = entity.get_type().ok_or(FunctionError::NoType)?;
        let return_type = return_type.ok_or(FunctionError::NoReturnType)?;
        let return_type_name = return_type.get_display_name();
        let calling_convention = CallConv::from(
            type_
                .get_calling_convention()
                .ok_or(FunctionError::NoCallingConvention)?,
        );
        let location = SourceLocation::from_entity(&entity);

        Ok(Self {
            entity,
            name,
            type_,
            return_type,
            return_type_name,
            is_dllimport,
            calling_convention,
            params,
            has_body,
            location,
        })
    }
}
