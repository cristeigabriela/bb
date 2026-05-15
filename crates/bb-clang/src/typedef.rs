//! Typedef index for resolving alias names to their canonical types.
//!
//! Windows headers expose most struct types under both an underscored
//! declaration name (e.g. `_LARGE_INTEGER`) and a typedef alias
//! (e.g. `LARGE_INTEGER`). Pointer typedefs like `HANDLE` and `PVOID`
//! never declare a struct — they alias `void *`.
//!
//! [`TypedefIndex::build`] walks a [`TranslationUnit`] once, collecting
//! every [`EntityKind::TypedefDecl`] into a name-keyed map. Each entry
//! records:
//!
//! - the immediate next link in the typedef chain (`typedef_of`),
//! - the fully resolved canonical display (`canonical`),
//! - the kind of the final type ([`TypedefKind`]),
//! - the full step-by-step chain.
//!
//! Consumers can then ask:
//!
//! - "Does this name resolve to a struct, and if so what's its canonical
//!   decl name?" — used by `bb-types` to make `LARGE_INTEGER` searches hit
//!   the `_LARGE_INTEGER` struct.
//! - "What are all the typedef aliases for a given canonical decl?" — used
//!   to attach `aliases` to [`Struct`](crate::Struct) for display + JSON.
//! - "What does this typedef expand to?" — used by the field/param
//!   renderers to annotate `HANDLE (void *)`, `PVOID (void *)`, etc.

use std::collections::HashMap;

use clang::{Entity, EntityKind, TranslationUnit, Type, TypeKind};
use serde::Serialize;

use crate::ext::AnonymousType;
use crate::location::SourceLocation;
use crate::type_info::{TypeProperties, is_array_kind, is_function_pointer, is_primitive_kind};

/* ────────────────────────────────── Types ───────────────────────────────── */

/// Classification of what a typedef ultimately resolves to.
///
/// Determined by inspecting the canonical type after walking through every
/// intermediate typedef link.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum TypedefKind {
    /// Canonical type is a `struct` or `class` declaration.
    Struct,
    /// Canonical type is a `union` declaration.
    Union,
    /// Canonical type is an `enum` declaration.
    Enum,
    /// Canonical type is a function pointer.
    FunctionPointer,
    /// Canonical type is a pointer (other than function pointer) — e.g. `void *`, `struct *`.
    Pointer,
    /// Canonical type is a fixed-size or incomplete array.
    Array,
    /// Canonical type is a builtin scalar (int, void, char, ...).
    Primitive,
    /// Anything else we don't classify (templates, dependent types, ...).
    Other,
}

/// A single typedef entry: name + chain + canonical resolution +
/// full type metadata (flattened from [`TypeProperties`]).
///
/// Serializes to a flat JSON object whose shape is identical to what
/// fields and params expose for their own types — programmers get the
/// same vocabulary (`is_pointer`, `pointer_depth`, `is_function_pointer`,
/// `underlying_type` for the terminal primitive, `underlying_record` for
/// the pointee record name, etc.) regardless of which entity they're
/// looking at. That's the contract that lets a generic `inspect_type`
/// helper accept any of these without branching.
#[derive(Debug, Clone, Serialize)]
pub struct Typedef {
    /// Typedef name as declared (e.g. `"LARGE_INTEGER"`, `"HANDLE"`).
    pub name: String,

    /// Categorical classification of the final canonical type. Programmers
    /// can switch on this rather than parsing the booleans below; the
    /// booleans give the full picture (`Pointer` vs `FunctionPointer` is
    /// distinguishable from `is_function_pointer`, for example).
    pub kind: TypedefKind,

    /// Final canonical display, after walking every intermediate typedef.
    /// For `HANDLE`, this is `"void *"`. For `LARGE_INTEGER`, this is
    /// `"_LARGE_INTEGER"`. Equal to `chain.last()`; kept as a top-level
    /// field for ergonomics ("just give me the resolved name").
    pub canonical: String,

    /// Canonical declaration name when the chain ends at a named
    /// `struct`/`union`/`enum`/`class` declaration **directly** (no
    /// intervening pointer or array). `None` for pointer typedefs even
    /// when they point at a named record — use the flattened
    /// `underlying_record` for that case.
    ///
    /// Used to attach `aliases` back to a [`Struct`](crate::Struct).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub canonical_decl_name: Option<String>,

