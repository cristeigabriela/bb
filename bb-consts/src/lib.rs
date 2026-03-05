//! This is a library that exposes the utilities necessary to make the
//! [`bb-consts`] CLI.
//!
//! This is an architecture that trivially allows us to share code between
//! [the CLI](bb-consts) and [the TUI](bb-consts-tui), and also reduces
//! the amount of code present in main.rs

/* ────────────────────── Parse, iter, collect, filter ────────────────────── */

use std::collections::HashMap;

use bb_clang::{ConstLookup, Constant, Enum, build_tu_entity_map};
use bb_shared::glob_match;
use clang::{Entity, EntityKind, TranslationUnit};

/// Parse name pattern.
///
/// When `name` contains `::`, it implies that the `name` command-line argument
/// is scoped to an enumeration.
///
/// If `::` is present, then, collect the pattern from the left-hand side of the string,
/// and use that to filter and collect enums, and take the right-hand side as the field
/// to look for in said enum.
///
/// # Examples
///
/// `name`: `Some("A::B")` -> `Some(("A", "B"))`
///
/// `name`: `Some("A")` -> `Some(("", "A"))`
///
/// `name`: `None` -> `None`
#[must_use]
pub fn parse_name_pattern(name: Option<&str>) -> (Option<&str>, Option<&str>) {
    match name {
        Some(n) if n.contains("::") => {
            // SAFETY: gabriela says this is fine because clap doesn't allow
            // there to be a non-empty string here ^^
            let (enum_part, const_part) = n.split_once("::").unwrap();
            (Some(enum_part), Some(const_part))
        }
        Some(n) => (None, Some(n)),
        None => (None, None),
    }
}

/* ─────────────────────────────── ConstFilter ─────────────────────────────── */

/// Filter criteria for enum/constant collection.
pub struct ConstFilter {
    pub header_filter: Option<String>,
    pub enum_pattern: Option<String>,
    pub const_pattern: Option<String>,
    pub case_sensitive: bool,
    pub scoped_to_enum: bool,
}

impl ConstFilter {
    #[must_use]
    pub fn matches_header(&self, entity: &Entity) -> bool {
        let Some(ref filter) = self.header_filter else {
            return true;
        };

        entity
            .get_location()
            .and_then(|loc| loc.get_file_location().file)
            .is_some_and(|f| {
                f.get_path()
                    .to_string_lossy()
                    .to_lowercase()
                    .ends_with(filter)
            })
    }

    #[must_use]
    pub fn matches_enum_name(&self, entity: &Entity) -> bool {
        self.enum_pattern.as_deref().is_none_or(|pat| {
            entity
                .get_name()
                .is_some_and(|name| glob_match(&name, pat, self.case_sensitive))
        })
    }

    #[must_use]
    pub fn matches_const_name(&self, entity: &Entity) -> bool {
        self.const_pattern.as_deref().is_none_or(|pat| {
            entity
                .get_name()
                .is_some_and(|name| glob_match(&name, pat, self.case_sensitive))
        })
    }
}

/* ──────────────────────────────── Collection ─────────────────────────────── */

/// Iterate over enum declarations in [`TranslationUnit`] and collect ones that
/// match filter settings.
#[must_use]
pub fn collect_enums<'a>(tu: &'a TranslationUnit<'a>, filter: &ConstFilter) -> Vec<Enum<'a>> {
    iter_enums(tu)
        .filter(|e| filter.matches_header(e) && filter.matches_enum_name(e))
        .filter_map(|e| Enum::try_from(e).ok())
        .collect()
}

/// Collect and recursively resolve constants from a [`TranslationUnit`].
///
/// Applies the header filter during collection. Name filtering is intentionally
/// deferred so that the lookup table built from these results contains every
/// constant needed for cross-reference resolution.
///
/// Use [`filter_constants_by_name`] after this call to apply the name pattern.
#[must_use]
pub fn collect_constants<'a>(
    tu: &'a TranslationUnit<'a>,
    filter: &ConstFilter,
) -> Vec<Constant<'a>> {
    if filter.scoped_to_enum {
        return Vec::new();
    }

    // Instead of building the translation unit entity map for every single
    // macro, build it once.
    let tu_map = build_tu_entity_map(tu);

    iter_constants(tu)
        .filter(|e| filter.matches_header(e))
        .filter_map(|e| {
            // If you fail building a "regular" constant, it might have references
            // preventing you from directly evaluating it. So, try and check if
            // attempting to resolve possible references fixes it.
            Constant::try_from(e)
                .or_else(|_| Constant::try_from_macro_with_map(e, &tu_map))
                .ok()
        })
        .collect()
}

/// Filter constants by the name pattern in the given filter.
///
/// Call this **after** constant collection so that every constant had a chance
/// to be resolved against the full TU entity map.
#[must_use]
pub fn filter_constants_by_name<'a>(
    constants: Vec<Constant<'a>>,
    filter: &ConstFilter,
) -> Vec<Constant<'a>> {
    match filter.const_pattern.as_deref() {
        Some(pat) => constants
            .into_iter()
            .filter(|c| glob_match(c.get_name(), pat, filter.case_sensitive))
            .collect(),
        None => constants,
    }
}

/// Iterate over enum declarations in a [`TranslationUnit`].
pub fn iter_enums<'a>(tu: &'a TranslationUnit<'a>) -> impl Iterator<Item = Entity<'a>> {
    tu.get_entity()
        .get_children()
        .into_iter()
        .filter(|e| matches!(e.get_kind(), EntityKind::EnumDecl))
}

/// Iterate over constant declarations in a [`TranslationUnit`].
pub fn iter_constants<'a>(tu: &'a TranslationUnit<'a>) -> impl Iterator<Item = Entity<'a>> {
    tu.get_entity().get_children().into_iter().filter(|e| {
        matches!(
            e.get_kind(),
            EntityKind::VarDecl | EntityKind::MacroDefinition
        )
    })
}

/* ───────────────────────────── Display lookup ───────────────────────────── */

/// Build a name -> value lookup table from all known constants (used for
/// display-time composition rendering).
pub fn build_lookup_table(enums: &[Enum], vars: &[Constant]) -> ConstLookup {
    let mut known = HashMap::new();
    for e in enums {
        for c in e.get_constants() {
            known.insert(c.get_name().to_string(), *c.get_value());
        }
    }
    for c in vars {
        known.insert(c.get_name().to_string(), *c.get_value());
    }
    known
}
