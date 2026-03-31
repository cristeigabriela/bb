//! Display and serialization helpers for architecture types.

use serde_json::json;

use crate::Register;
use crate::location::{MemoryOperand, ParamLocation, ReturnLocation};
use crate::reg::{Arm32Gpr, Arm64Gpr, X64Gpr, X64Xmm, X86Gpr};

/* ────────────────────────────── Register names ─────────────────────────── */

/// Get the canonical display name for a register.
#[must_use]
pub fn register_name(reg: &Register) -> &'static str {
    match reg {
        Register::X64Gpr(r) => match r {
            X64Gpr::Rax => "RAX",
            X64Gpr::Rcx => "RCX",
            X64Gpr::Rdx => "RDX",
            X64Gpr::Rbx => "RBX",
            X64Gpr::Rsp => "RSP",
            X64Gpr::Rbp => "RBP",
            X64Gpr::Rsi => "RSI",
            X64Gpr::Rdi => "RDI",
            X64Gpr::R8 => "R8",
            X64Gpr::R9 => "R9",
            X64Gpr::R10 => "R10",
            X64Gpr::R11 => "R11",
            X64Gpr::R12 => "R12",
            X64Gpr::R13 => "R13",
            X64Gpr::R14 => "R14",
            X64Gpr::R15 => "R15",
        },
        Register::X64Xmm(r) => match r {
            X64Xmm::Xmm0 => "XMM0",
            X64Xmm::Xmm1 => "XMM1",
            X64Xmm::Xmm2 => "XMM2",
            X64Xmm::Xmm3 => "XMM3",
            X64Xmm::Xmm4 => "XMM4",
            X64Xmm::Xmm5 => "XMM5",
            X64Xmm::Xmm6 => "XMM6",
            X64Xmm::Xmm7 => "XMM7",
            X64Xmm::Xmm8 => "XMM8",
            X64Xmm::Xmm9 => "XMM9",
            X64Xmm::Xmm10 => "XMM10",
            X64Xmm::Xmm11 => "XMM11",
            X64Xmm::Xmm12 => "XMM12",
            X64Xmm::Xmm13 => "XMM13",
            X64Xmm::Xmm14 => "XMM14",
            X64Xmm::Xmm15 => "XMM15",
        },
        Register::X86Gpr(r) => match r {
            X86Gpr::Eax => "EAX",
            X86Gpr::Ecx => "ECX",
            X86Gpr::Edx => "EDX",
            X86Gpr::Ebx => "EBX",
            X86Gpr::Esp => "ESP",
            X86Gpr::Ebp => "EBP",
            X86Gpr::Esi => "ESI",
            X86Gpr::Edi => "EDI",
        },
        Register::Arm64Gpr(r) => match r {
            Arm64Gpr::X0 => "X0",
            Arm64Gpr::X1 => "X1",
            Arm64Gpr::X2 => "X2",
            Arm64Gpr::X3 => "X3",
            Arm64Gpr::X4 => "X4",
            Arm64Gpr::X5 => "X5",
            Arm64Gpr::X6 => "X6",
            Arm64Gpr::X7 => "X7",
            Arm64Gpr::X8 => "X8",
            Arm64Gpr::X9 => "X9",
            Arm64Gpr::X10 => "X10",
            Arm64Gpr::X11 => "X11",
            Arm64Gpr::X12 => "X12",
            Arm64Gpr::X13 => "X13",
            Arm64Gpr::X14 => "X14",
            Arm64Gpr::X15 => "X15",
            Arm64Gpr::X16 => "X16",
            Arm64Gpr::X17 => "X17",
            Arm64Gpr::X18 => "X18",
            Arm64Gpr::X19 => "X19",
            Arm64Gpr::X20 => "X20",
            Arm64Gpr::X21 => "X21",
            Arm64Gpr::X22 => "X22",
            Arm64Gpr::X23 => "X23",
            Arm64Gpr::X24 => "X24",
            Arm64Gpr::X25 => "X25",
            Arm64Gpr::X26 => "X26",
            Arm64Gpr::X27 => "X27",
            Arm64Gpr::X28 => "X28",
            Arm64Gpr::X29 => "FP",
            Arm64Gpr::X30 => "LR",
            Arm64Gpr::Sp => "SP",
        },
        Register::Arm32Gpr(r) => match r {
            Arm32Gpr::R0 => "R0",
            Arm32Gpr::R1 => "R1",
            Arm32Gpr::R2 => "R2",
            Arm32Gpr::R3 => "R3",
            Arm32Gpr::R4 => "R4",
            Arm32Gpr::R5 => "R5",
            Arm32Gpr::R6 => "R6",
            Arm32Gpr::R7 => "R7",
            Arm32Gpr::R8 => "R8",
            Arm32Gpr::R9 => "R9",
            Arm32Gpr::R10 => "R10",
            Arm32Gpr::R11 => "FP",
            Arm32Gpr::R12 => "IP",
            Arm32Gpr::Sp => "SP",
            Arm32Gpr::Lr => "LR",
            Arm32Gpr::Pc => "PC",
        },
    }
}

/* ──────────────────────── ABI JSON serialization ────────────────────────── */

/// Serialize a [`MemoryOperand`] to JSON.
#[must_use]
pub fn operand_to_json(op: &MemoryOperand) -> serde_json::Value {
    match op {
        MemoryOperand::Reg(r) => json!({
            "kind": "reg",
            "register": register_name(r),
        }),
        MemoryOperand::RegImm { base, offset } => json!({
            "kind": "stack",
            "base": register_name(base),
            "offset": offset,
        }),
    }
}

/// Serialize a [`ParamLocation`] to JSON.
#[must_use]
pub fn param_abi_to_json(loc: &ParamLocation) -> serde_json::Value {
    match loc {
        ParamLocation::Direct { locations, size } => {
            let mut obj = match locations.first() {
                Some(op) => operand_to_json(op),
                None => json!({ "kind": "?" }),
            };
            if let Some(map) = obj.as_object_mut() {
                map.insert("size".into(), json!(size));
            }
            obj
        }
        ParamLocation::Indirect { pointer, size } => json!({
            "kind": "indirect",
            "pointer": operand_to_json(pointer),
            "size": size,
        }),
    }
}

/// Serialize a [`ReturnLocation`] to JSON.
#[must_use]
pub fn return_abi_to_json(loc: &ReturnLocation) -> serde_json::Value {
    match loc {
        ReturnLocation::Void => json!({ "kind": "void" }),
        ReturnLocation::Register(r) => json!({
            "kind": "reg",
            "register": register_name(r),
        }),
        ReturnLocation::Indirect => json!({ "kind": "indirect" }),
    }
}
