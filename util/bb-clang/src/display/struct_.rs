//! `WinDbg` `dt`-style display rendering for struct types.

use bb_shared::glob_match;
use colored::Colorize;
use std::collections::HashSet;
use std::fmt::Write;

use crate::clang_ext::DeclarationKind;
use crate::struct_::Field;
use crate::struct_::Struct;

/// Renders a struct in `WinDbg` `dt`-style format with Unicode box-drawing.
///
/// Uses a [`HashSet<String>`] to track type names currently in the expansion path.
/// When recursing into a nested type, we add its name to the set; after returning,
/// we remove it. This prevents infinite recursion while allowing the same type
/// to appear in different branches.
pub fn render_struct(s: &Struct, depth: usize, field_filter: Option<&str>) -> String {
    let mut out = super::render_type_header(s.get_name(), s.is_anonymous(), None, s.get_location());

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
    );

    if let Some(size) = s.get_size() {
        let _ = writeln!(out, "{}", format!("╰─ {size} bytes").dimmed());
    }

    out
}

/// Renders fields with Unicode box-drawing characters in a tree structure.
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
                    "<anonymous {}>",
                    field
                        .get_type()
                        .get_declaration_kind_name()
                        .unwrap_or("type")
                )
                .dimmed()
            }
        );

        if current_depth < max_depth && field.has_children() {
            let type_key = field
                .get_underlying_type()
                .get_declaration()
                .and_then(|d| d.get_name());

            if let Some(ref key) = type_key {
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
                            None,
                            seen,
                        );
                    }
                    seen.remove(key);
                }
            }
        }
    }
}
