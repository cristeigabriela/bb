//! Field type representation.
//!
//! Each field embeds a [`TypeInfo`](crate::TypeInfo) for shared type
//! classification (pointer, array, const, underlying type). Nameless
//! fields — typical of OVERLAPPED-style anonymous unions/structs in
//! Windows headers under default MSVC parsing — are accepted: their
//! [`Field::name`] is a synthetic `<anonymous_N>` (per-parent counter)
//! and [`Field::is_anonymous`] is set. When the field's underlying
//! declaration is itself anonymous, [`Field::anon_ref`] cross-references
//! the entry in the parent struct's `referenced_structs` /
//! `referenced_unions` slot.

use crate::error::FieldError;
use crate::ext::{AnonymousType, HasChildrenType};
use crate::location::SourceLocation;
use crate::record::{AnonRef, RecordKind};
use crate::type_info::TypeInfo;
use crate::union_::Union;
use clang::{Entity, EntityKind, EntityVisitResult, Type};
use serde::Serialize;

use super::Struct;

/* ────────────────────────────────── Types ───────────────────────────────── */

#[derive(Debug, Serialize)]
pub struct Field<'a> {
    #[serde(skip)]
    entity: Entity<'a>,
    #[serde(skip)]
    semantic_parent: Entity<'a>,
    /// Field name. For C-source nameless fields (the result of
    /// `DUMMYUNIONNAME` / `DUMMYSTRUCTNAME` macros expanding to empty
    /// under default MSVC parsing) this is a synthetic
    /// `<anonymous_N>` where `N` is the index among the parent
    /// record's nameless siblings only. The angle brackets make the
    /// synthetic name lexically impossible to mistake for a real C
    /// identifier.
    name: String,
    #[serde(rename = "type")]
    type_name: Option<String>,
    #[serde(flatten)]
    type_info: TypeInfo<'a>,
    /// `true` when this field has no name in the C source. The display
    /// layer suppresses the synthetic name; the JSON layer keeps it as
    /// the cross-reference key. Consumers use this flag to drop the
    /// field name when reconstructing C source.
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    is_anonymous: bool,
    /// Cross-reference into the parent struct's `referenced_structs` /
    /// `referenced_unions` slot. Set when the field's underlying
    /// declaration is an anonymous record. The pair
    /// `(enclosing_record, field_path)` selects the matching record
    /// entry; `kind` picks the slot.
    #[serde(skip_serializing_if = "Option::is_none")]
    anon_ref: Option<AnonRef>,
    location: Option<SourceLocation>,
    #[serde(rename = "offset_bits")]
    offset: usize,
    #[serde(rename = "offset")]
    offset_bytes: usize,
    size: usize,
    alignment: usize,
}

