//! This is a library that exposes the utilities necessary to make the
//! [`bb-types`] CLI.
//!
//! This is an architecture that trivially allows us to share code between
//! [the CLI](bb-types) and [the TUI](bb-types-tui), and also reduces
//! the amount of code present in main.rs

use bb_clang::Struct;
use bb_shared::glob_match;
use clang::{Entity, EntityKind, TranslationUnit};

/* ────────────────────── Parse, iter, collect, filter ────────────────────── */

#[must_use]
pub fn collect_structs<'a>(tu: &'a TranslationUnit<'a>, filter: &StructFilter) -> Vec<Struct<'a>> {
    iter_structs(tu)
        .filter(|e| filter.matches(e))
        .filter_map(|e| Struct::try_from(e).ok())
        .collect()
}

/// Iterate over struct declarations in a [`TranslationUnit`].
pub fn iter_structs<'a>(tu: &'a TranslationUnit<'a>) -> impl Iterator<Item = Entity<'a>> {
    tu.get_entity()
        .get_children()
        .into_iter()
        .filter(|e| matches!(e.get_kind(), EntityKind::StructDecl | EntityKind::ClassDecl))
}

/* ────────────────────────────────── Match ───────────────────────────────── */

pub struct StructFilter {
    pub name_pattern: Option<String>,
    pub header_filter: Option<String>,
    pub case_sensitive: bool,
}

impl StructFilter {
    #[must_use]
    pub fn matches(&self, entity: &Entity) -> bool {
        self.matches_name(entity) && self.matches_header(entity)
    }

    #[must_use]
    pub fn matches_name(&self, entity: &Entity) -> bool {
        match (&self.name_pattern, entity.get_name()) {
            (Some(pattern), Some(name)) => glob_match(&name, pattern, self.case_sensitive),
            (Some(_), None) => false,
            (None, _) => true,
        }
    }

    #[must_use]
    pub fn matches_header(&self, entity: &Entity) -> bool {
        let Some(filter) = self.header_filter.as_ref().map(|x| x.to_lowercase()) else {
            return true;
        };

        entity
            .get_location()
            .and_then(|loc| loc.get_file_location().file)
            .is_some_and(|f| {
                f.get_path()
                    .to_string_lossy()
                    .to_lowercase()
                    .ends_with(&filter)
            })
    }
}
