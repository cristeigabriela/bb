//! SDK and PHNT integration for bb.
//!
//! This crate provides Windows SDK and PHNT header management,
//! including header generation and parsing utilities.
//!
//! # Quick Start
//!
//! The easiest way to get started is using [`HeaderConfig`]:
//!
//! ```no_run
//! use bb_sdk::{HeaderConfig, Arch, SdkMode, iter_structs};
//! use bb_clang::Struct;
//! use clang::{Clang, Index};
//!
//! fn main() -> anyhow::Result<()> {
//!     // Create a configuration for parsing WinSDK headers
//!     let config = HeaderConfig::winsdk(Arch::Amd64, SdkMode::User)?;
//!
//!     // Set up Clang and parse
//!     let clang = Clang::new().expect("failed to initialize clang");
//!     let index = Index::new(&clang, false, false);
//!     let tu = config.parse(&index)?;
//!
//!     // Iterate over struct declarations
//!     for entity in iter_structs(&tu) {
//!         if let Ok(s) = Struct::try_from(entity) {
//!             println!("{}", s.get_name());
//!         }
//!     }
//!     Ok(())
//! }
//! ```

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
pub use parser::{iter_structs, parse_phnt, parse_winsdk};

// PHNT
pub use phnt::{PHNT_HEADER, PhntVersion, phnt_synthetic_header};

// Windows SDK
pub use winsdk::{SdkInfo, SdkMode, check_wdk_installed, get_sdk_info, sdk_header};

// Re-export bb-clang types for convenience
pub use bb_clang::{Field, ParseError, Struct};
