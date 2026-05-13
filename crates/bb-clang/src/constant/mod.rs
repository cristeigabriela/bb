//! Constant type representation.
//!
//! Provides a unified type for compile-time constant values extracted from
//! C/C++ ASTs, covering enum constants, evaluable variable declarations,
//! and `#define` macros (via [`cexpr`]).

mod macro_;
mod tokens;
mod value;

pub use macro_::{TuEntityMap, build_tu_entity_map};
pub use value::{ConstLookup, ConstValue};

use clang::source::SourceRange;
use clang::token::Token;
use clang::token::TokenKind;
use clang::{Entity, EntityKind};
use serde::Serialize;

use crate::error::ConstantError;
use crate::location::SourceLocation;
use tokens::clang_to_cexpr_token;

/* ────────────────────────────────── Macro ───────────────────────────────── */

/// A macro body token.
#[derive(Debug, Clone, Serialize)]
pub struct MacroBodyToken {
    /// Whether the token is of [`TokenKind::Identifier`].
    pub is_identifier: bool,
    /// The literal representation of the token.
    pub lit_representation: String,
}

/* ────────────────────────────────── Types ───────────────────────────────── */

/// A compile-time constant extracted from a C/C++ AST entity.
///
/// This representation might only be constructed from:
/// - [`EntityKind::EnumConstantDecl`] - an enum field value;
/// - [`EntityKind::VarDecl`] - a `const`/`constexpr`/`static const` variable
///   whose value clang can evaluate at compile time;
/// - [`EntityKind::MacroDefinition`] - a `#define` whose body evaluates to a
///   numeric constant (via the [`cexpr`] crate).
#[derive(Debug, Clone, Serialize)]
pub struct Constant<'a> {
    #[serde(skip)]
    entity: Entity<'a>,
    name: String,
    value: ConstValue,
    hex: String,
    #[serde(rename = "type", skip_serializing_if = "Option::is_none")]
    type_name: Option<String>,
    location: Option<SourceLocation>,
    /// The original C expression text (e.g. `(0x00000001L | 0x00000002L)`).
    #[serde(skip_serializing_if = "Option::is_none")]
    expression: Option<String>,
    /// Raw macro body tokens (identifier flag + spelling). Empty for non-macros.
    #[serde(skip)]
    body_tokens: Vec<MacroBodyToken>,
    /// Names of other constants this macro is composed of.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    components: Vec<String>,
    /// Fully resolved [`Constant`] objects for each name in [`components`](Self::components).
    /// Skipped during serialization; used by [`ToJson::to_json_full`] to emit
    /// `referred_components` at the root JSON level.
    #[serde(skip)]
    component_constants: Vec<Constant<'a>>,
}

impl<'a> Constant<'a> {
    /// Internal constructor for use by submodules.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn new(
        entity: Entity<'a>,
        name: String,
        value: ConstValue,
        type_name: Option<String>,
        location: Option<SourceLocation>,
        expression: Option<String>,
        body_tokens: Vec<MacroBodyToken>,
        components: Vec<String>,
        component_constants: Vec<Self>,
    ) -> Self {
        let hex = value.to_string();
        Self {
            entity,
            name,
            value,
            hex,
            type_name,
            location,
            expression,
            body_tokens,
            components,
            component_constants,
        }
    }

    #[must_use]
    pub const fn get_entity(&self) -> &Entity<'a> {
        &self.entity
    }

    #[must_use]
    pub fn get_name(&self) -> &str {
        &self.name
    }

    #[must_use]
    pub const fn get_value(&self) -> &ConstValue {
        &self.value
    }

    #[must_use]
    pub fn is_macro(&self) -> bool {
        matches!(self.get_entity().get_kind(), EntityKind::MacroDefinition)
    }

    #[must_use]
    pub fn is_var(&self) -> bool {
        matches!(self.get_entity().get_kind(), EntityKind::VarDecl)
    }

    #[must_use]
    pub fn is_enum_child(&self) -> bool {
        matches!(self.get_entity().get_kind(), EntityKind::EnumConstantDecl)
    }

    #[must_use]
    pub fn get_type_name(&self) -> Option<&str> {
        self.type_name.as_deref()
    }

    #[must_use]
    pub const fn get_location(&self) -> Option<&SourceLocation> {
        self.location.as_ref()
    }

    /// The original C expression text, if available.
    #[must_use]
    pub fn get_expression(&self) -> Option<&str> {
        self.expression.as_deref()
    }

    /// Raw body tokens of a macro definition (empty for non-macros).
    #[must_use]
    pub fn get_body_tokens(&self) -> &[MacroBodyToken] {
        &self.body_tokens
    }

    /// Names of other constants this macro is composed of.
    #[must_use]
    pub fn get_components(&self) -> &[String] {
        &self.components
    }

    /// Fully resolved [`Constant`] objects for each entry in [`get_components`](Self::get_components).
    ///
    /// Used by [`ToJson::to_json_full`] to emit `referred_components`.
    #[must_use]
    pub fn get_component_constants(&self) -> &[Self] {
        &self.component_constants
    }
}