impl<'a> Field<'a> {
    /// The underlying clang entity this `Field` was built from.
    ///
    /// Contract — two cases:
    ///
    /// 1. A [`EntityKind::FieldDecl`] for fields built from a real C
    ///    field declaration.
    ///
    /// 2. A [`EntityKind::StructDecl`] / [`EntityKind::UnionDecl`] /
    ///    [`EntityKind::ClassDecl`] for synthetic entries representing
    ///    anonymous nested records that appear as sibling decls (the
    ///    `DUMMYUNIONNAME`-expands-to-empty pattern under default MSVC
    ///    parsing).
    ///
    /// Use [`Self::get_field_decl`] when only a `FieldDecl` will do,
    /// or check [`Self::is_anonymous`] / [`Self::get_anon_ref`] to
    /// discriminate.
    #[must_use]
    pub const fn get_entity(&self) -> &Entity<'a> {
        &self.entity
    }

    /// The `FieldDecl` this field was built from, if any.
    ///
    /// Returns `None` for synthetic anon-record entries: those have
    /// no `FieldDecl` in clang's AST and store the record decl in
    /// [`Self::get_entity`] instead.
    #[must_use]
    pub fn get_field_decl(&self) -> Option<&Entity<'a>> {
        if self.entity.get_kind() == EntityKind::FieldDecl {
            Some(&self.entity)
        } else {
            None
        }
    }

    /// Semantic parent of this field — always the enclosing record.
    ///
    /// Real `FieldDecl`s: clang reports the enclosing record directly.
    ///
    /// Synthetic anon-record entries: the anon record's own
    /// `semantic_parent` resolves to the same enclosing record (because
    /// the anon decl was visited as a child of that record in
    /// `collect_fields`).
    #[must_use]
    pub const fn get_semantic_parent(&self) -> &Entity<'a> {
        &self.semantic_parent
    }
    #[must_use]
    pub fn get_name(&self) -> &str {
        &self.name
    }
    #[must_use]
    pub const fn get_type(&self) -> &Type<'a> {
        self.type_info.get_type()
    }
    #[must_use]
    pub fn get_type_name(&self) -> Option<&str> {
        self.type_name.as_deref()
    }
    #[must_use]
    pub fn get_canonical_type(&self) -> Type<'a> {
        self.type_info.get_canonical_type()
    }
    #[must_use]
    pub const fn get_type_info(&self) -> &TypeInfo<'a> {
        &self.type_info
    }
    #[must_use]
    pub const fn get_location(&self) -> Option<&SourceLocation> {
        self.location.as_ref()
    }
    #[must_use]
    pub const fn get_offset(&self) -> usize {
        self.offset
    }
    #[must_use]
    pub const fn get_offset_bytes(&self) -> usize {
        self.offset_bytes
    }
    #[must_use]
    pub const fn get_size(&self) -> usize {
        self.size
    }
    #[must_use]
    pub const fn get_alignment(&self) -> usize {
        self.alignment
    }
    #[must_use]
    pub const fn is_anonymous(&self) -> bool {
        self.is_anonymous
    }
    /// Cross-reference into the parent record's `referenced_structs` /
    /// `referenced_unions` slot. `Some` only when the field's
    /// underlying type is an anonymous record.
    #[must_use]
    pub const fn get_anon_ref(&self) -> Option<&AnonRef> {
        self.anon_ref.as_ref()
    }

    /// Returns the underlying type of this field, resolving pointers and arrays.
    #[must_use]
    pub fn get_underlying_type(&self) -> Type<'a> {
        self.type_info.get_underlying_type()
    }

    /// Returns true if this field's underlying type has child fields
    /// that can be expanded.
    #[must_use]
    pub fn has_children(&self) -> bool {
        self.get_underlying_type().has_children()
    }

    /// Classify the record kind of this field's underlying type.
    ///
    /// Returns `None` when the field's type isn't a record — it's a
    /// primitive, a function pointer, or otherwise unresolvable.
    ///
    /// Drives kind dispatch in nested-record walkers without going
    /// through the fallible `get_child_struct` / `get_child_union`
    /// constructors.
    #[must_use]
    pub fn record_kind(&self) -> Option<RecordKind> {
        let decl = self.get_underlying_type().get_declaration()?;
        match decl.get_kind() {
            EntityKind::StructDecl | EntityKind::ClassDecl => Some(RecordKind::Struct),
            EntityKind::UnionDecl => Some(RecordKind::Union),
            _ => None,
        }
    }

    /// The struct/class declaration this field's underlying type
    /// points at, if any.
    ///
    /// Returns `None` for unions or non-record types.
    /// Use [`Self::get_child_union`] for unions.
    #[must_use]
    pub fn get_child_struct(&self) -> Option<Struct<'a>> {
        let underlying = self.get_underlying_type();
        let decl = underlying.get_declaration()?;
        if !matches!(
            decl.get_kind(),
            EntityKind::StructDecl | EntityKind::ClassDecl
        ) {
            return None;
        }
        if let Some(aref) = &self.anon_ref
            && aref.kind == RecordKind::Struct
        {
            return Struct::from_anon(decl, aref.enclosing_record.clone(), aref.field_path.clone())
                .ok();
        }
        Struct::try_from(decl).ok()
    }

    /// Collect the child fields of this field's underlying record.
    ///
    /// When this field carries an [`AnonRef`], the anon context
    /// (enclosing record + field path) is inherited, so any synthetic
    /// names assigned to grand-child anon decls continue to use a
    /// consistent path prefix.
    ///
    /// Used by the display layer for inline tree expansion. Returns
    /// an empty `Vec` when the underlying type isn't a record.
    #[must_use]
    pub fn get_child_fields(&self) -> Vec<Self> {
        let underlying = self.get_underlying_type();
        let Some(decl) = underlying.get_declaration() else {
            return Vec::new();
        };
        let is_record = matches!(
            decl.get_kind(),
            EntityKind::StructDecl | EntityKind::ClassDecl | EntityKind::UnionDecl
        );
        if !is_record {
            return Vec::new();
        }
        match self.anon_ref.as_ref() {
            Some(aref) => collect_fields(&decl, &aref.enclosing_record, &aref.field_path),
            None => {
                let name = decl.get_name().unwrap_or_default();
                collect_fields(&decl, &name, &[])
            }
        }
    }

    /// The union declaration this field's underlying type points at,
    /// if any.
    ///
    /// Returns `None` for structs or non-record types.
    /// Use [`Self::get_child_struct`] for structs.
    #[must_use]
    pub fn get_child_union(&self) -> Option<Union<'a>> {
        let underlying = self.get_underlying_type();
        let decl = underlying.get_declaration()?;
        if decl.get_kind() != EntityKind::UnionDecl {
            return None;
        }
        if let Some(aref) = &self.anon_ref
            && aref.kind == RecordKind::Union
        {
            return Union::from_anon(decl, aref.enclosing_record.clone(), aref.field_path.clone())
                .ok();
        }
        Union::try_from(decl).ok()
    }
}

