//! This is a library that exposes the utilities necessary to make the
//! [`bb-funcs`] CLI.

use bb_clang::Function;
use clang::{Entity, EntityKind, TranslationUnit};

/* ────────────────────── Parse, iter, collect, filter ────────────────────── */

pub fn collect_funcs<'a>(tu: &'a TranslationUnit<'a>) -> Vec<Function<'a>> {
    iter_funcs(tu)
        .filter_map(|e| Function::try_from(e).ok())
        .collect()
}

pub fn iter_funcs<'a>(tu: &'a TranslationUnit<'a>) -> impl Iterator<Item = Entity<'a>> {
    tu.get_entity()
        .get_children()
        .into_iter()
        .filter(|e| matches!(e.get_kind(), EntityKind::FunctionDecl))
}
