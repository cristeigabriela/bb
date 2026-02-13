//! This is a library that exposes the utilities necessary to make the
//! [`bb-consts`] CLI.
//!
//! This is an architecture that trivially allows us to share code between
//! [the CLI](bb-consts) and [the TUI](bb-consts-tui), and also reduces
//! the amount of code present in main.rs

/* ────────────────────── Parse, iter, collect, filter ────────────────────── */

use std::collections::HashMap;

use bb_clang::{ConstLookup, Constant, Enum};
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

/// Two-pass constant collection over [`TranslationUnit`].
///
/// Returns directly-evaluated constants and failed macro entities (for later
/// resolution with a lookup).
///
/// Collects constants that match filter settings.
#[must_use]
pub fn collect_constants<'a>(
    tu: &'a TranslationUnit<'a>,
    filter: &ConstFilter,
) -> (Vec<Constant<'a>>, Vec<Entity<'a>>) {
    if filter.scoped_to_enum {
        return (Vec::new(), Vec::new());
    }

    let entities: Vec<_> = iter_constants(tu)
        .filter(|e| filter.matches_header(e) && filter.matches_const_name(e))
        .collect();

    let mut vars = Vec::new();
    let mut failed = Vec::new();

    for e in entities {
        match Constant::try_from(e) {
            Ok(c) => vars.push(c),
            Err(_) if e.get_kind() == EntityKind::MacroDefinition => failed.push(e),
            Err(_) => {}
        }
    }

    (vars, failed)
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

/* ────────────────────────────── Macros lookup ───────────────────────────── */

/// Build a name -> value lookup table from all known constants (macros and vars).
#[must_use]
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

/// Resolve failed macros that reference known constants by name.
pub fn resolve_macros<'a>(
    vars: &mut Vec<Constant<'a>>,
    known: &mut ConstLookup,
    failed: &[Entity<'a>],
) {
    for &e in failed {
        if let Ok(c) = Constant::try_from_macro_with_lookup(e, known) {
            known.insert(c.get_name().to_string(), *c.get_value());
            vars.push(c);
        }
    }
}
