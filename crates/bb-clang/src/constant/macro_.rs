//! Macro constant parsing with identifier substitution and cast stripping.
//!
//! Handles `#define` macros whose body references other named constants
//! (e.g., `#define C A | B`) by resolving each identifier recursively via
//! the translation unit entity map before [`cexpr`] evaluation.
//!
//! Also strips C-style cast expressions (e.g., `(NTSTATUS)`, `(ULONG_PTR)`)
//! that [`cexpr`] cannot evaluate, using entity kinds to distinguish
//! type names from constant names.

use std::collections::{HashMap, HashSet};

use clang::token::{Token, TokenKind};
use clang::{Entity, EntityKind, TranslationUnit};

use super::tokens::clang_to_cexpr_token;
use crate::error::ConstantError;
use crate::location::SourceLocation;

use super::value::ConstValue;
use super::{Constant, MacroBodyToken};

/* ────────────────────────────────── Types ───────────────────────────────── */

/// A map from constant name to its clang [`Entity`], covering all
/// [`EntityKind::MacroDefinition`], [`EntityKind::VarDecl`], and [`EntityKind::EnumConstantDecl`]
/// entities in a translation unit.
pub type TuEntityMap<'tu> = HashMap<String, Entity<'tu>>;

impl<'a> Constant<'a> {
    /// Recursively resolve a [`EntityKind::MacroDefinition`] into a [`Constant`].
    ///
    /// Builds a [`TuEntityMap`] from `entity`'s owning translation unit
    /// (via [`Entity::get_translation_unit`]) and delegates to
    /// [`try_from_macro_with_map`](Self::try_from_macro_with_map). Prefer
    /// that method when resolving many macros to amortize the map-build cost.
    ///
    /// # Errors
    ///
    /// - If `entity` is not a [`EntityKind::MacroDefinition`], will return [`ConstantError::NotMacroDeclaration`]
    /// - If `entity` is a function-like or builtin macro, will return [`ConstantError::UnsupportedMacro`]
    /// - If `entity` does not have a name, will return [`ConstantError::NoName`]
    /// - If `entity` has no source range, or its body cannot be reduced to a
    ///   numeric constant, will return [`ConstantError::NotEvaluable`]
    pub fn try_from_macro_recursive(entity: Entity<'a>) -> Result<Self, ConstantError> {
        let tu = entity.get_translation_unit();
        let tu_map = build_tu_entity_map(tu);
        Self::try_from_macro_with_map(entity, &tu_map)
    }

    /// Like [`try_from_macro_recursive`](Self::try_from_macro_recursive) but
    /// reuses a caller-supplied [`TuEntityMap`] instead of building one from
    /// `entity`'s translation unit. Build the map once with
    /// [`build_tu_entity_map`] and pass it here when resolving macros in bulk.
    ///
    /// # Errors
    ///
    /// - If `entity` is not a [`EntityKind::MacroDefinition`], will return [`ConstantError::NotMacroDeclaration`]
    /// - If `entity` is a function-like or builtin macro, will return [`ConstantError::UnsupportedMacro`]
    /// - If `entity` does not have a name, will return [`ConstantError::NoName`]
    /// - If `entity` has no source range, or its body cannot be reduced to a
    ///   numeric constant, will return [`ConstantError::NotEvaluable`]
    pub fn try_from_macro_with_map(
        entity: Entity<'a>,
        tu_map: &TuEntityMap<'a>,
    ) -> Result<Self, ConstantError> {
        Self::try_from_macro_impl(entity, &mut HashSet::new(), tu_map, &mut HashMap::new())
    }

