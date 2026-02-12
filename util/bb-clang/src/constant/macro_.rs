//! Macro constant parsing with identifier substitution.
//!
//! Handles `#define` macros whose body references other named constants
//! (e.g., `#define C A | B`) by substituting known values before [`cexpr`]
//! evaluation.

use clang::token::TokenKind;
use clang::{Entity, EntityKind};

use crate::cexpr::clang_to_cexpr_token;
use crate::error::ConstantError;
use crate::location::SourceLocation;

use super::value::ConstValue;
use super::{ConstLookup, Constant, MacroBodyToken};

impl<'a> Constant<'a> {
    /// Try to build a [`Constant`] from a [`EntityKind::MacroDefinition`] by substituting
    /// known identifier references with their literal values before evaluation.
    ///
    /// Use this for macros whose body references other named constants
    /// (e.g., `#define C A | B`). The body tokens are stored for display-time
    /// composition rendering.
    ///
    /// This is necessary because [`cexpr`] does not have the actual Clang context,
    /// or translation unit, and thus cannot evaluate other macros.
    ///
    /// # Arguments
    ///
    /// * `entity` - Expected to be a [`EntityKind::MacroDefinition`] entity.
    /// * `lookup` - A name-value lookup value of macros.
    ///
    /// # Errors
    ///
    /// - If the `entity` is not a [`EntityKind::MacroDefinition`], will return [`ConstantError::NotMacroDeclaration`].
    /// - If the `entity` is a complex or builtin macro, will return [`ConstantError::UnsupportedMacro`].
    /// - If the `entity` does not have a name, will return [`ConstantError::NoName`]
    /// - If the `entity` does not have a [`clang::SourceRange`], will return [`ConstantError::NotEvaluable`].
    /// - If the `entity` (before or after substitution) cannot be fully evaluated because of missing macro substitutions, will return [`ConstantError::NotEvaluable`].
    /// - If the `entity` cannot be fully evaluated using [`cexpr`], will return [`ConstantError::NotEvaluable`].
    pub fn try_from_macro_with_lookup(
        entity: Entity<'a>,
        lookup: &ConstLookup,
    ) -> Result<Self, ConstantError> {
        let kind = entity.get_kind();
        if kind != EntityKind::MacroDefinition {
            return Err(ConstantError::NotMacroDeclaration(kind));
        }
        if entity.is_function_like_macro() || entity.is_builtin_macro() {
            return Err(ConstantError::UnsupportedMacro);
        }

        let name = entity.get_name().ok_or(ConstantError::NoName)?;
        let type_name = entity.get_type().map(|t| t.get_display_name());

        let range = entity.get_range().ok_or(ConstantError::NotEvaluable)?;
        let tokens = range.tokenize();

        // Build body tokens for display and substituted tokens for evaluation.
        let mut cexpr_tokens = Vec::new();
        let mut body_tokens = Vec::new();
        let mut has_substitution = false;

        for (i, token) in tokens.iter().enumerate() {
            if i == 0 {
                // First token is the macro name
                cexpr_tokens.push(clang_to_cexpr_token(token));
                continue;
            }

            let is_identifier = token.get_kind() == TokenKind::Identifier;
            let lit_representation = token.get_spelling();

            if is_identifier {
                if let Some(value) = lookup.get(&lit_representation) {
                    // Substitute identifier with its literal value for cexpr
                    let raw = value.as_u64().ok_or(ConstantError::NotEvaluable)?;
                    let literal = format!("0x{raw:X}");
                    cexpr_tokens.push((cexpr::token::Kind::Literal, literal.as_bytes()).into());
                    body_tokens.push(MacroBodyToken {
                        is_identifier,
                        lit_representation,
                    });
                    has_substitution = true;
                    continue;
                }
            }

            body_tokens.push(MacroBodyToken {
                is_identifier,
                lit_representation,
            });
            cexpr_tokens.push(clang_to_cexpr_token(token));
        }

        if !has_substitution {
            return Err(ConstantError::NotEvaluable);
        }

        let (_, (_, result)) = cexpr::expr::macro_definition(&cexpr_tokens)
            .map_err(|_| ConstantError::NotEvaluable)?;

        let value = ConstValue::from_cexpr(result).ok_or(ConstantError::NotEvaluable)?;
        let location = SourceLocation::from_entity(&entity);

        Ok(Self::new(
            entity,
            name,
            value,
            type_name,
            location,
            body_tokens,
        ))
    }
}