/* ─────────────────────────────── Conversions ────────────────────────────── */

/// Attempt to generate [`Constant`] from supported entities.
impl<'a> TryFrom<Entity<'a>> for Constant<'a> {
    type Error = ConstantError;

    /// Attempt to generate a [`Constant`] from an [`EntityKind::EnumConstantDecl`], an evaluable
    /// [`EntityKind::VarDecl`], or a simple [`EntityKind::MacroDefinition`].
    ///
    /// Support for [`EntityKind::MacroDefinition`] parsing and processing comes from the use
    /// of the [`cexpr`] crate.
    fn try_from(entity: Entity<'a>) -> Result<Self, Self::Error> {
        let kind = entity.get_kind();
        let name = entity.get_name().ok_or(ConstantError::NoName)?;
        let type_name = entity.get_type().map(|t| t.get_display_name());
        let location = SourceLocation::try_from(&entity).ok();

        let (value, expression, body_tokens) = match kind {
            EntityKind::EnumConstantDecl => {
                let (signed, unsigned) = entity
                    .get_enum_constant_value()
                    .ok_or(ConstantError::NotEvaluable)?;
                let expr = extract_expression_from_entity(&entity);
                (
                    ConstValue::from_enum_constant(signed, unsigned),
                    expr,
                    Vec::new(),
                )
            }
            EntityKind::VarDecl => {
                let result = entity.evaluate().ok_or(ConstantError::NotEvaluable)?;
                let value = ConstValue::from_eval(result).ok_or(ConstantError::NotEvaluable)?;
                let expr = extract_expression_from_entity(&entity);
                (value, expr, Vec::new())
            }
            EntityKind::MacroDefinition => {
                if entity.is_function_like_macro() || entity.is_builtin_macro() {
                    return Err(ConstantError::UnsupportedMacro);
                }

                let range = entity.get_range().ok_or(ConstantError::NotEvaluable)?;
                let tokens = safe_tokenize(&range).ok_or(ConstantError::NotEvaluable)?;
                let body = extract_body_tokens(&tokens);
                let cexpr_tokens: Vec<_> = tokens.iter().map(clang_to_cexpr_token).collect();

                let (_, (_, result)) = cexpr::expr::macro_definition(&cexpr_tokens)
                    .map_err(|_| ConstantError::NotEvaluable)?;

                let value = ConstValue::from_cexpr(result).ok_or(ConstantError::NotEvaluable)?;
                let expr = expression_from_body_tokens(&body);
                (value, expr, body)
            }
            _ => return Err(ConstantError::NotConstant(kind)),
        };

        let hex = value.to_string();
        Ok(Self {
            entity,
            name,
            value,
            hex,
            type_name,
            location,
            expression,
            body_tokens,
            components: Vec::new(),
            component_constants: Vec::new(),
        })
    }
}

/* ──────────────────────────────── Utilities ─────────────────────────────── */

/// Extract macro body tokens as [`MacroBodyToken`].
fn extract_body_tokens(tokens: &[Token]) -> Vec<MacroBodyToken> {
    tokens
        .iter()
        .skip(1) // skip macro name
        .map(|t| MacroBodyToken {
            is_identifier: t.get_kind() == TokenKind::Identifier,
            lit_representation: t.get_spelling(),
        })
        .collect()
}

