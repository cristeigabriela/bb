//! Clang parsing utilities for bb.
//!
//! This crate provides abstractions for parsing C/C++ types, constants,
//! and functions from headers using libclang, with tree-style display
//! rendering and ABI-aware parameter location analysis.

mod clang_ext;
mod constant;
pub mod display;
mod enum_;
mod error;
mod function;
mod json;
pub(crate) mod location;
mod struct_;

pub use constant::{
    ConstLookup, ConstValue, Constant, MacroBodyToken, StripOuterParens, TuEntityMap,
    build_tu_entity_map,
};
pub use display::render_constants;
pub use enum_::Enum;
pub use error::{ConstantError, EnumError, FieldError, FunctionError, StructError};
pub use function::{CallConv, Function, Param};
pub use json::{ToJson, build_referred_components, collect_component_constants};
pub use location::SourceLocation;
pub use struct_::Field;
pub use struct_::Struct;

// Re-export commonly used clang types for convenience
pub use clang::{Entity, EntityKind, Index, TranslationUnit, Unsaved};