/* ──────────────────────────────── Utilities ─────────────────────────────── */

/// Collects all field declarations from a struct/class/union entity.
///
/// `enclosing_record` is the outermost named ancestor's display name.
/// For a top-level record this is the record's own name; for nested
/// anonymous records the same name propagates through the walk.
///
/// `parent_path` is the chain of field names from `enclosing_record`
/// down to (but not including) `parent`. Empty at the top level.
///
/// Nameless members are accepted with a synthetic name `<anonymous_N>`.
/// Two independent per-parent counters drive the index `N`:
///
/// - `nameless_field_idx` — bumps for nameless `FieldDecl`s.
/// - `sibling_decl_idx` — bumps for sibling anon-record decls
///   (`StructDecl` / `UnionDecl` / `ClassDecl` with no enclosing
///   FieldDecl).
///
/// Splitting the counters means an ordering shuffle in clang's child
/// visit order can't perturb existing identities. Both counters reset
/// per `collect_fields` call, matching the per-parent scoping in the
/// JSON `field_path` semantics.
#[must_use]
pub fn collect_fields<'a>(
    parent: &Entity<'a>,
    enclosing_record: &str,
    parent_path: &[String],
) -> Vec<Field<'a>> {
    let mut fields = Vec::new();
    let mut nameless_field_idx: usize = 0;
    let mut sibling_decl_idx: usize = 0;

    parent.visit_children(|child, _| {
        match child.get_kind() {
            EntityKind::FieldDecl => {
                let (name, is_anonymous) = match child.get_name() {
                    Some(n) => (n, false),
                    None => {
                        let n = format!("<anonymous_{nameless_field_idx}>");
                        nameless_field_idx += 1;
                        (n, true)
                    }
                };
                if let Ok(field) = build_field(
                    child,
                    parent,
                    name,
                    is_anonymous,
                    enclosing_record,
                    parent_path,
                ) {
                    fields.push(field);
                }
            }

            // Sibling anonymous record decls.
            //
            // Clang represents `union { ... } DUMMYUNIONNAME;` (where
            // the macro expands to empty under default MSVC parsing) as
            // an anonymous `UnionDecl` child of the parent struct — no
            // wrapping `FieldDecl`.
            //
            // We synthesize a Field for each so the anon record shows
            // up at its real offset in the parent's layout, with an
            // `anon_ref` pointing into the parent's `referenced_types`
            // slot for the body.
            EntityKind::StructDecl | EntityKind::ClassDecl | EntityKind::UnionDecl
                if child.is_anonymous() =>
            {
                let name = format!("<anonymous_{sibling_decl_idx}>");
                sibling_decl_idx += 1;
                if let Ok(field) =
                    build_anon_record_field(child, parent, name, enclosing_record, parent_path)
                {
                    fields.push(field);
                }
            }

            _ => {}
        }
        EntityVisitResult::Continue
    });

    fields
}

