//! Architecture definitions, register sets, and ABI location types.
//!
//! This crate provides the shared vocabulary for describing target architectures,
//! hardware registers, and where values live at the ABI level.

pub mod display;
pub mod json;
pub mod location;
pub mod reg;

use serde::Serialize;
use thiserror::Error;

pub use json::ToJson;
pub use location::{MemoryOperand, ParamLocation, ReturnLocation};
pub use reg::Register;

/* ────────────────────────────────── Types ───────────────────────────────── */

/// Target architecture.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, clap::ValueEnum)]
pub enum Arch {
    X86,
    Amd64,
    Arm,
    Arm64,
}

/* ──────────────────────────────── Errors ────────────────────────────────── */

#[derive(Debug, Error)]
#[error("unrecognized target triple: {0}")]
pub struct UnknownTripleError(pub String);

/* ───────────────────────────── Implementation ──────────────────────────── */

impl Arch {
    /// Pointer size in bytes for this architecture.
    #[must_use]
    pub const fn pointer_size(self) -> usize {
        match self {
            Self::Amd64 | Self::Arm64 => 8,
            Self::X86 | Self::Arm => 4,
        }
    }

    /// Derive the architecture from a clang target triple.
    pub fn from_triple(triple: &str) -> Result<Self, UnknownTripleError> {
        if triple.starts_with("x86_64") {
            Ok(Self::Amd64)
        } else if triple.starts_with("i686") || triple.starts_with("i386") {
            Ok(Self::X86)
        } else if triple.starts_with("aarch64") {
            Ok(Self::Arm64)
        } else if triple.starts_with("thumb") || triple.starts_with("arm") {
            Ok(Self::Arm)
        } else {
            Err(UnknownTripleError(triple.to_owned()))
        }
    }

    /// The MSVC target triple for this architecture.
    #[must_use]
    pub const fn target_triple(self) -> &'static str {
        match self {
            Self::X86 => "i686-pc-windows-msvc",
            Self::Amd64 => "x86_64-pc-windows-msvc",
            Self::Arm => "thumbv7-pc-windows-msvc",
            Self::Arm64 => "aarch64-pc-windows-msvc",
        }
    }
}
