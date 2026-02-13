//! Constant type representation.
//!
//! Provides a unified type for compile-time constant values extracted from
//! C/C++ ASTs, covering enum constants, evaluable variable declarations,
//! and `#define` macros (via [`cexpr`]).

mod macro_;
mod value;

pub use value::{ConstLookup, ConstValue};

use clang::token::Token;
use clang::token::TokenKind;
use clang::{Entity, EntityKind};
use serde::Serialize;

use crate::cexpr::clang_to_cexpr_token;
use crate::error::ConstantError;
use crate::location::SourceLocation;

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
    #[serde(rename = "type")]
    type_name: Option<String>,
    location: Option<SourceLocation>,
    /// Raw macro body tokens (identifier flag + spelling). Empty for non-macros.
    #[serde(skip)]
    body_tokens: Vec<MacroBodyToken>,
}

impl<'a> Constant<'a> {
    /// Internal constructor for use by submodules.
    pub(crate) fn new(
        entity: Entity<'a>,
        name: String,
        value: ConstValue,
        type_name: Option<String>,
        location: Option<SourceLocation>,
        body_tokens: Vec<MacroBodyToken>,
    ) -> Self {
        let hex = value.to_string();
        Self {
            entity,
            name,
            value,
            hex,
            type_name,
            location,
            body_tokens,
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

    /// Raw body tokens of a macro definition (empty for non-macros).
    #[must_use]
    pub fn get_body_tokens(&self) -> &[MacroBodyToken] {
        &self.body_tokens
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
        let location = SourceLocation::from_entity(&entity);

        let (value, body_tokens) = match kind {
            EntityKind::EnumConstantDecl => {
                let (signed, unsigned) = entity
                    .get_enum_constant_value()
                    .ok_or(ConstantError::NotEvaluable)?;
                (ConstValue::from_enum_constant(signed, unsigned), Vec::new())
            }
            EntityKind::VarDecl => {
                let result = entity.evaluate().ok_or(ConstantError::NotEvaluable)?;
                let value = ConstValue::from_eval(result).ok_or(ConstantError::NotEvaluable)?;
                (value, Vec::new())
            }
            EntityKind::MacroDefinition => {
                if entity.is_function_like_macro() || entity.is_builtin_macro() {
                    return Err(ConstantError::UnsupportedMacro);
                }

                let range = entity.get_range().ok_or(ConstantError::NotEvaluable)?;
                let tokens = range.tokenize();
                let body = extract_body_tokens(&tokens);
                let cexpr_tokens: Vec<_> = tokens.iter().map(clang_to_cexpr_token).collect();

                let (_, (_, result)) = cexpr::expr::macro_definition(&cexpr_tokens)
                    .map_err(|_| ConstantError::NotEvaluable)?;

                let value = ConstValue::from_cexpr(result).ok_or(ConstantError::NotEvaluable)?;
                (value, body)
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
            body_tokens,
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
