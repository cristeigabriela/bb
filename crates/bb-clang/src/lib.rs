//! Clang parsing utilities for bb.
//!
//! This crate provides abstractions for parsing C/C++ types, constants,
//! and functions from headers using libclang, with tree-style display
//! rendering and ABI-aware parameter location analysis.
//!
//! [`TypeInfo`] is the shared type metadata struct embedded in both
//! [`Field`] and [`Param`], providing pointer/array/const classification
//! and underlying type resolution.

mod constant;
pub mod display;
mod enum_;
mod error;
mod ext;
mod function;
mod json;
pub(crate) mod location;
mod struct_;
mod type_info;

pub use constant::{
    ConstLookup, ConstValue, Constant, MacroBodyToken, StripOuterParens, TuEntityMap,
    build_tu_entity_map,
};
pub use display::render_constants;
pub use enum_::Enum;
pub use error::{ConstantError, EnumError, FieldError, FunctionError, StructError};
pub use function::{CallConv, Function, Param};
pub use json::{ToJson, build_referred_components, collect_component_constants};
pub use location::{SourceLocation, entity_in_header};
pub use struct_::Field;
pub use struct_::Struct;
pub use type_info::TypeInfo;

// Re-export commonly used clang types for convenience
pub use clang::{Entity, EntityKind, Index, TranslationUnit, Unsaved};
