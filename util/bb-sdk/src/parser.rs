//! Parsing utilities for Windows SDK and PHNT headers.

use crate::phnt::{PhntVersion, phnt_synthetic_header};
use crate::winsdk::{SdkInfo, SdkMode, sdk_header};
use anyhow::Result;
use clang::{Entity, EntityKind, Index, TranslationUnit, Unsaved};
use std::path::PathBuf;

/// Build a [`Unsaved`] header to parse for `WinSDK` using [`crate::winsdk`] module.
pub fn parse_winsdk<'a>(
    index: &'a Index,
    sdk: &SdkInfo,
    args: &[String],
    mode: SdkMode,
) -> Result<TranslationUnit<'a>> {
    let args_refs: Vec<&str> = args.iter().map(String::as_str).collect();
    let synthetic_path = sdk.get_include_dir().join("__bb_synthetic.h");
    let unsaved = Unsaved::new(&synthetic_path, sdk_header(mode));

    let tu = index
        .parser(synthetic_path.as_os_str())
        .arguments(&args_refs)
        .unsaved(&[unsaved])
        .keep_going(true)
        .parse()?;

    Ok(tu)
}

/// Build a [`Unsaved`] header to parse for PHNT using [`crate::phnt`] module.
pub fn parse_phnt<'a>(
    index: &'a Index,
    args: &[String],
    version: PhntVersion,
    mode: SdkMode,
) -> Result<TranslationUnit<'a>> {
    let args_refs: Vec<&str> = args.iter().map(String::as_str).collect();
    let synthetic_path = PathBuf::from("__bb_phnt_synthetic.h");
    let header_content = phnt_synthetic_header(version, mode == SdkMode::Kernel);
    let unsaved = Unsaved::new(&synthetic_path, &header_content);

    let tu = index
        .parser(synthetic_path.as_os_str())
        .arguments(&args_refs)
        .unsaved(&[unsaved])
        .keep_going(true)
        .parse()?;

    Ok(tu)
}

/// Iterate over struct declarations in a [`TranslationUnit`].
pub fn iter_structs<'a>(tu: &'a TranslationUnit<'a>) -> impl Iterator<Item = Entity<'a>> {
    tu.get_entity()
        .get_children()
        .into_iter()
        .filter(|e| matches!(e.get_kind(), EntityKind::StructDecl | EntityKind::ClassDecl))
}