    /// Every step from `name` (exclusive) to `canonical` (inclusive).
    /// `chain.first()` is the immediate alias target (what `typedef X Y;`
    /// was written against); `chain.last()` equals `canonical`. For
    /// `HANDLE` (when defined as `typedef PVOID HANDLE`), this is
    /// `["PVOID", "void *"]`. For a single-step typedef like
    /// `LARGE_INTEGER`, this is `["_LARGE_INTEGER"]`.
    pub chain: Vec<String>,

    /// Full type metadata, flattened into this object's JSON output so
    /// the shape matches `Field` and `Param`. Contains qualifiers,
    /// pointer/array classification, the terminal `underlying_type`
    /// primitive (e.g. `"void"` for `HANDLE`), and the
    /// `underlying_record` decl name (e.g. `"_LARGE_INTEGER"` for
    /// `LARGE_INTEGER`).
    #[serde(flatten)]
    pub properties: TypeProperties,

    /// Source location of the `typedef` declaration, when available.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub location: Option<SourceLocation>,
}

/// Translation-unit-scoped index of every typedef declaration.
///
/// Build once after parsing with [`TypedefIndex::build`], then query
/// repeatedly. Lookup is O(1) on the typedef name; alias-reverse lookup is
/// O(1) on the canonical decl name.
#[derive(Debug, Default, Clone)]
pub struct TypedefIndex {
    by_name: HashMap<String, Typedef>,
    aliases_by_canonical: HashMap<String, Vec<String>>,
}

impl TypedefIndex {
    /// Walk a translation unit and collect every [`EntityKind::TypedefDecl`]
    /// at the top level.
    ///
    /// Typedefs whose underlying type cannot be resolved (anonymous in a way
    /// that produces no display name, or chains that hit a `None` from
    /// libclang) are skipped silently — partial information is more useful
    /// than a hard failure.
    #[must_use]
    pub fn build(tu: &TranslationUnit<'_>) -> Self {
        let mut by_name: HashMap<String, Typedef> = HashMap::new();
        let mut aliases_by_canonical: HashMap<String, Vec<String>> = HashMap::new();

        for entity in tu.get_entity().get_children() {
            if entity.get_kind() != EntityKind::TypedefDecl {
                continue;
            }
            let Some(td) = Self::build_one(&entity) else {
                continue;
            };

            if let Some(canonical_decl) = td.canonical_decl_name.as_ref() {
                aliases_by_canonical
                    .entry(canonical_decl.clone())
                    .or_default()
                    .push(td.name.clone());
            }

            // Last writer wins on duplicate names (multiple TU includes of
            // the same header through different inclusion paths).
            by_name.insert(td.name.clone(), td);
        }

        // Stable, deterministic alias ordering.
        for aliases in aliases_by_canonical.values_mut() {
            aliases.sort();
            aliases.dedup();
        }

        Self {
            by_name,
            aliases_by_canonical,
        }
    }

    /// Look up a typedef by its declared name.
    #[must_use]
    pub fn lookup(&self, name: &str) -> Option<&Typedef> {
        self.by_name.get(name)
    }

    /// Typedef aliases that resolve to a given canonical declaration name.
    ///
    /// Returns an empty slice when no typedef points to this declaration.
    #[must_use]
    pub fn aliases_for(&self, canonical_decl_name: &str) -> &[String] {
        self.aliases_by_canonical
            .get(canonical_decl_name)
            .map(Vec::as_slice)
            .unwrap_or(&[])
    }

    /// Find every typedef whose name matches `pattern` under `case_sensitive`
    /// glob matching. Used by `bb-types -s …` when the search pattern hits
    /// no struct.
    #[must_use]
    pub fn match_pattern(&self, pattern: &str, case_sensitive: bool) -> Vec<&Typedef> {
        let mut out: Vec<&Typedef> = self
            .by_name
            .values()
            .filter(|t| bb_shared::glob_match(&t.name, pattern, case_sensitive))
            .collect();
        out.sort_by(|a, b| a.name.cmp(&b.name));
        out
    }

    /// Every typedef name in the index, for "did you mean" suggestion lists.
    pub fn names(&self) -> impl Iterator<Item = &str> {
        self.by_name.keys().map(String::as_str)
    }

    /// Iterator over every collected [`Typedef`], for testing and tooling.
    pub fn iter(&self) -> impl Iterator<Item = &Typedef> {
        self.by_name.values()
    }

    /// Number of typedef entries collected.
    #[must_use]
    pub fn len(&self) -> usize {
        self.by_name.len()
    }

    /// `true` when no typedefs were collected.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.by_name.is_empty()
    }
}

/* ─────────────────────────────── Internals ──────────────────────────────── */

