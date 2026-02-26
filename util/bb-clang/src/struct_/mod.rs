//! Struct type representation.

mod field;

pub use field::Field;
use field::collect_fields;

use crate::clang_ext::{AnonymousType, DeclarationKind};
use crate::display;
use crate::error::StructError;
use crate::location::SourceLocation;
use clang::{Entity, EntityKind};
use serde::Serialize;
use std::collections::HashSet;

/* ────────────────────────────────── Types ───────────────────────────────── */

#[derive(Debug, Serialize)]
pub struct Struct<'a> {
    #[serde(skip)]
    entity: Entity<'a>,
    name: String,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    is_anonymous: bool,
    location: Option<SourceLocation>,
    size: Option<usize>,
    fields: Vec<Field<'a>>,
}

impl<'a> Struct<'a> {
    #[must_use]
    pub const fn get_entity(&self) -> &Entity<'a> {
        &self.entity
    }
    #[must_use]
    pub fn get_name(&self) -> &str {
        &self.name
    }
    #[must_use]
    pub fn get_fields(&self) -> &[Field<'a>] {
        &self.fields
    }
    #[must_use]
    pub const fn get_location(&self) -> Option<&SourceLocation> {
        self.location.as_ref()
    }
    #[must_use]
    pub const fn get_size(&self) -> Option<usize> {
        self.size
    }
    #[must_use]
    pub const fn is_anonymous(&self) -> bool {
        self.is_anonymous
    }

    /// Renders this struct in a `WinDbg` `dt`-style format with Unicode box-drawing.
    #[must_use]
    pub fn display(&self, depth: usize, field_filter: Option<&str>) -> String {
        display::render_struct(self, depth, field_filter)
    }

    /// Returns the names of expandable child types referenced by this struct's fields.
    ///
    /// Skips anonymous types (they have no meaningful name to look up).
    /// Each name appears at most once, in field order.
    #[must_use]
    pub fn referenced_type_names(&self) -> Vec<String> {
        let mut names = Vec::new();
        let mut seen = HashSet::new();
        for field in &self.fields {
            if !field.has_children() {
                continue;
            }
            let Some(decl) = field.get_underlying_type().get_declaration() else {
                continue;
            };
            let is_anonymous = decl
                .get_type()
                .and_then(|t| t.is_anonymous())
                .unwrap_or(false);
            if is_anonymous {
                continue;
            }
            if let Some(name) = decl.get_name() {
                if seen.insert(name.clone()) {
                    names.push(name);
                }
            }
        }
        names
    }

    /// Extracts all nested struct types up to the specified depth for JSON serialization.
    ///
    /// Unlike `display()`, this collects unique nested types into a flat list rather than
    /// rendering them inline. Each nested type appears only once in the result, regardless
    /// of how many fields reference it.
    ///
    /// # Cycle Detection
    ///
    /// Uses a [`HashSet<String>`] to track already-collected type names. Unlike `display()`,
    /// we do NOT remove names after processing because we want global deduplication
    /// (each type should appear exactly once in the output array).
    #[must_use]
    pub fn extract_nested_types(&self, depth: usize) -> Vec<Self> {
        let mut result = Vec::new();
        let mut seen = HashSet::new();
        seen.insert(self.name.clone());
        self.collect_nested(&mut result, &mut seen, depth, 0);
        result
    }

    /// Recursively collects nested struct types from fields.
    ///
    /// # Arguments
    ///
    /// * `result` - Accumulator for collected structs.
    /// * `seen` - Set of type names already processed (for global deduplication).
    /// * `max_depth` - Maximum recursion depth.
    /// * `current_depth` - Current recursion level.
    ///
    /// Note: Unlike `write_fields()`, we don't remove names from `seen` after processing
    /// because we want each type to appear only once in the final result.
    fn collect_nested(
        &self,
        result: &mut Vec<Self>,
        seen: &mut HashSet<String>,
        max_depth: usize,
        current_depth: usize,
    ) {
        if current_depth >= max_depth {
            return;
        }

        for field in &self.fields {
            if let Some(child_struct) = field.get_child_struct() {
                // Only process types we haven't seen before (global deduplication)
                if seen.insert(child_struct.name.clone()) {
                    // Recurse first to collect deeper types
                    child_struct.collect_nested(result, seen, max_depth, current_depth + 1);
                    result.push(child_struct);
                }
            }
        }
    }
}

/* ─────────────────────────────── Conversions ────────────────────────────── */

/// Generate [`Struct`] from entity that is either a [`EntityKind::ClassDecl`] or [`EntityKind::StructDecl`].
impl<'a> TryFrom<Entity<'a>> for Struct<'a> {
    type Error = StructError;

    fn try_from(entity: Entity<'a>) -> Result<Self, Self::Error> {
        let kind = entity.get_kind();
        if !matches!(kind, EntityKind::ClassDecl | EntityKind::StructDecl) {
            return Err(StructError::NotStructOrClass(kind));
        }

        let location = SourceLocation::from_entity(&entity);

        // Handle anonymous structures
        let is_anonymous = entity
            .get_type()
            .and_then(|t| t.is_anonymous())
            .unwrap_or(false);

        let name = if is_anonymous {
            let kind_str = entity
                .get_type()
                .and_then(|t| t.get_declaration_kind_name())
                .unwrap_or("type");
            format!("<anonymous {kind_str}>")
        } else {
            entity.get_name().ok_or(StructError::NoName)?
        };

        let fields = collect_fields(&entity);
        let size = entity.get_type().and_then(|t| t.get_sizeof().ok());

        Ok(Self {
            entity,
            name,
            is_anonymous,
            location,
            size,
            fields,
        })
    }
}