/// Build a [`Field`] with caller-supplied name and anon context.
///
/// Replaces the prior `TryFrom<(Entity, &Entity)>` constructor —
/// `collect_fields` is now the canonical entry point because it
/// computes the synthetic-name counter and threads
/// `enclosing_record` / `parent_path`. The function is `pub(crate)`
/// because the only legitimate caller is the field-collection loop.
pub(crate) fn build_field<'a>(
    entity: Entity<'a>,
    parent: &Entity<'a>,
    name: String,
    is_anonymous: bool,
    enclosing_record: &str,
    parent_path: &[String],
) -> Result<Field<'a>, FieldError> {
    let kind = entity.get_kind();
    if kind != EntityKind::FieldDecl {
        return Err(FieldError::NotField(kind));
    }

    let type_ = entity.get_type().ok_or(FieldError::NoType)?;
    let semantic_parent = entity.get_semantic_parent().ok_or(FieldError::NoType)?;
    let anonymous_type = type_.is_anonymous().unwrap_or(false);
    let type_name = (!anonymous_type).then(|| type_.get_display_name());

    let mut type_info = TypeInfo::from(type_);
    type_info.suppress_underlying_if_matches(type_name.as_deref());

    let parent_type = parent.get_type().ok_or(FieldError::NoType)?;
    // Offset lookup needs a real C identifier — skip for nameless
    // fields, which carry offset 0 by C semantics (anon members live
    // at the parent's offset).
    let offset = if is_anonymous {
        0
    } else {
        parent_type
            .get_offsetof(&name)
            .map_err(|_| FieldError::NoOffset(name.clone()))?
    };

    let location = SourceLocation::try_from(&entity).ok();
    let size = type_.get_sizeof().map_err(|_| FieldError::NoSize)?;
    let alignment = type_.get_alignof().map_err(|_| FieldError::NoAlignment)?;

    let anon_ref = compute_anon_ref(&type_, &name, enclosing_record, parent_path);

    Ok(Field {
        entity,
        semantic_parent,
        name,
        type_name,
        type_info,
        is_anonymous,
        anon_ref,
        location,
        offset,
        offset_bytes: offset / 8,
        size,
        alignment,
    })
}

/// Build a synthetic [`Field`] from an anonymous record decl that
/// appears as a direct child of a struct/union (no enclosing
/// FieldDecl). Used for the OVERLAPPED pattern where
/// `DUMMYUNIONNAME` expands to empty and the union is reachable only
/// as a sibling decl of the named fields.
///
/// The synthetic field's offset is recovered by asking the parent
/// type for the offset of the first reachable named member of the
/// anonymous record — under default MSVC parsing, anonymous members
/// are accessible from the parent's namespace, so this offset is
/// well-defined and exactly equals the anonymous record's own offset
/// within the parent.
fn build_anon_record_field<'a>(
    anon_decl: Entity<'a>,
    parent: &Entity<'a>,
    name: String,
    enclosing_record: &str,
    parent_path: &[String],
) -> Result<Field<'a>, FieldError> {
    let kind = match anon_decl.get_kind() {
        EntityKind::StructDecl | EntityKind::ClassDecl => RecordKind::Struct,
        EntityKind::UnionDecl => RecordKind::Union,
        // visit_children only calls us when one of those three matched.
        other => return Err(FieldError::NotField(other)),
    };

    let anon_type = anon_decl.get_type().ok_or(FieldError::NoType)?;
    let size = anon_type.get_sizeof().map_err(|_| FieldError::NoSize)?;
    let alignment = anon_type
        .get_alignof()
        .map_err(|_| FieldError::NoAlignment)?;
    let parent_type = parent.get_type().ok_or(FieldError::NoType)?;
    // Under default MSVC parsing the anon record's members are
    // reachable from the parent's namespace, so picking any of them
    // and asking the parent's type for its offset gives the anon
    // record's own offset. With `NONAMELESSUNION` defined (or any
    // other parse config that hides the namespace hoisting), this
    // fails — the synthetic field would have to land at the wrong
    // offset, so we drop it instead of silently emitting 0.
    let offset = anon_record_offset_in_parent(&parent_type, &anon_decl)
        .ok_or_else(|| FieldError::NoOffset(name.clone()))?;
    let location = SourceLocation::try_from(&anon_decl).ok();
    let semantic_parent = anon_decl.get_semantic_parent().ok_or(FieldError::NoType)?;

    let type_info = TypeInfo::from(anon_type);

    let mut path = parent_path.to_vec();
    path.push(name.clone());
    let anon_ref = Some(AnonRef {
        kind,
        enclosing_record: enclosing_record.to_string(),
        field_path: path,
    });

    Ok(Field {
        entity: anon_decl,
        semantic_parent,
        name,
        type_name: None,
        type_info,
        is_anonymous: true,
        anon_ref,
        location,
        offset,
        offset_bytes: offset / 8,
        size,
        alignment,
    })
}

