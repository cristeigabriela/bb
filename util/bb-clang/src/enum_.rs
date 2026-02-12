//! Enum type representation.

use clang::{Entity, EntityKind, Type};
use serde::Serialize;

use crate::constant::Constant;
use crate::display;
use crate::error::EnumError;
use crate::location::SourceLocation;
use crate::traits::AnonymousType;

/* ────────────────────────────────── Types ───────────────────────────────── */

#[derive(Debug, Serialize)]
pub struct Enum<'a> {
    #[serde(skip)]
    entity: Entity<'a>,
    name: String,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    is_anonymous: bool,
    #[serde(skip)]
    type_: Type<'a>,
    #[serde(rename = "type", skip_serializing_if = "Option::is_none")]
    type_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    location: Option<SourceLocation>,
    constants: Vec<Constant<'a>>,
}

impl<'a> Enum<'a> {
    #[must_use]
    pub const fn get_entity(&self) -> &Entity<'a> {
        &self.entity
    }

    #[must_use]
    pub fn get_name(&self) -> &str {
        &self.name
    }

    #[must_use]
    pub const fn is_anonymous(&self) -> bool {
        self.is_anonymous
    }

    #[must_use]
    pub const fn get_underlying_type(&self) -> &Type<'a> {
        &self.type_
    }

    #[must_use]
    pub fn get_type_name(&self) -> Option<&str> {
        self.type_name.as_deref()
    }

    #[must_use]
    pub const fn get_location(&self) -> Option<&SourceLocation> {
        self.location.as_ref()
    }

    #[must_use]
    pub fn get_constants(&self) -> &[Constant<'a>] {
        &self.constants
    }

    /// Render this enum with all its constants as a tree.
    #[must_use]
    pub fn display(&self) -> String {
        display::render_enum(self, None)
    }

    /// Render this enum showing only constants whose names match `pattern`.
    #[must_use]
    pub fn display_filtered(&self, pattern: &str, case_sensitive: bool) -> String {
        let filtered: Vec<_> = self
            .constants
            .iter()
            .filter(|c| bb_shared::glob_match(c.get_name(), pattern, case_sensitive))
            .cloned()
            .collect();

        if filtered.is_empty() {
            return String::new();
        }

        display::render_enum_constants(self, &filtered, None)
    }
}

/* ──────────────────────────────── Utilities ─────────────────────────────── */

/// Collects all [`EntityKind::EnumConstantDecl`] children as [`Constant`]s.
fn collect_enum_constants<'a>(entity: &Entity<'a>) -> Vec<Constant<'a>> {
    entity
        .get_children()
        .into_iter()
        .filter(|e| e.get_kind() == EntityKind::EnumConstantDecl)
        .filter_map(|e| Constant::try_from(e).ok())
        .collect()
}

/* ─────────────────────────────── Conversions ────────────────────────────── */

/// Generate a [`Enum`] from an entity that is an [`EntityKind::EnumDecl`].
impl<'a> TryFrom<Entity<'a>> for Enum<'a> {
    type Error = EnumError;

    fn try_from(entity: Entity<'a>) -> Result<Self, Self::Error> {
        let kind = entity.get_kind();
        if !matches!(kind, EntityKind::EnumDecl) {
            return Err(EnumError::NotEnum(kind));
        }

        let is_anonymous = entity
            .get_type()
            .and_then(|t| t.is_anonymous())
            .unwrap_or(false);

        let name = if is_anonymous {
            "<anonymous enum>".into()
        } else {
            entity.get_name().unwrap_or_else(|| "<unnamed enum>".into())
        };

        let type_ = entity.get_enum_underlying_type().ok_or(EnumError::NoType)?;
        let type_name = (!is_anonymous).then(|| type_.get_display_name());

        let constants = collect_enum_constants(&entity);

        let location = SourceLocation::from_entity(&entity);

        Ok(Self {
            entity,
            name,
            is_anonymous,
            type_,
            type_name,
            location,
            constants,
        })
    }
}
