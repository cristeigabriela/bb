//! SDK and PHNT integration for bb.
//!
//! This crate provides Windows SDK and PHNT header management,
//! including header generation and parsing utilities.

mod arch;
mod config;
mod parser;
mod phnt;
mod winsdk;

// High-level API
pub use config::HeaderConfig;

// Architecture
pub use arch::Arch;

// Parsing utilities
pub use parser::{parse_phnt, parse_winsdk};

// PHNT
pub use phnt::{PHNT_HEADER, PhntVersion, phnt_synthetic_header};

// Windows SDK
pub use winsdk::{SdkInfo, SdkMode, check_wdk_installed, get_sdk_info, sdk_header};

// Re-export bb-clang types for convenience
pub use bb_clang::{Field, FieldError, Struct, StructError};
