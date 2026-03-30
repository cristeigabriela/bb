//! x86-64 register definitions.

use serde::Serialize;

/* ────────────────────────────────── Types ───────────────────────────────── */

/// x86-64 general-purpose registers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
pub enum X64Gpr {
    Rax,
    Rcx,
    Rdx,
    Rbx,
    Rsp,
    Rbp,
    Rsi,
    Rdi,
    R8,
    R9,
    R10,
    R11,
    R12,
    R13,
    R14,
    R15,
}

/// x86-64 SSE registers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
pub enum X64Xmm {
    Xmm0,
    Xmm1,
    Xmm2,
    Xmm3,
    Xmm4,
    Xmm5,
    Xmm6,
    Xmm7,
    Xmm8,
    Xmm9,
    Xmm10,
    Xmm11,
    Xmm12,
    Xmm13,
    Xmm14,
    Xmm15,
}

/* ────────────────────────────── Param registers ────────────────────────── */

/// Integer/pointer parameter registers in positional order.
pub const X64_INT_PARAM_REGS: [X64Gpr; 4] = [X64Gpr::Rcx, X64Gpr::Rdx, X64Gpr::R8, X64Gpr::R9];

/// Floating-point parameter registers in positional order.
pub const X64_FLOAT_PARAM_REGS: [X64Xmm; 4] =
    [X64Xmm::Xmm0, X64Xmm::Xmm1, X64Xmm::Xmm2, X64Xmm::Xmm3];
