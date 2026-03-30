//! Function declaration representation with ABI-aware parameter locations.
//!
//! Automatically detects the target architecture from the translation unit
//! and assigns each parameter its ABI location (register or stack offset)
//! based on the calling convention.

mod callconv;
mod param;

pub use callconv::CallConv;
pub use param::Param;

use bb_arch::{Arch, ReturnLocation};
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
    arch: Arch,
    calling_convention: CallConv,
    return_location: ReturnLocation,
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
    pub const fn get_type(&self) -> &Type<'a> {
        &self.type_
    }
    #[must_use]
    pub const fn get_return_type(&self) -> &Type<'a> {
        &self.return_type
    }
    #[must_use]
    pub fn get_return_type_name(&self) -> &str {
        &self.return_type_name
    }
    #[must_use]
    pub const fn is_dllimport(&self) -> bool {
        self.is_dllimport
    }
    #[must_use]
    pub const fn get_arch(&self) -> Arch {
        self.arch
    }
    #[must_use]
    pub const fn get_calling_convention(&self) -> &CallConv {
        &self.calling_convention
    }
    #[must_use]
    pub const fn get_return_location(&self) -> &ReturnLocation {
        &self.return_location
    }
    #[must_use]
    pub fn get_params(&self) -> &[Param<'a>] {
        &self.params
    }
    #[must_use]
    pub const fn has_body(&self) -> bool {
        self.has_body
    }
    #[must_use]
    pub const fn get_location(&self) -> Option<&SourceLocation> {
        self.location.as_ref()
    }

    /// Render a detailed ABI breakdown.
    #[must_use]
    pub fn display_detail(&self) -> String {
        crate::display::render_function_detail(self)
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

        // Detect architecture from the translation unit's target triple.
        let target = entity.get_translation_unit().get_target();
        let arch = Arch::from_triple(&target.triple)
            .map_err(|e| FunctionError::UnknownArch(e.0))?;

        let mut is_dllimport: bool = false;
        let mut has_body: bool = false;
        let mut params: Vec<Param<'a>> = Vec::new();

        for entry in entity.get_children() {
            match entry.get_kind() {
                EntityKind::DllImport => is_dllimport = true,
                EntityKind::CompoundStmt => has_body = true,
                EntityKind::ParmDecl => {
                    params.push(Param::try_from(entry)?);
                }
                _ => {}
            }
        }

        let name = entity.get_name().ok_or(FunctionError::NoName)?;
        let type_ = entity.get_type().ok_or(FunctionError::NoType)?;
        let return_type = type_
            .get_result_type()
            .ok_or(FunctionError::NoReturnType)?;
        let return_type_name = return_type.get_display_name();
        let calling_convention = CallConv::try_from(&type_)?;
        let location = SourceLocation::from_entity(&entity);

        // Compute return location.
        let return_location = calling_convention.return_location(arch, &return_type);

        Ok(Self {
            entity,
            name,
            type_,
            return_type,
            return_type_name,
            is_dllimport,
            arch,
            calling_convention,
            return_location,
            params,
            has_body,
            location,
        })
    }
}
