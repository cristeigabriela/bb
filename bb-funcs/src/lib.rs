//! This is a library that exposes the utilities necessary to make the
//! [`bb-funcs`] CLI.
//!
//! This is an architecture that trivially allows us to share code between
//! the CLI and a future TUI, and also reduces the amount of code present
//! in main.rs.

use bb_clang::Function;
use bb_shared::glob_match;
use clang::{Entity, EntityKind, TranslationUnit};

/* ────────────────────── Parse, iter, collect, filter ────────────────────── */

#[must_use]
pub fn collect_funcs<'a>(tu: &'a TranslationUnit<'a>) -> Vec<Function<'a>> {
    iter_funcs(tu)
        .filter_map(|e| Function::try_from(e).ok())
        .collect()
}

#[must_use]
pub fn collect_funcs_filtered<'a>(
    tu: &'a TranslationUnit<'a>,
    filter: &FuncFilter,
) -> Vec<Function<'a>> {
    let funcs: Vec<Function<'a>> = iter_funcs(tu)
        .filter(|e| filter.matches(e))
        .filter_map(|e| Function::try_from(e).ok())
        .collect();

    filter.post_filter(funcs)
}

/// Iterate over function declarations in a [`TranslationUnit`].
pub fn iter_funcs<'a>(tu: &'a TranslationUnit<'a>) -> impl Iterator<Item = Entity<'a>> {
    tu.get_entity()
        .get_children()
        .into_iter()
        .filter(|e| matches!(e.get_kind(), EntityKind::FunctionDecl))
}

/* ────────────────────────────────── Match ───────────────────────────────── */

pub struct FuncFilter {
    pub name_pattern: Option<String>,
    pub header_filter: Option<String>,
    pub case_sensitive: bool,
    pub dllimport_only: bool,
}

impl FuncFilter {
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

    /// Post-collection filter: apply `dllimport_only` to already-parsed functions.
    #[must_use]
    pub fn post_filter<'a>(&self, funcs: Vec<Function<'a>>) -> Vec<Function<'a>> {
        if self.dllimport_only {
            funcs.into_iter().filter(Function::is_dllimport).collect()
        } else {
            funcs
        }
    }
}
