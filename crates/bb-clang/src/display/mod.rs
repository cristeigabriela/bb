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
mod typedef;

pub use bb_arch::display::register_name;
pub use constant::render_constants;
pub use enum_::{render_enum, render_enum_constants};
pub use function::{
    format_abi_param, format_arch, format_callconv, format_location, format_operand,
    format_return_location, format_tags, render_function_detail, render_function_item,
    render_function_list,
};
pub use struct_::{render_struct, render_union, typedef_annotation};
pub use typedef::format_typedef_summary;

/// Render a type header line: styled name + optional type info +
/// optional aliases chip + optional location.
///
/// Anonymous names are dimmed; named types are cyan + bold. `type_name`,
/// when given, is rendered in cyan (used for enum underlying-type tags).
/// `aliases`, when non-empty, is rendered as a dim `[aka X, Y]` chip after
/// the type info (used for struct typedef aliases).
#[must_use]
pub fn render_type_header(
    name: &str,
    is_anonymous: bool,
    type_name: Option<&str>,
    aliases: &[String],
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
    let alias_chip = if aliases.is_empty() {
        String::new()
    } else {
        format!("  {}", format!("[aka {}]", aliases.join(", ")).dimmed())
    };
    let loc_info = location
        .map(|loc| format!(" {}", loc.to_string().dimmed()))
        .unwrap_or_default();
    format!("{name_styled}{type_info}{alias_chip}{loc_info}\n")
}
