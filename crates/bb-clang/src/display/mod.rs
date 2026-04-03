//! Display rendering for bb-clang types.
//!
//! Provides tree-style rendering with Unicode box-drawing characters
//! for structs, enums, constants, and functions.

use colored::Colorize;

use crate::location::SourceLocation;

mod constant;
mod enum_;
mod function;
mod struct_;

pub use bb_arch::display::register_name;
pub use constant::render_constants;
pub use enum_::{render_enum, render_enum_constants};
pub use function::{
    format_abi_param, format_arch, format_callconv, format_location, format_operand,
    format_return_location, format_tags, render_function_detail, render_function_item,
    render_function_list,
};
pub use struct_::render_struct;

/// Render a type header line: styled name + optional type info + optional location.
///
/// Anonymous names are dimmed; named types are cyan + bold.
#[must_use]
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
