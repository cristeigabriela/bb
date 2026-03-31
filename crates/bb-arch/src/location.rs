//! Memory operand and parameter location types.

use serde::Serialize;

use crate::reg::Register;

/* ────────────────────────────────── Types ───────────────────────────────── */

/// A way to refer to a value's location, matching disassembler notation.
///
/// - `Reg(RCX)` → a value sitting in a register.
/// - `RegImm { base: RSP, offset: 0x28 }` → a value at `[RSP + 0x28]`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum MemoryOperand {
    /// Value is in a register directly.
    Reg(Register),
    /// Value is in memory at `[base + offset]`.
    RegImm { base: Register, offset: i64 },
}

/// Where a parameter lives at the ABI level.
///
/// A single parameter may occupy one or more locations (e.g., a 64-bit value
/// split across two 32-bit registers on ARM32), or may be passed indirectly
/// (caller allocates, passes pointer).
///
/// Stack offsets are relative to RSP/ESP **at callee entry** — after CALL
/// pushed the return address, before any prologue instructions execute.
/// This is the ABI contract and does not depend on prologue style.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub enum ParamLocation {
    /// Value stored directly at one or more locations.
    ///
    /// Usually a single register or single stack slot.
    /// Multiple entries for register pairs (e.g., ARM32 `R0:R1` for a 64-bit value).
    ///
    /// `size` is the total size of the parameter in bytes.
    Direct {
        locations: Vec<MemoryOperand>,
        size: usize,
    },

    /// Value is passed indirectly: caller allocates memory, passes a pointer.
    ///
    /// The pointer itself is at the given operand. `size` is the size of the
    /// pointed-to value, not the pointer.
    Indirect { pointer: MemoryOperand, size: usize },
}

/// Where a function's return value is placed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub enum ReturnLocation {
    /// No return value (`void`).
    Void,
    /// Return value is placed directly in a register.
    Register(Register),
    /// Return value is written to caller-allocated memory.
    /// The caller passes a hidden pointer as the first argument.
    Indirect,
}