    /// Internal recursive implementation. Separated so that the TU entity map
    /// is built exactly once (in [`try_from_macro_recursive`]) and reused for
    /// all recursive component resolutions. `type_alias_cache` memoizes
    /// [`is_type_alias_macro`] within a single top-level macro resolution
    /// — see TODO at module bottom about cross-macro caching.
    fn try_from_macro_impl(
        entity: Entity<'a>,
        resolving: &mut HashSet<String>,
        tu_map: &TuEntityMap<'a>,
        type_alias_cache: &mut HashMap<String, bool>,
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

        // Cycle guard.
        if !resolving.insert(name.clone()) {
            return Err(ConstantError::NotEvaluable);
        }

        let range = entity.get_range().ok_or_else(|| {
            resolving.remove(&name);
            ConstantError::NotEvaluable
        })?;
        let tokens = range.tokenize();
        let first = tokens.first().ok_or_else(|| {
            resolving.remove(&name);
            ConstantError::NotEvaluable
        })?;

        // First token is the macro name itself.
        let mut cexpr_tokens = vec![clang_to_cexpr_token(first)];
        let mut body_tokens: Vec<MacroBodyToken> = Vec::new();
        let mut component_constants: Vec<Constant<'a>> = Vec::new();
        let mut local_lookup: HashMap<String, ConstValue> = HashMap::new();
        let mut has_transform = false;

        let body = &tokens[1..];
        let mut i = 0;
        while i < body.len() {
            // Strip C-style cast patterns: `( TYPE )` where every identifier
            // inside is NOT a known constant (determined by tu_map).
            let skip = cast_len(body, i, tu_map, type_alias_cache);
            if skip > 0 {
                has_transform = true;
                i += skip;
                continue;
            }

            let token = &body[i];
            let is_identifier = token.get_kind() == TokenKind::Identifier;
            let spelling = token.get_spelling();

            if is_identifier {
                // Resolve this identifier as a constant component if not yet done.
                if !local_lookup.contains_key(&spelling)
                    && !resolving.contains(&spelling)
                    && let Some(&comp_entity) = tu_map.get(&spelling)
                {
                    let resolved = match comp_entity.get_kind() {
                        EntityKind::MacroDefinition => {
                            // Recurse: reuse the same tu_map.
                            Self::try_from_macro_impl(
                                comp_entity,
                                resolving,
                                tu_map,
                                type_alias_cache,
                            )
                            .or_else(|_| Constant::try_from(comp_entity))
                            .ok()
                        }
                        _ => Constant::try_from(comp_entity).ok(),
                    };

                    if let Some(c) = resolved {
                        local_lookup.insert(spelling.clone(), *c.get_value());
                        component_constants.push(c);
                    }
                }

                // Substitute the identifier with its resolved literal value.
                if let Some(value) = local_lookup.get(&spelling) {
                    let raw = value.as_u64().ok_or(ConstantError::NotEvaluable)?;
                    let literal = format!("0x{raw:X}");
                    cexpr_tokens.push((cexpr::token::Kind::Literal, literal.as_bytes()).into());
                    body_tokens.push(MacroBodyToken {
                        is_identifier: true,
                        lit_representation: spelling,
                    });
                    has_transform = true;
                    i += 1;
                    continue;
                }
            }

            body_tokens.push(MacroBodyToken {
                is_identifier,
                lit_representation: spelling,
            });
            cexpr_tokens.push(clang_to_cexpr_token(token));
            i += 1;
        }

        resolving.remove(&name);

        if !has_transform {
            return Err(ConstantError::NotEvaluable);
        }

        let (_, (_, result)) = cexpr::expr::macro_definition(&cexpr_tokens)
            .map_err(|_| ConstantError::NotEvaluable)?;

        let value = ConstValue::from_cexpr(result).ok_or(ConstantError::NotEvaluable)?;
        let location = SourceLocation::try_from(&entity).ok();

        let components: Vec<String> = component_constants
            .iter()
            .map(|c| c.get_name().to_string())
            .collect();

        let expression = super::expression_from_body_tokens(&body_tokens);

        Ok(Self::new(
            entity,
            name,
            value,
            type_name,
            location,
            expression,
            body_tokens,
            components,
            component_constants,
        ))
    }
}

/* ──────────────────────────────── Utilities ─────────────────────────────── */

/// Build a [`TuEntityMap`] from a translation unit, covering every
/// `MacroDefinition`, `VarDecl`, and `EnumConstantDecl` in the TU.
#[must_use]
pub fn build_tu_entity_map<'tu>(tu: &'tu TranslationUnit<'tu>) -> TuEntityMap<'tu> {
    let mut map = HashMap::new();
    for e in tu.get_entity().get_children() {
        match e.get_kind() {
            EntityKind::MacroDefinition | EntityKind::VarDecl => {
                if let Some(name) = e.get_name() {
                    map.insert(name, e);
                }
            }
            EntityKind::EnumDecl => {
                for child in e.get_children() {
                    if child.get_kind() == EntityKind::EnumConstantDecl
                        && let Some(name) = child.get_name()
                    {
                        map.insert(name, child);
                    }
                }
            }
            _ => {}
        }
    }
    map
}

/* ───────────────────────────── Cast detection ───────────────────────────── */

