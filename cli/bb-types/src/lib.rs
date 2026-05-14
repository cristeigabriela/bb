//! This is a library that exposes the utilities necessary to make the
//! [`bb-types`] CLI.
//!
//! This is an architecture that trivially allows us to share code between
//! [the CLI](bb-types) and [the TUI](bb-types-tui), and also reduces
//! the amount of code present in main.rs

use bb_clang::{Struct, Typedef, TypedefIndex};
use bb_shared::glob_match;
use clang::{Entity, EntityKind, TranslationUnit};

/* ────────────────────── Parse, iter, collect, filter ────────────────────── */

/// Collect every struct/class in the TU that matches `filter`.
///
/// If `index` is supplied, each returned [`Struct`] is annotated with its
/// typedef aliases (e.g. `_LARGE_INTEGER` will carry `["LARGE_INTEGER"]`).
/// The filter's name pattern is then matched against the struct's
/// canonical name **and** every alias, so users can search by either form
/// — `bb-types -s LARGE_INTEGER` and `bb-types -s _LARGE_INTEGER` both
/// hit the same struct.
///
/// When `index` is `None`, behaves exactly like the legacy pre-typedef
/// code path: name-only matching on `Entity::get_name()`, no aliases.
#[must_use]
pub fn collect_structs<'a>(
    tu: &'a TranslationUnit<'a>,
    filter: &StructFilter,
    index: Option<&TypedefIndex>,
) -> Vec<Struct<'a>> {
    iter_structs(tu)
        // Header filter still runs at entity level — cheap and avoids
        // building a Struct for every entity in the TU.
        .filter(|e| filter.matches_header(e))
        .filter_map(|e| Struct::try_from(e).ok())
        .map(|s| {
            if let Some(idx) = index {
                let aliases = idx.aliases_for(s.get_name()).to_vec();
                s.with_aliases(aliases)
            } else {
                s
            }
        })
        .filter(|s| filter.matches_struct_name(s))
        .collect()
}

/// Iterate over struct/class declarations in a [`TranslationUnit`].
///
/// Typedefs are intentionally **not** included here — they're surfaced
/// separately via [`TypedefIndex`]. A typedef-name search finds its
/// canonical struct through that index, then is reported as a regular
/// struct hit with its alias attached.
pub fn iter_structs<'a>(tu: &'a TranslationUnit<'a>) -> impl Iterator<Item = Entity<'a>> {
    tu.get_entity().get_children().into_iter().filter(|e| {
        matches!(
            e.get_kind(),
            EntityKind::StructDecl | EntityKind::ClassDecl | EntityKind::UnionDecl
        )
    })
}

/// Find every typedef in `index` whose name matches `filter.name_pattern`.
///
/// Used by the CLI to surface typedef-only hits (`HANDLE`, `PVOID`, ...)
/// when no struct matches the user's search. Returns an empty slice when
/// the filter has no name pattern.
#[must_use]
pub fn find_typedef_hits<'i>(index: &'i TypedefIndex, filter: &StructFilter) -> Vec<&'i Typedef> {
    let Some(ref pattern) = filter.name_pattern else {
        return Vec::new();
    };
    index.match_pattern(pattern, filter.case_sensitive)
}

/* ────────────────────────────────── Match ───────────────────────────────── */

pub struct StructFilter {
    pub name_pattern: Option<String>,
    pub header_filter: Option<String>,
    pub case_sensitive: bool,
}

impl StructFilter {
    /// Whether `entity`'s source file matches the header filter (if any).
    ///
    /// Cheap entity-level prefilter that runs before [`Struct::try_from`].
    #[must_use]
    pub fn matches_header(&self, entity: &Entity) -> bool {
        self.header_filter
            .as_ref()
            .is_none_or(|f| bb_clang::entity_in_header(entity, f))
    }

    /// Whether a fully-built [`Struct`] matches the name pattern.
    ///
    /// Matches against the struct's canonical name and every typedef
    /// alias, so a search for `LARGE_INTEGER` resolves to the
    /// `_LARGE_INTEGER` struct via its alias.
    #[must_use]
    pub fn matches_struct_name(&self, s: &Struct) -> bool {
        let Some(ref pattern) = self.name_pattern else {
            return true;
        };
        if glob_match(s.get_name(), pattern, self.case_sensitive) {
            return true;
        }
        s.get_aliases()
            .iter()
            .any(|a| glob_match(a, pattern, self.case_sensitive))
    }

    /// Legacy entity-only name match.
    ///
    /// Retained for callers that don't have a built [`Struct`] yet (none
    /// in tree as of now, but kept stable for future entity-level uses).
    #[must_use]
    pub fn matches_name(&self, entity: &Entity) -> bool {
        match (&self.name_pattern, entity.get_name()) {
            (Some(pattern), Some(name)) => glob_match(&name, pattern, self.case_sensitive),
            (Some(_), None) => false,
            (None, _) => true,
        }
    }

    /// Entity-level prefilter combining name and header checks.
    ///
    /// Used by callers that filter on raw `Entity` (e.g. suggestion
    /// lists). Note that `collect_structs` does the post-build name
    /// match itself because aliases aren't known at entity time.
    #[must_use]
    pub fn matches(&self, entity: &Entity) -> bool {
        self.matches_name(entity) && self.matches_header(entity)
    }
}
