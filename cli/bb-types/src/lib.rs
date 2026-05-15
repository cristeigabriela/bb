//! This is a library that exposes the utilities necessary to make the
//! [`bb-types`] CLI.
//!
//! This is an architecture that trivially allows us to share code between
//! [the CLI](bb-types) and [the TUI](bb-types-tui), and also reduces
//! the amount of code present in main.rs

use std::collections::HashSet;

use bb_clang::display::format_typedef_summary;
use bb_clang::json::records_to_json_full;
use bb_clang::{Struct, ToJson, Typedef, TypedefIndex, Union};
use bb_cli::print_suggestions;
use bb_shared::glob_match;
use clang::{Entity, EntityKind, TranslationUnit};
use colored::Colorize;
use serde_json::Value;

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
/// Excludes [`EntityKind::UnionDecl`] — unions are surfaced through
/// the parallel [`iter_unions`] and [`collect_unions`] entry points so
/// the [`Struct`] / [`Union`] types stay strictly separated. Anonymous
/// nested unions never appear as top-level decls in the TU, so the
/// filter is exact.
pub fn iter_structs<'a>(tu: &'a TranslationUnit<'a>) -> impl Iterator<Item = Entity<'a>> {
    tu.get_entity()
        .get_children()
        .into_iter()
        .filter(|e| matches!(e.get_kind(), EntityKind::StructDecl | EntityKind::ClassDecl))
}

/// Iterate over union declarations in a [`TranslationUnit`].
///
/// Yields only **named** top-level unions (e.g. `_LARGE_INTEGER`).
/// Anonymous unions never appear at the top level — they always live
/// inside a parent record and are surfaced via that record's
/// `referenced_unions` slot.
pub fn iter_unions<'a>(tu: &'a TranslationUnit<'a>) -> impl Iterator<Item = Entity<'a>> {
    tu.get_entity()
        .get_children()
        .into_iter()
        .filter(|e| e.get_kind() == EntityKind::UnionDecl)
}

/// Collect every named union in the TU that matches `filter`.
///
/// Mirrors [`collect_structs`]: optional [`TypedefIndex`] attaches
/// typedef aliases (so `_LARGE_INTEGER` carries `["LARGE_INTEGER"]`),
/// and the filter's name pattern matches against both the canonical
/// name and every alias.
#[must_use]
pub fn collect_unions<'a>(
    tu: &'a TranslationUnit<'a>,
    filter: &StructFilter,
    index: Option<&TypedefIndex>,
) -> Vec<Union<'a>> {
    iter_unions(tu)
        .filter(|e| filter.matches_header(e))
        .filter_map(|e| Union::try_from(e).ok())
        .map(|u| {
            if let Some(idx) = index {
                let aliases = idx.aliases_for(u.get_name()).to_vec();
                u.with_aliases(aliases)
            } else {
                u
            }
        })
        .filter(|u| filter.matches_union_name(u))
        .collect()
}

/// Find a single named union by its exact canonical name.
/// Mirrors [`find_struct_by_name`].
#[must_use]
pub fn find_union_by_name<'a>(
    tu: &'a TranslationUnit<'a>,
    name: &str,
    index: Option<&TypedefIndex>,
) -> Option<Union<'a>> {
    iter_unions(tu)
        .filter(|e| e.get_name().as_deref() == Some(name))
        .filter_map(|e| Union::try_from(e).ok())
        .map(|u| {
            if let Some(idx) = index {
                let aliases = idx.aliases_for(u.get_name()).to_vec();
                u.with_aliases(aliases)
            } else {
                u
            }
        })
        .next()
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