impl TypedefIndex {
    /// Build a single [`Typedef`] from a [`EntityKind::TypedefDecl`] entity.
    ///
    /// Returns `None` if the entity is unnamed, has no usable underlying
    /// type, or chains to something we can't represent.
    fn build_one(entity: &Entity<'_>) -> Option<Typedef> {
        let name = entity.get_name()?;
        let entity_type = entity.get_type()?;
        let underlying = entity.get_typedef_underlying_type()?;

        // Compute the full type metadata from the typedef's own clang
        // type. This is what makes the JSON shape compatible with
        // Field/Param — programmers see the same vocabulary everywhere.
        let properties = TypeProperties::from_type(&entity_type);

        // Walk the chain step-by-step, recording each link. After every
        // typedef link, the next type may itself be a typedef entity —
        // libclang exposes this via TypeKind::Typedef and a declaration
        // with its own get_typedef_underlying_type.
        let mut chain: Vec<String> = Vec::new();
        let mut current = underlying;
        let mut guard = 0_usize;
        loop {
            chain.push(clean_type_name(&current));

            if current.get_kind() != TypeKind::Typedef {
                break;
            }
            let Some(decl) = current.get_declaration() else {
                break;
            };
            let Some(next) = decl.get_typedef_underlying_type() else {
                break;
            };
            current = next;

            guard += 1;
            if guard > 64 {
                // Defensive: walk shouldn't cycle for well-formed headers,
                // but cap iterations rather than spinning forever.
                break;
            }
        }

        // The terminal type is the canonical form for our purposes.
        let canonical_type = current;
        let canonical = chain
            .last()
            .cloned()
            .unwrap_or_else(|| clean_type_name(&canonical_type));

        let kind = classify(&canonical_type);
        let canonical_decl_name = match kind {
            TypedefKind::Struct | TypedefKind::Union | TypedefKind::Enum => {
                let is_anon = canonical_type.is_anonymous().unwrap_or(false);
                if is_anon {
                    None
                } else {
                    canonical_type.get_declaration().and_then(|d| d.get_name())
                }
            }
            _ => None,
        };

        let location = SourceLocation::try_from(entity).ok();

        Some(Typedef {
            name,
            kind,
            canonical,
            canonical_decl_name,
            chain,
            properties,
            location,
        })
    }
}

/// Normalize a [`Type`] to a clean display string suitable for API output.
///
/// For struct/union/enum types, drops the leading `struct `/`union `/
/// `enum ` keyword that `Type::get_display_name` includes — programmers
/// have the `kind` field for that distinction, so the keyword would just
/// be redundant noise in cross-references. For typedef and pointer/array
/// types, falls back to the display name (which is already clean).
fn clean_type_name(ty: &Type<'_>) -> String {
    // For typedef types, the display name *is* the typedef alias name
    // (e.g. "PVOID", "HANDLE"). That's exactly what we want for chain
    // intermediates.
    if ty.get_kind() == TypeKind::Typedef {
        return ty.get_display_name();
    }
    // For records/enums with a named declaration, use the bare decl name.
    if let Some(decl) = ty.get_declaration()
        && let Some(name) = decl.get_name()
    {
        return name;
    }
    // Pointers, arrays, primitives, and anonymous records: clang's
    // display name is already the right thing (`void *`, `int`, etc.).
    ty.get_display_name()
}

/// Classify the terminal type of a typedef chain.
///
/// Delegates kind-set membership to the shared helpers in
/// [`crate::type_info`] (`is_primitive_kind`, `is_array_kind`,
/// `is_function_pointer`) so the lists of primitive/array `TypeKind`
/// variants live in exactly one place — no inlined repetition.
fn classify(ty: &Type<'_>) -> TypedefKind {
    let canonical = ty.get_canonical_type();

    // Function pointer detection requires looking through the pointee.
    if canonical.get_pointee_type().is_some() {
        if is_function_pointer(&canonical) {
            return TypedefKind::FunctionPointer;
        }
        return TypedefKind::Pointer;
    }

    match canonical.get_kind() {
        TypeKind::Record => match canonical
            .get_declaration()
            .map(|d| d.get_kind())
            .unwrap_or(EntityKind::UnexposedDecl)
        {
            EntityKind::UnionDecl => TypedefKind::Union,
            EntityKind::StructDecl | EntityKind::ClassDecl => TypedefKind::Struct,
            _ => TypedefKind::Struct,
        },
        TypeKind::Enum => TypedefKind::Enum,
        x if is_array_kind(x) => TypedefKind::Array,
        x if is_primitive_kind(x) => TypedefKind::Primitive,
        _ => TypedefKind::Other,
    }
}
