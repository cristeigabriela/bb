//! Display rendering for constants and enums.

use std::fmt::Write;

use colored::Colorize;

use crate::constant::{ConstLookup, Constant, MacroBodyToken};
use crate::enum_::Enum;

/// Format macro body tokens as a colored composition string.
///
/// Resolves identifier tokens to colored `NAME=VALUE` segments.
/// Strips outer parentheses and skips non-meaningful punctuation.
/// Returns `None` if no identifiers could be resolved.
///
/// Colors follow the struct display palette:
/// - Identifier names: cyan (matching type names)
/// - Resolved values: yellow (matching offsets/values)
/// - Operators/punctuation: dimmed
/// - Unresolved literals: yellow
fn format_composition(tokens: &[MacroBodyToken], lookup: &ConstLookup) -> Option<String> {
    let has_resolvable = tokens.iter().any(
        |MacroBodyToken {
             is_identifier,
             lit_representation,
         }| *is_identifier && lookup.contains_key(lit_representation),
    );
    if !has_resolvable {
        return None;
    }

    let tokens = strip_outer_parens(tokens);

    let mut parts = Vec::new();
    for MacroBodyToken {
        is_identifier,
        lit_representation,
    } in tokens
    {
        if *is_identifier {
            if let Some(value) = lookup.get(lit_representation) {
                parts.push(format!(
                    "{}{}{}",
                    lit_representation.cyan(),
                    "=".dimmed(),
                    value.to_string().yellow()
                ));
            } else {
                parts.push(format!("{}", lit_representation.white()));
            }
        } else {
            let s = lit_representation.trim();
            if !s.is_empty() {
                parts.push(format!("{}", s.dimmed()));
            }
        }
    }

    Some(parts.join(" "))
}

/// Strip matching outer parentheses from a token slice.
fn strip_outer_parens(tokens: &[MacroBodyToken]) -> &[MacroBodyToken] {
    if tokens.len() >= 2
        && !tokens[0].is_identifier
        && tokens[0].lit_representation == "("
        && !tokens[tokens.len() - 1].is_identifier
        && tokens[tokens.len() - 1].lit_representation == ")"
    {
        &tokens[1..tokens.len() - 1]
    } else {
        tokens
    }
}

/// Render a slice of constants as a tree.
///
/// When `nested` is true (enum children), type is omitted. All items use `├─`
/// because the caller adds the closing `╰─` footer.
///
/// When `lookup` is provided, macros with identifier references show an inline
/// composition like `A=0x1 | B=0x2`.
#[must_use]
pub fn render_constants(
    constants: &[Constant],
    nested: bool,
    lookup: Option<&ConstLookup>,
) -> String {
    let mut out = String::new();
    if constants.is_empty() {
        return out;
    }

    // Calculate name maximum column width.
    let max_name = constants
        .iter()
        .map(|c| c.get_name().len())
        .max()
        .unwrap_or(0);

    // Calculate value maximum column width.
    let max_value = constants
        .iter()
        .map(|c| c.get_value().to_string().len())
        .max()
        .unwrap_or(0);

    // Calculate type maximum column width (only used when not nested).
    let max_type = constants
        .iter()
        .map(|c| c.get_type_name().map_or(1, str::len))
        .max()
        .unwrap_or(1);

    let count = constants.len();
    for (i, c) in constants.iter().enumerate() {
        let is_last = i == count - 1;
        let connector = if is_last && !nested {
            "╰─"
        } else {
            "├─"
        };

        let name_padded = format!("{:<width$}", c.get_name(), width = max_name);
        let value_str = c.get_value().to_string();

        let composition = lookup.and_then(|l| format_composition(c.get_body_tokens(), l));

        if nested {
            let _ = write!(
                out,
                "{} {}  {}",
                connector.dimmed(),
                name_padded.white().bold(),
                value_str.yellow(),
            );
        } else {
            let type_padded = format!(
                "{:<width$}",
                if c.is_macro() {
                    ""
                } else {
                    c.get_type_name().unwrap_or("?")
                },
                width = max_type
            );
            let value_padded = format!("{value_str:<max_value$}");
            let _ = write!(
                out,
                "{} {}  {}  {}",
                connector.dimmed(),
                name_padded.white().bold(),
                type_padded.cyan(),
                value_padded.yellow(),
            );

            if let Some(loc) = c.get_location() {
                let _ = write!(out, "  {}", loc.to_string().dimmed());
            }
        }

        let _ = writeln!(out);

        // Render macro expansion as a colored sub-line
        if let Some(comp) = composition {
            let prefix = if is_last && !nested { "   " } else { "│  " };
            let _ = writeln!(out, "{}{} {}", prefix.dimmed(), "╰─".dimmed(), comp);
        }
    }

    out
}

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
    let mut out = super::render_type_header(
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
