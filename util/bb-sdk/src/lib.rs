//! SDK and PHNT integration for bb.
//!
//! This crate provides Windows SDK and PHNT header management,
//! including header generation and parsing utilities.

mod arch;
mod parser;
mod phnt;
mod winsdk;

pub use arch::Arch;
pub use parser::{iter_structs, parse_phnt, parse_winsdk};
pub use phnt::{PHNT_HEADER, PhntVersion, phnt_synthetic_header};
pub use winsdk::{SdkInfo, SdkMode, check_wdk_installed, get_sdk_info, sdk_header};

// Re-export bb-clang types for convenience
pub use bb_clang::{Field, ParseError, Struct};
