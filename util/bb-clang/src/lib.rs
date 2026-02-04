//! Clang parsing utilities for bb.
//!
//! This crate provides abstractions for parsing C/C++ struct definitions
//! using libclang and rendering them in a `WinDbg` `dt`-style format.

mod display;
mod error;
mod field;
mod struct_;
pub mod traits;

pub use error::ParseError;
pub use field::Field;
pub use struct_::{SourceLocation, Struct};

// Re-export commonly used clang types for convenience
pub use clang::{Entity, EntityKind, Index, TranslationUnit, Unsaved};
