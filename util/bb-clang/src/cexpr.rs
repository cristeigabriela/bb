//! Utilities for [`cexpr`]-[`clang`] interop.

/// Convert a [`clang::token::Token`] to a [`cexpr::token::Token`].
pub fn clang_to_cexpr_token(token: &clang::token::Token) -> cexpr::token::Token {
    let kind = match token.get_kind() {
        clang::token::TokenKind::Comment => cexpr::token::Kind::Comment,
        clang::token::TokenKind::Identifier => cexpr::token::Kind::Identifier,
        clang::token::TokenKind::Keyword => cexpr::token::Kind::Keyword,
        clang::token::TokenKind::Literal => cexpr::token::Kind::Literal,
        clang::token::TokenKind::Punctuation => cexpr::token::Kind::Punctuation,
    };

    // Create [`cexpr::token::Token`] from the [`cexpr::token::Kind`] and the
    // [`clang::token::Token`]'s textual representation.
    (kind, token.get_spelling().as_bytes()).into()
}
