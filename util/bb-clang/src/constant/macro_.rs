//! Macro constant parsing with identifier substitution and cast stripping.
//!
//! Handles `#define` macros whose body references other named constants
//! (e.g., `#define C A | B`) by substituting known values before [`cexpr`]
//! evaluation.
//!
//! Also strips C-style cast expressions (e.g., `(NTSTATUS)`, `(ULONG_PTR)`)
//! that [`cexpr`] cannot evaluate, using the lookup table to distinguish
//! type names from constant names.

use clang::token::{Token, TokenKind};
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
    /// This also strips C-style cast patterns (`(TYPE)`) from the token stream
    /// before evaluation, where `TYPE` is an identifier not present in the lookup
    /// table (i.e., a type name rather than a constant). This handles macros like
    /// `#define STATUS_SUCCESS ((NTSTATUS)0x00000000L)`.
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

        // First token is the macro name.
        let mut cexpr_tokens = vec![clang_to_cexpr_token(&tokens[0])];
        let mut body_tokens = Vec::new();
        let mut has_transform = false;

        // Process body tokens (everything after the macro name).
        let body = &tokens[1..];
        let mut i = 0;
        while i < body.len() {
            // gabriela says:
            //
            // I would just like to apologize for this MESS!! i dont like it either!
            //
            // I wish there was a way to take the source-range and analyze everything
            // as the underlying AST entities so that I could filter all CStyleCastExpr's
            // but... there is no way afaik. so we do it the hacky way. :c
            //
            // Strip C-style cast patterns: `( TYPE )` where TYPE is one or more
            // identifier/keyword tokens and none of the identifiers are known
            // constants. This handles macros like:
            //   #define STATUS_SUCCESS   ((NTSTATUS)0x00000000L)
            //   #define INVALID_HANDLE   ((HANDLE)(LONG_PTR)-1)
            let skip = cast_len(body, i, lookup);
            if skip > 0 {
                has_transform = true;
                i += skip;
                continue;
            }

            let token = &body[i];
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
                    has_transform = true;
                    i += 1;
                    continue;
                }
            }

            body_tokens.push(MacroBodyToken {
                is_identifier,
                lit_representation,
            });
            cexpr_tokens.push(clang_to_cexpr_token(token));
            i += 1;
        }

        if !has_transform {
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

/* ───────────────────────────── Cast detection ───────────────────────────── */

/// Check if a C-style cast pattern starts at position `i` in the token slice.
///
/// Returns the number of tokens to skip (including the parens), or 0 if
/// the tokens at `i` do not form a cast.
///
/// A cast is `( TYPE )` where TYPE is one or more tokens that are either:
/// - [`TokenKind::Identifier`] not present in the lookup (i.e., a type name),
/// - [`TokenKind::Keyword`] (e.g., `unsigned`, `long`, `int`),
/// - [`TokenKind::Punctuation`] `*` (pointer marker).
///
/// If any identifier inside the parens IS a known constant, it's treated as
/// a grouped expression (not a cast) and 0 is returned.
fn cast_len(tokens: &[Token], i: usize, lookup: &ConstLookup) -> usize {
    // Must start with `(`
    if tokens[i].get_kind() != TokenKind::Punctuation || tokens[i].get_spelling() != "(" {
        return 0;
    }

    let mut j = i + 1;
    let mut has_type_token = false;

    while j < tokens.len() {
        let kind = tokens[j].get_kind();
        let spelling = tokens[j].get_spelling();

        // Closing paren: if we saw at least one type token, it's a cast.
        if kind == TokenKind::Punctuation && spelling == ")" {
            return if has_type_token { j - i + 1 } else { 0 };
        }

        match kind {
            // Identifier not in the lookup → type name (e.g. NTSTATUS, HANDLE)
            TokenKind::Identifier if !lookup.contains_key(&spelling) => {
                has_type_token = true;
            }
            // Identifier that IS in the lookup → it's a constant, not a cast
            TokenKind::Identifier => return 0,
            // C keywords are always type-related inside parens (unsigned, long, ...)
            TokenKind::Keyword => {
                has_type_token = true;
            }
            // Pointer marker is part of a type (e.g. `void *`)
            TokenKind::Punctuation if spelling == "*" => {
                has_type_token = true;
            }
            // Anything else (operator, literal, etc.) → not a cast
            _ => return 0,
        }

        j += 1;
    }

    0 // unclosed paren
}