/// Compute the bit-offset of an anonymous record inside its parent.
///
/// Strategy: walk the anon record's children for any reachable named
/// member, then ask the parent type for that member's offset. Under
/// default MSVC parsing the anon record's members are hoisted into
/// the parent's namespace, so the answer equals the anon record's
/// own offset.
///
/// Recurses through nested anonymous records — the OVERLAPPED case
/// (anon union → anon struct → `Offset`) bottoms out on the named
/// `Offset` field even though every intermediate record is nameless.
///
/// Returns the offset in bits (libclang's `get_offsetof` unit), or
/// `None` if no reachable named member exists. The caller treats
/// `None` as a build failure rather than substituting zero.
fn anon_record_offset_in_parent(
    parent_type: &clang::Type<'_>,
    anon_decl: &Entity<'_>,
) -> Option<usize> {
    for child in anon_decl.get_children() {
        match child.get_kind() {
            EntityKind::FieldDecl => {
                if let Some(name) = child.get_name()
                    && let Ok(off) = parent_type.get_offsetof(&name)
                {
                    return Some(off);
                }
            }
            EntityKind::StructDecl | EntityKind::ClassDecl | EntityKind::UnionDecl
                if child.is_anonymous() =>
            {
                if let Some(off) = anon_record_offset_in_parent(parent_type, &child) {
                    return Some(off);
                }
            }
            _ => {}
        }
    }
    None
}

/// If the field's underlying declaration is an anonymous record, build
/// the [`AnonRef`] that cross-references the matching entry in the
/// enclosing struct's `referenced_structs` / `referenced_unions` slot.
///
/// Returns `None` for fields whose type is named (its target lives in
/// the named lookup slot), a primitive, a function pointer, or an
/// anonymous *pointer-to-record* (the pointee is at one remove and
/// shouldn't be inlined).
fn compute_anon_ref(
    field_type: &Type<'_>,
    field_name: &str,
    enclosing_record: &str,
    parent_path: &[String],
) -> Option<AnonRef> {
    let canonical = field_type.get_canonical_type();
    // Only the *direct* declaration kind matters — we don't follow
    // pointer/array layers, because an anon record reached via a
    // pointer wouldn't be inlined anyway.
    let decl = canonical.get_declaration()?;
    if !decl.is_anonymous() {
        return None;
    }
    let kind = match decl.get_kind() {
        EntityKind::StructDecl | EntityKind::ClassDecl => RecordKind::Struct,
        EntityKind::UnionDecl => RecordKind::Union,
        _ => return None,
    };
    let mut path: Vec<String> = parent_path.to_vec();
    path.push(field_name.to_string());
    Some(AnonRef {
        kind,
        enclosing_record: enclosing_record.to_string(),
        field_path: path,
    })
}