/// Find a single struct/union/class declaration by its exact canonical
/// name. Used to auto-expand pointer-typedef targets — when a search
/// surfaces `LPSECURITY_ATTRIBUTES`, we use this to also pull in the
/// `_SECURITY_ATTRIBUTES` struct it points to.
///
/// Returns `None` if no declaration matches (e.g. the typedef points to
/// an opaque struct never defined in the TU).
#[must_use]
pub fn find_struct_by_name<'a>(
    tu: &'a TranslationUnit<'a>,
    name: &str,
    index: Option<&TypedefIndex>,
) -> Option<Struct<'a>> {
    iter_structs(tu)
        .filter(|e| e.get_name().as_deref() == Some(name))
        .filter_map(|e| Struct::try_from(e).ok())
        .map(|s| {
            if let Some(idx) = index {
                let aliases = idx.aliases_for(s.get_name()).to_vec();
                s.with_aliases(aliases)
            } else {
                s
            }
        })
        .next()
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
    /// alias, so a search for `FILETIME` resolves to the
    /// `_FILETIME` struct via its alias.
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

    /// Whether a fully-built [`Union`] matches the name pattern.
    /// Mirrors [`Self::matches_struct_name`] — `LARGE_INTEGER` resolves
    /// to `_LARGE_INTEGER` via its typedef alias.
    #[must_use]
    pub fn matches_union_name(&self, u: &Union) -> bool {
        let Some(ref pattern) = self.name_pattern else {
            return true;
        };
        if glob_match(u.get_name(), pattern, self.case_sensitive) {
            return true;
        }
        u.get_aliases()
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

/* ──────────────────────────────── Results ───────────────────────────────── */

/// One end-to-end result of running a `bb-types` query: every record
/// the user's filter resolved (structs + unions) plus the typedef
/// hits that surfaced alongside.
///
/// Built by [`collect_results`]. `typedef_hits` is the full set used
/// for JSON / SQLite output; `typedef_hits_text` is the filtered
/// view for plain-text display, with redundant stubs (those already
/// covered by an `[aka …]` chip on a rendered record) suppressed.
pub struct TypeResults<'tu, 'idx> {
    pub structs: Vec<Struct<'tu>>,
    pub unions: Vec<Union<'tu>>,
    pub typedef_hits: Vec<&'idx Typedef>,
    pub typedef_hits_text: Vec<&'idx Typedef>,
}

impl<'tu, 'idx> TypeResults<'tu, 'idx> {
    /// `true` when nothing matched — caller should fall back to
    /// [`suggest_alternatives`].
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.structs.is_empty() && self.unions.is_empty() && self.typedef_hits.is_empty()
    }

    /// Top-level JSON shape:
    ///
    /// ```jsonc
    /// {
    ///   "command":          "...",
    ///   "types":            [ /* structs + unions, distinguished by kind */ ],
    ///   "referenced_types": [ /* every record reachable through fields */ ],
    ///   "typedefs":         [ /* all typedef hits */ ]
    /// }
    /// ```
    ///
    /// Delegates to [`bb_clang::json::records_to_json_full`] for the
    /// `types` + `referenced_types` half, then wraps with the
    /// CLI-level `command` and `typedefs` envelope.
    #[must_use]
    pub fn to_json_value(&self, command: String) -> Value {
        let typedefs_value =
            serde_json::to_value(&self.typedef_hits).expect("Typedef list serializes");
        let mut output = records_to_json_full(&self.structs, &self.unions);
        let obj = output
            .as_object_mut()
            .expect("records_to_json_full returns an object");
        obj.insert("command".to_string(), Value::String(command));
        obj.insert("typedefs".to_string(), typedefs_value);
        output
    }

    /// Records (structs + unions, distinguished by `"kind"`)
    /// flattened to one row per record, ready for
    /// [`bb_sql::export_json_to_sqlite`] under the `"types"` table.
    #[must_use]
    pub fn records_as_json_rows(&self) -> Vec<Value> {
        let mut rows: Vec<Value> = self.structs.iter().map(ToJson::to_json).collect();
        rows.extend(self.unions.iter().map(ToJson::to_json));
        rows
    }

    /// Typedef hits as JSON rows, ready for SQLite export under the
    /// `"typedefs"` table.
    pub fn typedefs_as_json_rows(&self) -> anyhow::Result<Vec<Value>> {
        self.typedef_hits
            .iter()
            .map(|t| serde_json::to_value(t).map_err(anyhow::Error::from))
            .collect()
    }

    /// `WinDbg` `dt`-style plain-text rendering. Renders every struct,
    /// then every union, then a trailing typedefs section for hits not
    /// already covered by an `[aka …]` chip on a rendered record.
    #[must_use]
    pub fn format_display(
        &self,
        depth: usize,
        field_name: Option<&str>,
        typedef_index: Option<&TypedefIndex>,
    ) -> String {
        let mut out = String::new();
        for s in &self.structs {
            out.push_str(&s.display(depth, field_name, typedef_index));
        }
        for u in &self.unions {
            out.push_str(&u.display(depth, field_name, typedef_index));
        }

        if !self.typedef_hits_text.is_empty() {
            if !self.structs.is_empty() || !self.unions.is_empty() {
                out.push('\n');
            }
            out.push_str(&format!("{}\n", "typedefs".white().bold().underline()));
            for t in &self.typedef_hits_text {
                out.push_str(&format_typedef_summary(t));
                out.push('\n');
            }
        }

        out
    }
}

/// Run the full collection + auto-expansion + typedef-hit resolution
/// pipeline for one `bb-types` query.
///
/// Steps, in order:
/// 1. Collect structs and unions matching `filter`, with typedef
///    aliases attached.
/// 2. Auto-expand pointer-typedef targets: when the user searches
///    `LPSECURITY_ATTRIBUTES`, also pull in `_SECURITY_ATTRIBUTES`.
///    Same for union-typed pointer typedefs.
/// 3. Resolve typedef hits: typedefs whose name matches the pattern,
///    plus typedef aliases of any rendered record (so JSON consumers
///    always see both directions).
/// 4. Compute the filtered `typedef_hits_text` view: hide stubs whose
///    target is already rendered (the `[aka …]` chip covers it).
#[must_use]
pub fn collect_results<'tu, 'idx>(
    tu: &'tu TranslationUnit<'tu>,
    filter: &StructFilter,
    typedef_index: &'idx TypedefIndex,
) -> TypeResults<'tu, 'idx> {
    let mut structs = collect_structs(tu, filter, Some(typedef_index));
    let mut unions = collect_unions(tu, filter, Some(typedef_index));

    auto_expand_pointer_typedef_targets(tu, typedef_index, filter, &mut structs, &mut unions);

    let rendered_canonical_names: HashSet<&str> = structs
        .iter()
        .map(Struct::get_name)
        .chain(unions.iter().map(Union::get_name))
        .collect();

    let typedef_hits = collect_typedef_hits(typedef_index, filter, &rendered_canonical_names);
    let typedef_hits_text =
        suppress_redundant_typedef_stubs(&typedef_hits, &rendered_canonical_names);

    TypeResults {
        structs,
        unions,
        typedef_hits,
        typedef_hits_text,
    }
}

