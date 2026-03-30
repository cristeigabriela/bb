//! ARM32 (Thumb/ARM) register definitions.

use serde::Serialize;

/* ────────────────────────────────── Types ───────────────────────────────── */

/// ARM32 general-purpose registers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
pub enum Arm32Gpr {
    R0,
    R1,
    R2,
    R3,
    R4,
    R5,
    R6,
    R7,
    R8,
    R9,
    R10,
    /// Frame pointer.
    R11,
    /// Intra-procedure scratch register.
    R12,
    /// Stack pointer.
    Sp,
    /// Link register.
    Lr,
    /// Program counter.
    Pc,
}

/* ────────────────────────────── Param registers ────────────────────────── */

/// Integer/pointer parameter registers in positional order (ARM32 AAPCS).
pub const ARM32_INT_PARAM_REGS: [Arm32Gpr; 4] = [
    Arm32Gpr::R0,
    Arm32Gpr::R1,
    Arm32Gpr::R2,
    Arm32Gpr::R3,
];
