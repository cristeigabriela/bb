//! Display rendering for bb-clang types.
//!
//! Provides tree-style rendering with Unicode box-drawing characters
//! for structs, enums, and constants.

use colored::Colorize;

use crate::location::SourceLocation;

mod const_;
mod enum_;
mod struct_;

pub use const_::render_constants;
pub use enum_::{render_enum, render_enum_constants};
pub use struct_::render_struct;

/// Render a type header line: styled name + optional type info + optional location.
///
/// Anonymous names are dimmed; named types are cyan + bold.
pub fn render_type_header(
    name: &str,
    is_anonymous: bool,
    type_name: Option<&str>,
    location: Option<&SourceLocation>,
) -> String {
    let name_styled = if is_anonymous {
        name.dimmed()
    } else {
        name.cyan().bold()
    };
    let type_info = type_name
        .map(|t| format!("  {}", t.cyan()))
        .unwrap_or_default();
    let loc_info = location
        .map(|loc| format!(" {}", loc.to_string().dimmed()))
        .unwrap_or_default();
    format!("{name_styled}{type_info}{loc_info}\n")
}