/// When the user's pattern matched a pointer typedef like
/// `LPSECURITY_ATTRIBUTES`, also pull in the record it points at
/// (here `_SECURITY_ATTRIBUTES`) so the layout dump is meaningful.
fn auto_expand_pointer_typedef_targets<'tu>(
    tu: &'tu TranslationUnit<'tu>,
    typedef_index: &TypedefIndex,
    filter: &StructFilter,
    structs: &mut Vec<Struct<'tu>>,
    unions: &mut Vec<Union<'tu>>,
) {
    let initial_typedef_pattern_hits = find_typedef_hits(typedef_index, filter);
    let already: HashSet<String> = structs
        .iter()
        .map(|s| s.get_name().to_string())
        .chain(unions.iter().map(|u| u.get_name().to_string()))
        .collect();

    let mut to_pull: Vec<String> = Vec::new();
    let mut seen_pull: HashSet<String> = HashSet::new();
    for td in &initial_typedef_pattern_hits {
        let candidate = td
            .canonical_decl_name
            .as_deref()
            .or(td.properties.underlying_record.as_deref());
        if let Some(record_name) = candidate
            && !already.contains(record_name)
            && seen_pull.insert(record_name.to_string())
        {
            to_pull.push(record_name.to_string());
        }
    }

    // Records have distinct namespaces in C, so a name resolves to at
    // most one of struct / union.
    for record_name in to_pull {
        if let Some(s) = find_struct_by_name(tu, &record_name, Some(typedef_index)) {
            structs.push(s);
        } else if let Some(u) = find_union_by_name(tu, &record_name, Some(typedef_index)) {
            unions.push(u);
        }
    }
}

/// Resolve every typedef the user could reasonably want surfaced:
/// pattern-name matches plus reverse aliases of every rendered record.
/// Sorted by name; deduplicated.
fn collect_typedef_hits<'idx>(
    typedef_index: &'idx TypedefIndex,
    filter: &StructFilter,
    rendered_canonical_names: &HashSet<&str>,
) -> Vec<&'idx Typedef> {
    let mut seen: HashSet<&str> = HashSet::new();
    let mut acc: Vec<&Typedef> = Vec::new();

    for t in find_typedef_hits(typedef_index, filter) {
        if seen.insert(t.name.as_str()) {
            acc.push(t);
        }
    }

    for name in rendered_canonical_names {
        for alias_name in typedef_index.aliases_for(name) {
            if let Some(t) = typedef_index.lookup(alias_name)
                && seen.insert(t.name.as_str())
            {
                acc.push(t);
            }
        }
    }

    acc.sort_by(|a, b| a.name.cmp(&b.name));
    acc
}

/// Filter typedef hits down to those worth printing in plain-text
/// output: a typedef whose target is already rendered as a record
/// shows up as the record's `[aka …]` chip, so an additional stub
/// row is noise. JSON / SQLite always keep the full list.
fn suppress_redundant_typedef_stubs<'idx>(
    hits: &[&'idx Typedef],
    rendered_canonical_names: &HashSet<&str>,
) -> Vec<&'idx Typedef> {
    hits.iter()
        .copied()
        .filter(|t| {
            let direct = t.canonical_decl_name.as_deref();
            let via_pointer = t.properties.underlying_record.as_deref();
            !direct.is_some_and(|c| rendered_canonical_names.contains(c))
                && !via_pointer.is_some_and(|c| rendered_canonical_names.contains(c))
        })
        .collect()
}

/// Print a "did-you-mean" suggestion list to stdout when a query
/// produced no records and no typedef hits. Candidates: every
/// struct name, every union name, every typedef name in the TU.
pub fn suggest_alternatives(
    tu: &TranslationUnit<'_>,
    typedef_index: &TypedefIndex,
    pattern: Option<&str>,
) {
    let struct_names: Vec<String> = iter_structs(tu).filter_map(|e| e.get_name()).collect();
    let union_names: Vec<String> = iter_unions(tu).filter_map(|e| e.get_name()).collect();
    let mut candidates: Vec<&str> = struct_names.iter().map(String::as_str).collect();
    candidates.extend(union_names.iter().map(String::as_str));
    candidates.extend(typedef_index.names());
    print_suggestions(
        "structs, unions or typedefs",
        pattern,
        candidates.into_iter(),
    );
}
