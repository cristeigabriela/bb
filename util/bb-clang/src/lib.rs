//! Clang parsing utilities for bb.
//!
//! This crate provides abstractions for parsing C/C++ types and constants
//! from headers using libclang, with tree-style display rendering.

pub(crate) mod cexpr;
mod constant;
pub(crate) mod display;
mod enum_;
mod error;
mod field;
mod json;
pub(crate) mod location;
mod struct_;
pub mod traits;

pub use constant::{ConstLookup, ConstValue, Constant, MacroBodyToken, StripOuterParens};
pub use display::render_constants;
pub use enum_::Enum;
pub use error::{ConstantError, EnumError, FieldError, StructError};
pub use field::Field;
pub use json::ToJson;
pub use location::SourceLocation;
pub use struct_::Struct;

// Re-export commonly used clang types for convenience
pub use clang::{Entity, EntityKind, Index, TranslationUnit, Unsaved};
