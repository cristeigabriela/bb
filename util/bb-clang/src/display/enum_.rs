//! Display rendering for enum types.

use std::fmt::Write;

use colored::Colorize;

use crate::constant::{ConstLookup, Constant};
use crate::enum_::Enum;

use super::const_::render_constants;
use super::render_type_header;

/// Render an enum with its constants as a tree.
///
/// The underlying type is shown on the header line, not per-constant.
pub fn render_enum(e: &Enum, lookup: Option<&ConstLookup>) -> String {
    render_enum_constants(e, e.get_constants(), lookup)
}

/// Render an enum header with a custom set of constants.
///
/// Used by `display_filtered` when only a subset of constants should be shown.
pub fn render_enum_constants(
    e: &Enum,
    constants: &[Constant],
    lookup: Option<&ConstLookup>,
) -> String {
    let mut out = render_type_header(
        e.get_name(),
        e.is_anonymous(),
        e.get_type_name(),
        e.get_location(),
    );

    out.push_str(&render_constants(constants, true, lookup));

    let _ = writeln!(
        out,
        "{}",
        format!("╰─ {} constants", constants.len()).dimmed()
    );

    out
}
