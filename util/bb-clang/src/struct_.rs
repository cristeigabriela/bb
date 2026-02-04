//! Struct type representation.

use crate::display;
use crate::error::ParseError;
use crate::field::{Field, collect_fields};
use crate::traits::{AnonymousType, DeclarationKind};
use clang::{Entity, EntityKind};
use serde::Serialize;
use std::collections::HashSet;

/// Source location information (file, line, column).
#[derive(Debug, Clone, Serialize)]
pub struct SourceLocation {
    pub file: Option<String>,
    pub line: u32,
    pub column: u32,
}

impl SourceLocation {
    /// Format as "<file:line:col>" or "line:col" if no file.
    #[must_use]
    pub fn display_short(&self) -> String {
        match &self.file {
            Some(f) => format!("{}:{}:{}", f, self.line, self.column),
            None => format!("{}:{}", self.line, self.column),
        }
    }
}

#[derive(Debug, Serialize)]
pub struct Struct<'a> {
    #[serde(skip)]
    entity: Entity<'a>,
    name: String,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    is_anonymous: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
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
    ///
    /// See [`display::render_struct`] for full documentation on cycle detection strategy.
    #[must_use]
    pub fn display(&self, depth: usize, field_filter: Option<&str>) -> String {
        display::render_struct(
            &self.name,
            self.location.as_ref(),
            self.size,
            &self.fields,
            depth,
            field_filter,
        )
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
    /// * `result` - Accumulator for collected structs
    /// * `seen` - Set of type names already processed (for global deduplication)
    /// * `max_depth` - Maximum recursion depth
    /// * `current_depth` - Current recursion level
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

/// Generate [`Struct`] from entity that is either a class or struct declaration.
impl<'a> TryFrom<Entity<'a>> for Struct<'a> {
    type Error = ParseError;

    fn try_from(entity: Entity<'a>) -> Result<Self, Self::Error> {
        let kind = entity.get_kind();
        if !matches!(kind, EntityKind::ClassDecl | EntityKind::StructDecl) {
            return Err(ParseError::NotStructOrClass);
        }

        // Extract location info
        let location = entity.get_location().map(|loc| {
            let file_loc = loc.get_file_location();
            SourceLocation {
                file: file_loc
                    .file
                    .map(|f| f.get_path())
                    .and_then(|p| p.file_name().map(|n| n.to_string_lossy().into_owned())),
                line: file_loc.line,
                column: file_loc.column,
            }
        });

        // Handle anonymous structures
        let is_anonymous = entity
            .get_type()
            .and_then(|t| t.is_anonymous())
            .unwrap_or(false);

        let name = if is_anonymous {
            let kind_str = entity
                .get_type()
                .and_then(|t| t.get_declaration_kind_name())
                .unwrap_or_else(|| "type".into());
            format!("<anonymous {kind_str}>")
        } else {
            entity.get_name().ok_or(ParseError::NoName)?
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