/// Check if a C-style cast pattern starts at position `i` in the token slice.
///
/// Returns the number of tokens to skip (including the parens), or 0 if
/// the tokens at `i` do not form a cast.
///
/// A cast is `( TYPE )` where TYPE is one or more tokens that are either:
/// - [`TokenKind::Identifier`] not present in `tu_map` (i.e. a typedef name),
/// - [`TokenKind::Identifier`] present in `tu_map` as a **type-alias macro**
///   (its body is itself made of type tokens — e.g. `#define NTSTATUS LONG`,
///   which `um/powerbase.h` emits in user-mode SDK; see
///   [`is_type_alias_macro`] for the recursive criterion),
/// - [`TokenKind::Keyword`] (e.g., `unsigned`, `long`, `int`),
/// - [`TokenKind::Punctuation`] `*` (pointer marker).
///
/// If any identifier inside the parens IS a known **value** constant
/// (present in `tu_map` with a non-type body), the parens are treated as
/// a grouped expression rather than a cast and 0 is returned.
fn cast_len(
    tokens: &[Token],
    i: usize,
    tu_map: &TuEntityMap,
    type_alias_cache: &mut HashMap<String, bool>,
) -> usize {
    if tokens[i].get_kind() != TokenKind::Punctuation || tokens[i].get_spelling() != "(" {
        return 0;
    }

    let mut j = i + 1;
    let mut has_type_token = false;

    while j < tokens.len() {
        // gabriela says:
        //
        // I would just like to apologize for this MESS!! i dont like it either!
        //
        // I wish there was a way to take the source-range and analyze everything
        // as the underlying AST entities so that I could filter all CStyleCastExpr's
        // but... there is no way afaik. so we do it the hacky way. :c

        // Strip C-style cast patterns: `( TYPE )` where TYPE is one or more
        // identifier/keyword tokens and none of the identifiers are known
        // value constants.
        let kind = tokens[j].get_kind();
        let spelling = tokens[j].get_spelling();

        if kind == TokenKind::Punctuation && spelling == ")" {
            return if has_type_token { j - i + 1 } else { 0 };
        }

        match kind {
            TokenKind::Identifier if !tu_map.contains_key(&spelling) => {
                has_type_token = true;
            }
            // Identifier IS in tu_map. Normally that means "value
            // constant — this isn't a cast". But the SDK contains
            // type-alias macros like `#define NTSTATUS LONG` (powerbase.h
            // does this when winternl.h hasn't already defined
            // `NT_SUCCESS`, which fires in phnt user mode where we strip
            // winternl.h). In that case `(NTSTATUS)` IS a valid cast —
            // the identifier expands to type tokens, not a value. Defer
            // to is_type_alias_macro for the recursive check (memoized
            // via `type_alias_cache`).
            TokenKind::Identifier => {
                if is_type_alias_macro(tu_map, &spelling, &mut HashSet::new(), type_alias_cache) {
                    has_type_token = true;
                } else {
                    return 0;
                }
            }
            TokenKind::Keyword => {
                has_type_token = true;
            }
            TokenKind::Punctuation if spelling == "*" => {
                has_type_token = true;
            }
            _ => return 0,
        }

        j += 1;
    }

    0
}

/// Whether a macro in `tu_map` named `name` is a *type-alias macro* —
/// i.e. one whose body consists entirely of type-shaped tokens
/// (keywords, pointer `*`, identifiers that are themselves either
/// not-in-`tu_map` or recursively type-alias macros). `#define NTSTATUS
/// LONG` is the canonical case: the body is the single identifier
/// `LONG`, which isn't a constant in `tu_map` — so the macro aliases
/// the type `LONG`.
///
/// `cache` memoizes results across calls within the same macro
/// resolution — `try_from_macro_impl` creates one fresh cache per
/// top-level macro and threads it through every `cast_len` invocation
/// in the body loop, avoiding repeated `entity.get_range().tokenize()`
/// calls for the same identifier (`NTSTATUS` typically shows up many
/// times in a single ntstatus.h-derived chain).
///
/// A separate `seen` set guards against pathological recursion through
/// a chain of macros that name each other. Function-like macros are
/// rejected outright — they're never types, regardless of body.
fn is_type_alias_macro(
    tu_map: &TuEntityMap,
    name: &str,
    seen: &mut HashSet<String>,
    cache: &mut HashMap<String, bool>,
) -> bool {
    if let Some(&hit) = cache.get(name) {
        return hit;
    }
    if !seen.insert(name.to_string()) {
        return false; // cycle within this resolve — don't cache (resolve-local)
    }
    let result = is_type_alias_macro_uncached(tu_map, name, seen, cache);
    cache.insert(name.to_string(), result);
    result
}

// TODO: future: replace `is_type_alias_macro` detection entirely with a
// pre-expansion pass over macro bodies. The idea (see
// https://discourse.llvm.org/t/using-clang-to-expand-macros/25402 — when
// you find a `CXCursor_MacroExpansion`, `clang_tokenize` over its extent
// returns the expanded tokens) is to substitute identifier-in-`tu_map`
// references with their resolved body tokens before `cast_len` sees the
// stream. `(NTSTATUS)0x..L` would already be `(LONG)0x..L` by the time
// cast detection runs, so cast_len doesn't need the
// type-alias-recognition branch (or this whole helper). Bigger refactor
// than the current targeted fix; flagged for later.
fn is_type_alias_macro_uncached(
    tu_map: &TuEntityMap,
    name: &str,
    seen: &mut HashSet<String>,
    cache: &mut HashMap<String, bool>,
) -> bool {
    let Some(entity) = tu_map.get(name) else {
        return false;
    };
    if entity.get_kind() != EntityKind::MacroDefinition {
        return false;
    }
    if entity.is_function_like_macro() || entity.is_builtin_macro() {
        return false;
    }
    let Some(range) = entity.get_range() else {
        return false;
    };
    let tokens = range.tokenize();
    // tokens[0] is the macro name itself. The body is the rest.
    let body = &tokens[1..];
    if body.is_empty() {
        return false;
    }
    body.iter().all(|t| match t.get_kind() {
        TokenKind::Keyword => true,
        TokenKind::Identifier => {
            let s = t.get_spelling();
            !tu_map.contains_key(&s) || is_type_alias_macro(tu_map, &s, seen, cache)
        }
        TokenKind::Punctuation if t.get_spelling() == "*" => true,
        _ => false,
    })
}
