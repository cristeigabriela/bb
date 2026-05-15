//! Struct type representation.

pub(crate) mod field;

pub use field::Field;
pub(crate) use field::collect_fields;

use crate::display;
use crate::error::StructError;
use crate::ext::{AnonymousType, DeclarationKind};
use crate::location::SourceLocation;
use crate::record::RecordKind;
use crate::union_::Union;
use clang::{Entity, EntityKind};
use serde::Serialize;
use std::collections::HashSet;

/* ────────────────────────────────── Types ───────────────────────────────── */

#[derive(Debug, Serialize)]
pub struct Struct<'a> {
    #[serde(skip)]
    entity: Entity<'a>,
    name: String,
    /// Always [`RecordKind::Struct`]. Carried so the JSON shape is
    /// symmetric with [`Union`] entries; consumers can dispatch on
    /// `kind` without checking which array the entry came from.
    kind: RecordKind,
    /// Typedef names that resolve to this struct (e.g. `["LARGE_INTEGER"]`
    /// for `_LARGE_INTEGER`). Populated by callers that have a
    /// [`TypedefIndex`](crate::TypedefIndex) — see
    /// [`Struct::with_aliases`]. Empty when no typedef points here, or
    /// when the caller didn't supply an index.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    aliases: Vec<String>,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    is_anonymous: bool,
    /// Outermost named ancestor's display name. `Some` only for
    /// anonymous nested structs surfaced in a parent's
    /// `referenced_structs` slot.
    #[serde(skip_serializing_if = "Option::is_none")]
    enclosing_record: Option<String>,
    /// Field-name chain from `enclosing_record` down to this record.
    /// Empty for top-level / named structs.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    field_path: Vec<String>,
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

    /// Typedef names that resolve to this struct.
    #[must_use]
    pub fn get_aliases(&self) -> &[String] {
        &self.aliases
    }

    /// Attach typedef alias names to this struct.
    #[must_use]
    pub fn with_aliases(mut self, aliases: Vec<String>) -> Self {
        self.aliases = aliases;
        self
    }

    /// Stable identity for dedup across a flat nested-record list. For
    /// named structs, returns the canonical name. For anonymous ones,
    /// returns `"enclosing_record::field_path"`.
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

    /// Renders this struct in a `WinDbg` `dt`-style format with Unicode box-drawing.
    #[must_use]
    pub fn display(
        &self,
        depth: usize,
        field_filter: Option<&str>,
        typedef_index: Option<&crate::TypedefIndex>,
    ) -> String {
        display::render_struct(self, depth, field_filter, typedef_index)
    }

    /// Names of *named* record types (struct or union) referenced by
    /// this struct's fields. Anonymous nested records are omitted here
    /// because they have no string name — they're only visible in the
    /// full `to_json_full` dump where `(enclosing_record, field_path)`
    /// disambiguates them.
    #[must_use]
    pub fn referenced_type_names(&self) -> Vec<String> {
        referenced_named_record_names(&self.fields)
    }

    /// All nested record types — structs and unions, named and
    /// anonymous — referenced from this struct's field tree, up to
    /// `max_depth` levels of recursion. Returns `(structs, unions)`
    /// for the two JSON slots. Deduplicated by [`Self::identity`] /
    /// [`Union::identity`].
    #[must_use]
    pub fn extract_nested_records(&self, max_depth: usize) -> (Vec<Self>, Vec<Union<'a>>) {
        let mut structs: Vec<Self> = Vec::new();
        let mut unions: Vec<Union<'a>> = Vec::new();
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

/// Generate [`Struct`] from a `struct`/`class` declaration entity.
///
/// Rejects [`EntityKind::UnionDecl`] — unions go through [`Union::try_from`]
/// and are never returned as a struct. This is the change that makes
/// unions un-findable as top-level types via `bb-types -s ...`.
impl<'a> TryFrom<Entity<'a>> for Struct<'a> {
    type Error = StructError;

    fn try_from(entity: Entity<'a>) -> Result<Self, Self::Error> {
        let kind = entity.get_kind();
        if !matches!(kind, EntityKind::ClassDecl | EntityKind::StructDecl) {
            return Err(StructError::NotStructOrClass(kind));
        }

        let location = SourceLocation::try_from(&entity).ok();

        let is_anonymous = entity
            .get_type()
            .and_then(|t| t.is_anonymous())
            .unwrap_or(false);

        let name = if is_anonymous {
            let kind_str = entity
                .get_type()
                .and_then(|t| t.get_declaration_kind_name())
                .unwrap_or("struct");
            format!("<anonymous {kind_str}>")
        } else {
            entity.get_name().ok_or(StructError::NoName)?
        };

        let fields = collect_fields(&entity, &name, &[]);
        let size = entity.get_type().and_then(|t| t.get_sizeof().ok());

        Ok(Self {
            entity,
            name,
            kind: RecordKind::Struct,
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

impl<'a> Struct<'a> {
    /// Build a [`Struct`] for an anonymous nested record, carrying the
    /// caller-supplied identity context. Used by [`Field::get_child_struct`]
    /// when the field's `anon_ref` indicates an anonymous struct.
    pub(crate) fn from_anon(
        entity: Entity<'a>,
        enclosing_record: String,
        field_path: Vec<String>,
    ) -> Result<Self, StructError> {
        let kind = entity.get_kind();
        if !matches!(kind, EntityKind::ClassDecl | EntityKind::StructDecl) {
            return Err(StructError::NotStructOrClass(kind));
        }
        let location = SourceLocation::try_from(&entity).ok();
        // Synthetic name = the field name that points at this record,
        // i.e. the last element of the path. Stable per snapshot.
        let name = field_path
            .last()
            .cloned()
            .unwrap_or_else(|| "<anonymous>".to_string());
        let fields = collect_fields(&entity, &enclosing_record, &field_path);
        let size = entity.get_type().and_then(|t| t.get_sizeof().ok());
        Ok(Self {
            entity,
            name,
            kind: RecordKind::Struct,
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

/* ───────────────────────────── Nested traversal ─────────────────────────── */

/// Collect names of every *named* record (struct or union) referenced
/// by `fields`. Anonymous nested records are intentionally excluded —
/// they live in the full-record extraction path instead, keyed by
/// `(enclosing_record, field_path)`.
pub(crate) fn referenced_named_record_names(fields: &[Field]) -> Vec<String> {
    let mut names = Vec::new();
    let mut seen: HashSet<String> = HashSet::new();
    for field in fields {
        // anon_ref means anonymous target — skip for the name list.
        if field.get_anon_ref().is_some() {
            continue;
        }
        if !field.has_children() {
            continue;
        }
        let Some(decl) = field.get_underlying_type().get_declaration() else {
            continue;
        };
        if !matches!(
            decl.get_kind(),
            EntityKind::StructDecl | EntityKind::ClassDecl | EntityKind::UnionDecl
        ) {
            continue;
        }
        if let Some(name) = decl.get_name()
            && seen.insert(name.clone())
        {
            names.push(name);
        }
    }
    names
}

/// Walk `fields`, accumulating every reachable struct and union (named
/// or anonymous) into `structs` / `unions`. Cycles are broken by the
/// composite-identity `seen` set.
pub(crate) fn collect_nested_from_fields<'a>(
    fields: &[Field<'a>],
    structs: &mut Vec<Struct<'a>>,
    unions: &mut Vec<Union<'a>>,
    seen: &mut HashSet<String>,
    max_depth: usize,
    current_depth: usize,
) {
    if current_depth >= max_depth {
        return;
    }
    for field in fields {
        // Dispatch on the field's underlying record kind directly so
        // a future change to `get_child_struct` / `get_child_union`
        // can't silently change which arm fires.
        match field.record_kind() {
            Some(RecordKind::Struct) => {
                let Some(child_struct) = field.get_child_struct() else {
                    continue;
                };
                let id = child_struct.identity();
                if seen.insert(id) {
                    collect_nested_from_fields(
                        child_struct.get_fields(),
                        structs,
                        unions,
                        seen,
                        max_depth,
                        current_depth + 1,
                    );
                    structs.push(child_struct);
                }
            }
            Some(RecordKind::Union) => {
                let Some(child_union) = field.get_child_union() else {
                    continue;
                };
                let id = child_union.identity();
                if seen.insert(id) {
                    collect_nested_from_fields(
                        child_union.get_fields(),
                        structs,
                        unions,
                        seen,
                        max_depth,
                        current_depth + 1,
                    );
                    unions.push(child_union);
                }
            }
            None => {}
        }
    }
}