/// Reconstruct the C expression string from macro body tokens.
///
/// Spaces are inserted only between two "word" tokens (identifiers or
/// number literals). Punctuation and brackets bind tightly:
/// `(DWORD)(FOO | BAR)` not `( DWORD ) ( FOO | BAR )`.
pub(crate) fn expression_from_body_tokens(tokens: &[MacroBodyToken]) -> Option<String> {
    if tokens.is_empty() {
        return None;
    }
    let mut out = String::new();
    for (i, t) in tokens.iter().enumerate() {
        let s = t.lit_representation.as_str();
        if i > 0 && needs_space(tokens[i - 1].lit_representation.as_str(), s) {
            out.push(' ');
        }
        out.push_str(s);
    }
    let trimmed = out.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}

/// Whether a space is needed between two adjacent tokens.
///
/// Space is inserted only when both tokens are "words" (identifiers,
/// numbers, keywords). Punctuation tokens never get leading/trailing spaces.
fn needs_space(prev: &str, cur: &str) -> bool {
    is_word_token(prev) && is_word_token(cur)
}

fn is_word_token(s: &str) -> bool {
    s.bytes()
        .next()
        .is_some_and(|b| b.is_ascii_alphanumeric() || b == b'_')
}

/// Extract the C expression from an entity's token range (for enum constants and var decls).
///
/// For enum constants, skips the name and `=` prefix.
/// For var decls, skips everything up to and including `=`.
fn extract_expression_from_entity(entity: &Entity) -> Option<String> {
    let range = entity.get_range()?;
    let tokens = safe_tokenize(&range)?;
    // Find the `=` separator and take everything after it.
    let eq_pos = tokens.iter().position(|t| t.get_spelling() == "=")?;
    let expr_tokens: Vec<_> = tokens[eq_pos + 1..]
        .iter()
        .map(clang::token::Token::get_spelling)
        // Skip trailing semicolons (var decls).
        .filter(|s| s != ";")
        .collect();
    if expr_tokens.is_empty() {
        return None;
    }
    let mut out = String::new();
    for (i, s) in expr_tokens.iter().enumerate() {
        if i > 0 && needs_space(expr_tokens[i - 1].as_str(), s) {
            out.push(' ');
        }
        out.push_str(s);
    }
    Some(out)
}

/* ────────────────────────────── safe_tokenize ──────────────────────────── */

/// Wrap [`SourceRange::tokenize`] to drop ranges where `clang_tokenize`
/// returns zero tokens.
///
/// **Why:** clang-rs 2.0.0 reads uninitialized memory for the token array
/// pointer when `clang_tokenize` writes `count == 0`. Rust 1.78+'s
/// `slice::from_raw_parts` precondition check turns that into a
/// non-unwinding abort (uncatchable by `catch_unwind`). The trigger we've
/// seen in kernel-mode parses is *inverted* source ranges (`end.offset <
/// start.offset`), produced by some synthesized/intrinsic declarations in
/// `wdm.h` / `ntddk.h`. Detecting `end <= start` (within a single file)
/// before calling `.tokenize()` avoids the bad branch entirely.
///
/// Returns `None` when the range is missing a file, spans multiple files,
/// or is empty/inverted.
pub(crate) fn safe_tokenize<'tu>(range: &SourceRange<'tu>) -> Option<Vec<Token<'tu>>> {
    let start = range.get_start().get_file_location();
    let end = range.get_end().get_file_location();
    // Both endpoints must be in the same file, and the range must cover at
    // least one character.
    match (start.file, end.file) {
        (Some(sf), Some(ef)) if sf == ef && end.offset > start.offset => Some(range.tokenize()),
        _ => None,
    }
}

/* ───────────────────────────── Type utilities ───────────────────────────── */

/// Strip matching outer parentheses from a macro body token slice.
pub trait StripOuterParens {
    fn strip_outer_parens(&self) -> &[MacroBodyToken];
}

impl StripOuterParens for [MacroBodyToken] {
    fn strip_outer_parens(&self) -> &[MacroBodyToken] {
        if self.len() >= 2
            && !self[0].is_identifier
            && self[0].lit_representation == "("
            && !self[self.len() - 1].is_identifier
            && self[self.len() - 1].lit_representation == ")"
        {
            &self[1..self.len() - 1]
        } else {
            self
        }
    }
}
