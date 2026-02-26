//! Error types for clang parsing.

use clang::EntityKind;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum StructError {
    #[error("Entity is neither a struct or class: {0:?}")]
    NotStructOrClass(EntityKind),
    #[error("Entity does not have a name")]
    NoName,
}

#[derive(Debug, Error)]
pub enum FieldError {
    #[error("Entity is not a field: {0:?}")]
    NotField(EntityKind),
    #[error("Entity does not have a type")]
    NoType,
    #[error("Entity does not have a name")]
    NoName,
    #[error("Entity's type does not have a size")]
    NoSize,
    #[error("Entity's type does not contain a field named {0} to get the offset of")]
    NoOffset(String),
    #[error("Entity's type does not have an alignment")]
    NoAlignment,
}

#[derive(Debug, Error)]
pub enum EnumError {
    #[error("Entity is not an enum: {0:?}")]
    NotEnum(EntityKind),
    #[error("Entity does not have a type")]
    NoType,
}

#[derive(Debug, Error)]
pub enum ConstantError {
    #[error("Entity is not a constant: {0:?}")]
    NotConstant(EntityKind),
    #[error("Entity is not a macro declaration: {0:?}")]
    NotMacroDeclaration(EntityKind),
    #[error("Entity is a macro definition, but it is a function-like macro or built-in macro")]
    UnsupportedMacro,
    #[error("Entity does not have a name")]
    NoName,
    #[error("Constant value could not be evaluated")]
    NotEvaluable,
}

#[derive(Debug, Error)]
pub enum FunctionError {
    #[error("Entity is not a function: {0:?}")]
    NotFunction(EntityKind),
    #[error("Entity does not have a nam")]
    NoName,
    #[error("Entity does not have a type")]
    NoType,
    #[error("Function does not have a return type")]
    NoReturnType,
    #[error("Entity type does not have a calling convention")]
    NoCallingConvention,
    #[error("ParamError: {0}")]
    Param(#[from] ParamError),
}

#[derive(Debug, Error)]
pub enum ParamError {
    #[error("Entity is not a function param: {0:?}")]
    NotParam(EntityKind),
    #[error("Entity does not have a semantic parent")]
    NoSemanticParent,
    #[error("Entity does not have a type")]
    NoType,
}

#[derive(Debug, Error)]
pub enum Error {
    #[error("Struct error: {0}")]
    Struct(#[from] StructError),
    #[error("Field error: {0}")]
    Field(#[from] FieldError),
    #[error("Enum error: {0}")]
    Enum(#[from] EnumError),
    #[error("Constant error: {0}")]
    Constant(#[from] ConstantError),
    #[error("Function error: {0}")]
    Function(#[from] FunctionError),
    #[error("Param error: {0}")]
    Param(#[from] ParamError),
}
