//! ARM64 (`AArch64`) register definitions.

use serde::Serialize;

/* ────────────────────────────────── Types ───────────────────────────────── */

/// ARM64 general-purpose registers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
pub enum Arm64Gpr {
    X0,
    X1,
    X2,
    X3,
    X4,
    X5,
    X6,
    X7,
    X8,
    X9,
    X10,
    X11,
    X12,
    X13,
    X14,
    X15,
    X16,
    X17,
    X18,
    X19,
    X20,
    X21,
    X22,
    X23,
    X24,
    X25,
    X26,
    X27,
    X28,
    /// Frame pointer.
    X29,
    /// Link register.
    X30,
    /// Stack pointer (not a GPR in the traditional sense, but addressable).
    Sp,
}

/* ────────────────────────────── Param registers ────────────────────────── */

/// Integer/pointer parameter registers in positional order (Windows ARM64 ABI).
/// Reserved for future ARM64 AAPCS implementation.
#[allow(dead_code)]
pub const ARM64_INT_PARAM_REGS: [Arm64Gpr; 8] = [
    Arm64Gpr::X0,
    Arm64Gpr::X1,
    Arm64Gpr::X2,
    Arm64Gpr::X3,
    Arm64Gpr::X4,
    Arm64Gpr::X5,
    Arm64Gpr::X6,
    Arm64Gpr::X7,
];
