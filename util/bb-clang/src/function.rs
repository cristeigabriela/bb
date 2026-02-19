//! Function declaration representation.

use crate::{SourceLocation, callconv::CallConv, error::FunctionError};
use clang::{Entity, EntityKind, Type};
use serde::Serialize;

/* ────────────────────────────────── Types ───────────────────────────────── */

#[derive(Debug, Serialize)]
pub struct Function<'a> {
    #[serde(skip)]
    entity: Entity<'a>,
    name: String,
    #[serde(skip)]
    type_: Type<'a>,
    calling_convention: CallConv,
    location: Option<SourceLocation>,
}

/* ─────────────────────────────── Conversions ────────────────────────────── */

impl<'a> TryFrom<Entity<'a>> for Function<'a> {
    type Error = FunctionError;

    fn try_from(entity: Entity<'a>) -> Result<Self, Self::Error> {
        let kind = entity.get_kind();
        if !matches!(kind, EntityKind::FunctionDecl) {
            return Err(FunctionError::NotFunction(kind));
        }
        todo!()

        //Ok(Self{
        //    entity
        //})
    }
}
