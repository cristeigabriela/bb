//! Hardware register definitions for each supported architecture.

mod arm32;
mod arm64;
mod x64;
mod x86;

pub use arm32::*;
pub use arm64::*;
pub use x64::*;
pub use x86::*;

use serde::Serialize;

/* ────────────────────────────────── Types ───────────────────────────────── */

/// A hardware register, across all supported architectures.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
pub enum Register {
    X64Gpr(X64Gpr),
    X64Xmm(X64Xmm),
    X86Gpr(X86Gpr),
    Arm64Gpr(Arm64Gpr),
    Arm32Gpr(Arm32Gpr),
}

/* ─────────────────────────────── Conversions ────────────────────────────── */

impl From<X64Gpr> for Register {
    fn from(r: X64Gpr) -> Self {
        Self::X64Gpr(r)
    }
}

impl From<X64Xmm> for Register {
    fn from(r: X64Xmm) -> Self {
        Self::X64Xmm(r)
    }
}

impl From<X86Gpr> for Register {
    fn from(r: X86Gpr) -> Self {
        Self::X86Gpr(r)
    }
}

impl From<Arm64Gpr> for Register {
    fn from(r: Arm64Gpr) -> Self {
        Self::Arm64Gpr(r)
    }
}

impl From<Arm32Gpr> for Register {
    fn from(r: Arm32Gpr) -> Self {
        Self::Arm32Gpr(r)
    }
}
