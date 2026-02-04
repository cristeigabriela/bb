//! `WinDbg` `dt`-style display rendering for struct types.
//!
//! This module handles the visual representation of structs using Unicode
//! box-drawing characters in a tree structure.

use crate::matcher::glob_match;
use colored::Colorize;
use std::collections::HashSet;
use std::fmt::Write;

use super::traits::DeclarationKind;
use super::{Field, SourceLocation};

/// Renders a struct in `WinDbg` `dt`-style format with Unicode box-drawing.
///
/// # Arguments
///
/// * `name` - The struct name
/// * `location` - Optional source location (file, line, column)
/// * `size` - Optional total size in bytes
/// * `fields` - The struct's fields
/// * `depth` - Maximum recursion depth for expanding nested types (0 = no expansion)
/// * `field_filter` - Optional glob pattern to filter which fields are displayed
///
/// # Cycle Detection
///
/// Uses a [`HashSet<String>`] to track type names currently in the expansion path.
/// When recursing into a nested type, we add its name to the set; after returning,
/// we remove it. This prevents infinite recursion for:
///
/// - Direct self-references (e.g., `LIST_ENTRY` contains `PLIST_ENTRY`)
/// - Indirect cycles (e.g., `A` → `B` → `A`)
/// - Diamond patterns are handled correctly (same type via different paths expands once per branch)
pub fn render_struct(
    name: &str,
    location: Option<&SourceLocation>,
    size: Option<usize>,
    fields: &[Field],
    depth: usize,
    field_filter: Option<&str>,
) -> String {
    let mut out = String::new();

    let loc_info = location
        .map(|loc| format!(" {}", loc.display_short().dimmed()))
        .unwrap_or_default();
    let _ = writeln!(out, "{}{}", name.cyan().bold(), loc_info);

    // Initialize the seen set with the root struct name to prevent self-reference cycles
    let mut seen = HashSet::new();
    seen.insert(name.to_string());
    write_fields(&mut out, fields, depth, 0, "", field_filter, &mut seen);

    if let Some(size) = size {
        let _ = writeln!(out, "{}", format!("╰─ {size} bytes").dimmed());
    }

    out
}

/// Renders fields with Unicode box-drawing characters in a tree structure.
///
/// # Arguments
///
/// * `out` - Output string buffer
/// * `fields` - Fields to render
/// * `max_depth` - Maximum recursion depth for nested types
/// * `current_depth` - Current depth in the recursion
/// * `prefix` - Indentation prefix for the current level (contains box chars like "│  ")
/// * `field_filter` - Optional glob pattern to filter fields (only applied at root level)
/// * `seen` - Set of type names currently in the ancestor chain (for cycle detection)
///
/// # Cycle Detection Strategy
///
/// We use `seen.insert()` before recursing and `seen.remove()` after returning.
/// This tracks only the current path from root to leaf, not all visited types.
///
/// Why remove after recursing? Consider this structure:
/// ```text
/// Root
/// ├─ field_a: TypeX
/// │  └─ nested: TypeY
/// └─ field_b: TypeX  ← Should still expand, even though we saw TypeX in field_a
/// ```
///
/// If we never removed from `seen`, `field_b` wouldn't expand `TypeX`.
/// By removing after returning, we allow the same type to appear in different branches,
/// while still preventing cycles within a single branch.
fn write_fields(
    out: &mut String,
    fields: &[Field],
    max_depth: usize,
    current_depth: usize,
    prefix: &str,
    field_filter: Option<&str>,
    seen: &mut HashSet<String>,
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

        let name_styled = if field_filter.is_some() {
            name.white().bold().underline()
        } else {
            name.white().bold()
        };

        let _ = writeln!(
            out,
            "{}{} {} {} {}  {}",
            prefix,
            connector.dimmed(),
            offset.yellow(),
            format!("[{size}]").green(),
            name_styled,
            if let Some(name) = type_name {
                name.cyan()
            } else {
                format!(
                    "<anonymous {:?}>",
                    field.get_type().get_declaration_kind_name().unwrap()
                )
                .dimmed()
            }
        );

        // Recurse into child fields if:
        // 1. We haven't exceeded max_depth
        // 2. The field's type has expandable children
        // 3. The type is not already in our current ancestor chain (cycle detection)
        if current_depth < max_depth && field.has_children() {
            let type_key = field
                .get_underlying_type()
                .get_declaration()
                .and_then(|d| d.get_name());

            if let Some(ref key) = type_key {
                // seen.insert() returns true if the key was NOT already present
                if seen.insert(key.clone()) {
                    let child_fields = field.get_child_fields();
                    if !child_fields.is_empty() {
                        let new_prefix = format!("{prefix}{child_prefix}");
                        write_fields(
                            out,
                            &child_fields,
                            max_depth,
                            current_depth + 1,
                            &new_prefix,
                            None, // Field filter only applies at root level
                            seen,
                        );
                    }
                    // Remove from seen after recursion completes, allowing the same
                    // type to appear in sibling branches of the tree
                    seen.remove(key);
                }
            }
        }
    }
}
