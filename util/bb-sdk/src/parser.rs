//! Parsing utilities for Windows SDK and PHNT headers.

use crate::phnt::{PhntVersion, phnt_synthetic_header};
use crate::winsdk::{SdkInfo, SdkMode, sdk_header};
use anyhow::Result;
use clang::{Index, TranslationUnit, Unsaved};
use std::path::PathBuf;

/* ──────────────────────────────── Utilities ─────────────────────────────── */

/// Parse Windows SDK headers into a [`TranslationUnit`].
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
    detailed_preprocessing: bool,
) -> Result<TranslationUnit<'a>> {
    let args_refs: Vec<&str> = args.iter().map(String::as_str).collect();
    let synthetic_path = sdk.get_include_dir().join("__bb_synthetic.h");
    let header_content = sdk_header(mode);
    let unsaved = Unsaved::new(&synthetic_path, &header_content);

    let tu = index
        .parser(synthetic_path.as_os_str())
        .arguments(&args_refs)
        .detailed_preprocessing_record(detailed_preprocessing)
        .unsaved(&[unsaved])
        .keep_going(true)
        .parse()?;

    Ok(tu)
}

/// Parse PHNT headers into a [`TranslationUnit`].
///
/// When `detailed_preprocessing` is true, the translation unit records
/// macro definitions (needed by bb-consts for `#define` extraction).
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
    let args_refs: Vec<&str> = args.iter().map(String::as_str).collect();
    let synthetic_path = PathBuf::from("__bb_phnt_synthetic.h");
    let header_content = phnt_synthetic_header(version, mode == SdkMode::Kernel);
    let unsaved = Unsaved::new(&synthetic_path, &header_content);

    let tu = index
        .parser(synthetic_path.as_os_str())
        .arguments(&args_refs)
        .detailed_preprocessing_record(detailed_preprocessing)
        .unsaved(&[unsaved])
        .keep_going(true)
        .parse()?;

    Ok(tu)
}
