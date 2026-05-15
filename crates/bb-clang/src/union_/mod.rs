//! Union type representation.
//!
//! Parallel to [`Struct`](crate::Struct) but accepts only
//! [`EntityKind::UnionDecl`]. Unions are NOT findable as top-level
//! entries via `bb-types -s ...` — they surface only via the
//! `referenced_unions` slot of a struct (or another union) that has
//! the union as a field type. Named unions (`_LARGE_INTEGER`) live
//! there alongside anonymous nested unions (`_OVERLAPPED::<anonymous_0>`).

use clang::{Entity, EntityKind};
use serde::Serialize;
use std::collections::HashSet;

use crate::error::UnionError;
use crate::ext::AnonymousType;
use crate::location::SourceLocation;
use crate::record::RecordKind;
use crate::struct_::{Field, Struct, collect_fields, collect_nested_from_fields};

/* ────────────────────────────────── Types ───────────────────────────────── */

#[derive(Debug, Serialize)]
pub struct Union<'a> {
    #[serde(skip)]
    entity: Entity<'a>,
    name: String,
    /// Always [`RecordKind::Union`]. Carried so JSON consumers can
    /// dispatch on `kind` uniformly across struct and union entries.
    kind: RecordKind,
    /// Typedef names that resolve to this union (e.g.
    /// `["LARGE_INTEGER"]` for `_LARGE_INTEGER`). Populated by callers
    /// that have a [`TypedefIndex`](crate::TypedefIndex). Mirrors
    /// [`Struct::get_aliases`](crate::Struct::get_aliases).
    #[serde(skip_serializing_if = "Vec::is_empty")]
    aliases: Vec<String>,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    is_anonymous: bool,
    /// Outermost named ancestor — `Some` only for anonymous nested
    /// unions surfaced in a parent's `referenced_unions` slot.
    #[serde(skip_serializing_if = "Option::is_none")]
    enclosing_record: Option<String>,
    /// Field-name chain from `enclosing_record` to this union. Empty
    /// for named top-level unions.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    field_path: Vec<String>,
    location: Option<SourceLocation>,
    size: Option<usize>,
    fields: Vec<Field<'a>>,
}

impl<'a> Union<'a> {
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
    #[must_use]
    pub const fn kind(&self) -> RecordKind {
        self.kind
    }
    #[must_use]
    pub fn get_enclosing_record(&self) -> Option<&str> {
        self.enclosing_record.as_deref()
    }
    #[must_use]
    pub fn get_field_path(&self) -> &[String] {
        &self.field_path
    }

    /// Typedef names that resolve to this union.
    #[must_use]
    pub fn get_aliases(&self) -> &[String] {
        &self.aliases
    }

    /// Attach typedef alias names. Mirrors
    /// [`Struct::with_aliases`](crate::Struct::with_aliases).
    #[must_use]
    pub fn with_aliases(mut self, aliases: Vec<String>) -> Self {
        self.aliases = aliases;
        self
    }

    /// Render this union in `WinDbg` `dt`-style format. Mirrors
    /// [`Struct::display`](crate::Struct::display).
    #[must_use]
    pub fn display(
        &self,
        depth: usize,
        field_filter: Option<&str>,
        typedef_index: Option<&crate::TypedefIndex>,
    ) -> String {
        crate::display::render_union(self, depth, field_filter, typedef_index)
    }

    /// Stable identity for dedup across a flat nested-record list.
    /// Mirrors [`Struct::identity`].
    #[must_use]
    pub fn identity(&self) -> String {
        if self.is_anonymous {
            format!(
                "{}::{}",
                self.enclosing_record.as_deref().unwrap_or(""),
                self.field_path.join("::")
            )
        } else {
            self.name.clone()
        }
    }

    /// Names of named record types (struct or union) referenced by
    /// this union's fields. Anonymous nested records are omitted.
    /// Mirrors [`Struct::referenced_type_names`].
    #[must_use]
    pub fn referenced_type_names(&self) -> Vec<String> {
        crate::struct_::referenced_named_record_names(&self.fields)
    }

    /// All nested record types reachable from this union's fields,
    /// up to `max_depth` levels. Same semantics as
    /// [`Struct::extract_nested_records`].
    #[must_use]
    pub fn extract_nested_records(&self, max_depth: usize) -> (Vec<Struct<'a>>, Vec<Self>) {
        let mut structs: Vec<Struct<'a>> = Vec::new();
        let mut unions: Vec<Self> = Vec::new();
        let mut seen: HashSet<String> = HashSet::new();
        seen.insert(self.identity());
        collect_nested_from_fields(
            &self.fields,
            &mut structs,
            &mut unions,
            &mut seen,
            max_depth,
            0,
        );
        (structs, unions)
    }
}

/* ─────────────────────────────── Conversions ────────────────────────────── */

/// Generate a named [`Union`] from a `UnionDecl` entity.
impl<'a> TryFrom<Entity<'a>> for Union<'a> {
    type Error = UnionError;

    fn try_from(entity: Entity<'a>) -> Result<Self, Self::Error> {
        let kind = entity.get_kind();
        if kind != EntityKind::UnionDecl {
            return Err(UnionError::NotUnion(kind));
        }
        let location = SourceLocation::try_from(&entity).ok();
        let is_anonymous = entity
            .get_type()
            .and_then(|t| t.is_anonymous())
            .unwrap_or(false);
        // A named-typed reference reaching this constructor still has
        // a real name on the entity. Anonymous-typed unions go through
        // `from_anon` with caller-supplied context.
        let name = entity
            .get_name()
            .unwrap_or_else(|| "<anonymous union>".to_string());
        let fields = collect_fields(&entity, &name, &[]);
        let size = entity.get_type().and_then(|t| t.get_sizeof().ok());
        Ok(Self {
            entity,
            name,
            kind: RecordKind::Union,
            aliases: Vec::new(),
            is_anonymous,
            enclosing_record: None,
            field_path: Vec::new(),
            location,
            size,
            fields,
        })
    }
}

impl<'a> Union<'a> {
    /// Build a [`Union`] for an anonymous nested record, carrying the
    /// caller-supplied identity. Used by [`Field::get_child_union`]
    /// when the field's `anon_ref` indicates an anonymous union.
    pub(crate) fn from_anon(
        entity: Entity<'a>,
        enclosing_record: String,
        field_path: Vec<String>,
    ) -> Result<Self, UnionError> {
        let kind = entity.get_kind();
        if kind != EntityKind::UnionDecl {
            return Err(UnionError::NotUnion(kind));
        }
        let location = SourceLocation::try_from(&entity).ok();
        let name = field_path
            .last()
            .cloned()
            .unwrap_or_else(|| "<anonymous>".to_string());
        let fields = collect_fields(&entity, &enclosing_record, &field_path);
        let size = entity.get_type().and_then(|t| t.get_sizeof().ok());
        Ok(Self {
            entity,
            name,
            kind: RecordKind::Union,
            aliases: Vec::new(),
            is_anonymous: true,
            enclosing_record: Some(enclosing_record),
            field_path,
            location,
            size,
            fields,
        })
    }
}
