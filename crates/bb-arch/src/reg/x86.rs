//! x86 (32-bit) register definitions.

use serde::Serialize;

/* ────────────────────────────────── Types ───────────────────────────────── */

/// x86 general-purpose registers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
pub enum X86Gpr {
    Eax,
    Ecx,
    Edx,
    Ebx,
    Esp,
    Ebp,
    Esi,
    Edi,
}

/* ────────────────────────────── Param registers ────────────────────────── */

/// Fastcall parameter registers in positional order.
pub const X86_FASTCALL_PARAM_REGS: [X86Gpr; 2] = [X86Gpr::Ecx, X86Gpr::Edx];
