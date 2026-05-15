//! Parsing utilities for Windows SDK and PHNT headers.

use std::path::PathBuf;

use anyhow::Result;
use clang::{Index, TranslationUnit, Unsaved};

use crate::HeaderConfigKind;
use crate::cache;
use crate::phnt::{PhntVersion, phnt_synthetic_header};
use crate::winsdk::{SdkInfo, SdkMode, sdk_header};

/* ──────────────────────────────── Utilities ─────────────────────────────── */

/// Parse Windows SDK headers into a [`TranslationUnit`].
///
/// Transparently consults the on-disk AST cache first
/// (`<user-cache-dir>/bb/ast/<sha256>.ast`). On a miss, parses and writes
/// the result for next time. Set `BB_NO_CACHE=1` to bypass.
///
/// When `detailed_preprocessing` is true, the translation unit records
/// macro definitions (needed by bb-consts for `#define` extraction).
///
/// # Errors
///
/// Will return an `Err` if parsing fails.
pub fn parse_winsdk<'a>(
    index: &'a Index,
    sdk: &SdkInfo,
    args: &[String],
    mode: SdkMode,
    kind: HeaderConfigKind,
    detailed_preprocessing: bool,
) -> Result<TranslationUnit<'a>> {
    let synthetic_path = sdk.get_include_dir().join("__bb_synthetic.h");
    let header_content = sdk_header(mode, kind);
    parse_with_cache(
        index,
        &synthetic_path,
        &header_content,
        args,
        detailed_preprocessing,
    )
}

/// Parse PHNT headers into a [`TranslationUnit`].
///
/// Same caching behavior as [`parse_winsdk`].
///
/// # Errors
///
/// Will return an `Err` if parsing fails.
pub fn parse_phnt<'a>(
    index: &'a Index,
    args: &[String],
    version: PhntVersion,
    mode: SdkMode,
    detailed_preprocessing: bool,
) -> Result<TranslationUnit<'a>> {
    let synthetic_path = PathBuf::from("__bb_phnt_synthetic.h");
    let header_content = phnt_synthetic_header(version, mode == SdkMode::Kernel);
    parse_with_cache(
        index,
        &synthetic_path,
        &header_content,
        args,
        detailed_preprocessing,
    )
}

/* ───────────────────────────────── Shared ────────────────────────────────── */

/// Run a fresh libclang parse against `synthetic_path` + `header_content`.
fn parse_fresh<'a>(
    index: &'a Index,
    synthetic_path: &std::path::Path,
    header_content: &str,
    args: &[String],
    detailed_preprocessing: bool,
) -> Result<TranslationUnit<'a>> {
    let args_refs: Vec<&str> = args.iter().map(String::as_str).collect();
    let unsaved = Unsaved::new(synthetic_path, header_content);
    let tu = index
        .parser(synthetic_path.as_os_str())
        .arguments(&args_refs)
        .detailed_preprocessing_record(detailed_preprocessing)
        .unsaved(&[unsaved])
        .keep_going(true)
        .parse()?;
    Ok(tu)
}

/// Cache-aware parse: try to load from disk first; on miss, parse and
/// save. Falls back to a plain parse if the cache directory isn't
/// available or `BB_NO_CACHE` is set.
fn parse_with_cache<'a>(
    index: &'a Index,
    synthetic_path: &std::path::Path,
    header_content: &str,
    args: &[String],
    detailed_preprocessing: bool,
) -> Result<TranslationUnit<'a>> {
    if cache::is_disabled() {
        return parse_fresh(
            index,
            synthetic_path,
            header_content,
            args,
            detailed_preprocessing,
        );
    }

    let Some(cache_path) = cache::cache_path(header_content, args, detailed_preprocessing) else {
        return parse_fresh(
            index,
            synthetic_path,
            header_content,
            args,
            detailed_preprocessing,
        );
    };

    if cache_path.exists()
        && let Ok(tu) = TranslationUnit::from_ast(index, &cache_path)
    {
        return Ok(tu);
    }

    let tu = parse_fresh(
        index,
        synthetic_path,
        header_content,
        args,
        detailed_preprocessing,
    )?;
    if let Err(e) = tu.save(&cache_path) {
        // Saving is best-effort. Don't fail the parse if writing fails;
        // common cause is a slow / readonly cache dir.
        eprintln!(
            "bb-sdk: failed to save AST cache to {}: {e:?}",
            cache_path.display()
        );
    }
    Ok(tu)
}
