//! `WinDbg` `dt`-style display rendering for struct types.

use bb_shared::glob_match;
use colored::Colorize;
use std::collections::HashSet;
use std::fmt::Write;

use crate::ext::DeclarationKind;
use crate::struct_::Field;
use crate::struct_::Struct;
use crate::typedef::TypedefIndex;
use crate::union_::Union;

/// Renders a struct in `WinDbg` `dt`-style format with Unicode box-drawing.
///
/// Uses a [`HashSet<String>`] to track type names currently in the expansion path.
/// When recursing into a nested type, we add its name to the set; after returning,
/// we remove it. This prevents infinite recursion while allowing the same type
/// to appear in different branches.
///
/// `typedef_index`, when supplied, drives the dim `(canonical)` annotation
/// next to each typedef'd field type — `HANDLE (void *)`, `PVOID (void *)`,
/// `LARGE_INTEGER (_LARGE_INTEGER)`, etc. When `None`, falls back to the
/// per-field `underlying_type` metadata, which covers struct typedefs only.
#[must_use]
pub fn render_struct(
    s: &Struct,
    depth: usize,
    field_filter: Option<&str>,
    typedef_index: Option<&TypedefIndex>,
) -> String {
    let mut out = super::render_type_header(
        s.get_name(),
        s.is_anonymous(),
        None,
        s.get_aliases(),
        s.get_location(),
    );

    let mut seen = HashSet::new();
    seen.insert(s.get_name().to_string());
    write_fields(
        &mut out,
        s.get_fields(),
        depth,
        0,
        "",
        field_filter,
        &mut seen,
        typedef_index,
    );

    if let Some(size) = s.get_size() {
        let _ = writeln!(out, "{}", format!("╰─ {size} bytes").dimmed());
    }

    out
}

/// Render a [`Union`] in the same tree style as [`render_struct`].
///
/// Mirrors `render_struct` field-for-field so `bb-types -s LARGE_INTEGER`
/// produces a layout dump indistinguishable in structure from a struct
/// render — the only material difference is the kind word in the header.
#[must_use]
pub fn render_union(
    u: &Union,
    depth: usize,
    field_filter: Option<&str>,
    typedef_index: Option<&TypedefIndex>,
) -> String {
    let mut out = super::render_type_header(
        u.get_name(),
        u.is_anonymous(),
        None,
        u.get_aliases(),
        u.get_location(),
    );

    let mut seen = HashSet::new();
    seen.insert(u.get_name().to_string());
    write_fields(
        &mut out,
        u.get_fields(),
        depth,
        0,
        "",
        field_filter,
        &mut seen,
        typedef_index,
    );

    if let Some(size) = u.get_size() {
        let _ = writeln!(out, "{}", format!("╰─ {size} bytes").dimmed());
    }

    out
}

/// Resolve the dim "(canonical)" annotation for a field type, if any.
///
/// Returns `None` when there is no annotation to render (type isn't a
/// typedef, or canonical equals the displayed name). Centralized so the
/// CLI, TUI, and function param renderers all use the same logic.
///
/// The fallback `underlying_record` arg is the record/enum decl name
/// after stripping pointers/arrays (the "what struct is this?"
/// answer) — kept as a fallback for when no [`TypedefIndex`] is supplied.
/// The primitive `underlying_type` from [`TypeProperties`](crate::TypeProperties)
/// isn't useful here because it loses pointer-ness (`HANDLE`'s primitive
/// is `void`, but the user wants to see `void *`).
#[must_use]
pub fn typedef_annotation(
    type_name: &str,
    underlying_record: Option<&str>,
    typedef_index: Option<&TypedefIndex>,
) -> Option<String> {
    // 1. Prefer the typedef index: works for any kind of typedef chain.
    if let Some(idx) = typedef_index
        && let Some(td) = idx.lookup(type_name)
        && td.canonical != type_name
    {
        return Some(td.canonical.clone());
    }

    // 2. Fall back to the per-field underlying record: struct/union/enum
    //    only, but still useful when no index is wired through.
    if let Some(u) = underlying_record
        && u != type_name
    {
        return Some(u.to_string());
    }

    None
}

/// Renders fields with Unicode box-drawing characters in a tree structure.
#[allow(clippy::too_many_arguments)]
fn write_fields(
    out: &mut String,
    fields: &[Field],
    max_depth: usize,
    current_depth: usize,
    prefix: &str,
    field_filter: Option<&str>,
    seen: &mut HashSet<String>,
    typedef_index: Option<&TypedefIndex>,
) {
    let filtered: Vec<_> = fields
        .iter()
        .filter(|f| field_filter.is_none_or(|pat| glob_match(f.get_name(), pat, false)))
        .collect();

    let count = filtered.len();

    for (i, field) in filtered.iter().enumerate() {
        let is_last = i == count - 1;
        let connector = if is_last { "╰─" } else { "├─" };
        let child_prefix = if is_last { "   " } else { "│  " };

        let offset = format!("+{:#05x}", field.get_offset_bytes());
        let size = format!("{:>3}", field.get_size());
        let name = field.get_name();
        let type_name = field.get_type_name();

        // Synthetic names for nameless C-source fields are JSON-only
        // identifiers — never shown in CLI/TUI. The `<anonymous union>`
        // chip in the type column conveys the "this is anonymous"
        // signal visually.
        let name_styled = if field.is_anonymous() {
            String::new().normal()
        } else if field_filter.is_some() {
            name.white().bold().underline()
        } else {
            name.white().bold()
        };

        let type_cell = if let Some(name) = type_name {
            let underlying = field.get_type_info().underlying_record.as_deref();
            let annotation = typedef_annotation(name, underlying, typedef_index);
            match annotation {
                Some(canon) => format!("{} {}", name.cyan(), format!("({canon})").dimmed()),
                None => name.cyan().to_string(),
            }
        } else {
            format!(
                "<anonymous {}>",
                field
                    .get_type()
                    .get_declaration_kind_name()
                    .unwrap_or("type")
            )
            .dimmed()
            .to_string()
        };

        let _ = writeln!(
            out,
            "{}{} {} {} {}  {}",
            prefix,
            connector.dimmed(),
            offset.yellow(),
            format!("[{size}]").green(),
            name_styled,
            type_cell,
        );

        if current_depth < max_depth && field.has_children() {
            // Use composite identity for anonymous fields (synthetic
            // names alone collide across parents) and the underlying
            // decl name for named ones.
            let type_key = if let Some(aref) = field.get_anon_ref() {
                Some(aref.identity())
            } else {
                field
                    .get_underlying_type()
                    .get_declaration()
                    .and_then(|d| d.get_name())
            };

            if let Some(key) = type_key.as_ref()
                && seen.insert(key.clone())
            {
                let child_fields = field.get_child_fields();
                if !child_fields.is_empty() {
                    let new_prefix = format!("{prefix}{child_prefix}");
                    write_fields(
                        out,
                        &child_fields,
                        max_depth,
                        current_depth + 1,
                        &new_prefix,
                        None,
                        seen,
                        typedef_index,
                    );
                }
                seen.remove(key);
            }
        }
    }
}
