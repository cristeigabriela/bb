//! Function calling convention representation and ABI parameter assignment.

use bb_arch::{
    Arch, MemoryOperand, ParamLocation, Register, ReturnLocation,
    reg::{
        X64Gpr, X64Xmm, X64_FLOAT_PARAM_REGS, X64_INT_PARAM_REGS, X86Gpr,
        X86_FASTCALL_PARAM_REGS,
    },
};
use clang::{Type, TypeKind};
use serde::Serialize;

use crate::error::FunctionError;

/* ────────────────────────────────── Types ───────────────────────────────── */

/// A limited representation of [`clang::CallingConvention`] with further context,
/// and extensions that expose more information.
///
/// On AMD64, ARM64, ARM32, you might be surprised to see that the sole calling
/// convention used on `WinSDK` and PHNT SDKs is [`CallConv::Cdecl`].
///
/// On x86, you wouldn't be surprised to see that the only calling conventions
/// used on `WinSDK` and PHNT SDKs are [`CallConv::Cdecl`], [`CallConv::Fastcall`]
/// and [`CallConv::Stdcall`].
///
/// Therefore, we will be focusing on those first and foremost.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum CallConv {
    /* ───────────────────────────────── Shared ───────────────────────────────── */
    /// - On x64, this will be the [Microsoft standard x64 calling convention](https://learn.microsoft.com/en-us/cpp/build/x64-calling-convention?view=msvc-170).
    ///     - For details on the ABI, [see here](https://learn.microsoft.com/en-us/cpp/build/x64-software-conventions?view=msvc-170).
    ///     - For details on how the stack is used, [see here](https://learn.microsoft.com/en-us/cpp/build/stack-usage?view=msvc-170).
    ///     - Strict 1:1 positional mapping: param at position N uses slot N.
    ///     - Integer/pointer args: `RCX`, `RDX`, `R8`, `R9` for positions 0–3.
    ///     - Float/double args: `XMM0`–`XMM3` for positions 0–3.
    ///     - Aggregates of size 1, 2, 4, or 8 bytes → treated as integers → GPR.
    ///     - Aggregates of other sizes → passed by pointer (indirect) → pointer in GPR slot.
    ///     - Position >= 4 → stack at `[RSP + 0x28 + (position - 4) * 8]`
    ///       (callee-entry RSP, before prologue).
    ///     - 32-byte shadow space always reserved by caller for positions 0–3.
    ///     - Returns integer in `RAX`, float in `XMM0`.
    ///
    /// ---
    ///
    /// - On x86, the caller pushes the arguments to the stack, in reverse, so that they may be popped in order.
    ///     - The caller is responsible for cleaning up the stack.
    ///     - Returns integer in `EAX`.
    ///
    /// ---
    ///
    /// - On ARM32/ARM64: WIP
    Cdecl,

    /* ───────────────────── x86 — may I never see you again ──────────────────── */
    /// - On x86:
    ///     - First two arguments that fit in a DWORD (left-to-right) are passed in `ECX` and `EDX` respectively.
    ///     - Arguments larger than DWORD skip register assignment.
    ///     - The remainder arguments (right-to-left) are pushed on the stack.
    ///     - Callee is responsible for cleaning up the stack.
    ///     - Returns integer in `EAX`.
    Fastcall,

    /// - On x86: same as cdecl, BUT:
    ///     - Callee is responsible with cleaning up the stack.
    ///     - `ECX`, `EDX` are reserved.
    ///     - Returns integer in `EAX`.
    Stdcall,
}

/* ─────────────────────────────── Conversions ────────────────────────────── */

impl<'a> TryFrom<&Type<'a>> for CallConv {
    type Error = FunctionError;

    fn try_from(type_: &Type<'a>) -> Result<Self, Self::Error> {
        let cc = type_
            .get_calling_convention()
            .ok_or(FunctionError::NoCallingConvention)?;
        match cc {
            clang::CallingConvention::Cdecl => Ok(Self::Cdecl),
            clang::CallingConvention::Stdcall => Ok(Self::Stdcall),
            clang::CallingConvention::Fastcall => Ok(Self::Fastcall),
            _ => Err(FunctionError::NoCallingConvention),
        }
    }
}

/* ──────────────────────── Parameter assignment ─────────────────────────── */

impl CallConv {
    /// Assign ABI locations to each parameter based on architecture and
    /// calling convention rules.
    #[must_use]
    pub fn assign_params(&self, arch: Arch, param_types: &[Type<'_>]) -> Vec<ParamLocation> {
        match (arch, self) {
            (Arch::Amd64, Self::Cdecl) => assign_x64_microsoft(param_types),
            (Arch::X86, Self::Cdecl) => assign_x86_cdecl(param_types),
            (Arch::X86, Self::Fastcall) => assign_x86_fastcall(param_types),
            (Arch::X86, Self::Stdcall) => assign_x86_stdcall(param_types),
            (Arch::Arm64, Self::Cdecl) => todo!("ARM64 AAPCS parameter assignment"),
            (Arch::Arm, Self::Cdecl) => todo!("ARM32 AAPCS parameter assignment"),
            _ => unreachable!(),
        }
    }

    /// Determine where the return value is placed based on architecture and
    /// calling convention.
    #[must_use]
    pub fn return_location(&self, arch: Arch, return_type: &Type<'_>) -> ReturnLocation {
        if return_type.get_kind() == TypeKind::Void {
            return ReturnLocation::Void;
        }

        match arch {
            Arch::Amd64 => return_location_x64(return_type),
            Arch::X86 => ReturnLocation::Register(Register::X86Gpr(X86Gpr::Eax)),
            Arch::Arm64 | Arch::Arm => todo!("ARM return location"),
        }
    }
}

/* ─────────────────────── Type classification helpers ────────────────────── */

/// Returns `true` if the type is a floating-point scalar (float or double).
fn is_float(ty: &Type<'_>) -> bool {
    let kind = ty.get_canonical_type().get_kind();
    matches!(kind, TypeKind::Float | TypeKind::Double)
}

/// Returns `true` if the type is a "simple" type that fits in a register
/// at the given pointer size — i.e., integer, pointer, enum, or bool.
fn is_register_int(ty: &Type<'_>, pointer_size: usize) -> bool {
    let canonical = ty.get_canonical_type();
    let kind = canonical.get_kind();

    // Pointers, references, and bool are always register-sized.
    if matches!(
        kind,
        TypeKind::Pointer
            | TypeKind::BlockPointer
            | TypeKind::LValueReference
            | TypeKind::RValueReference
            | TypeKind::Bool
            | TypeKind::Enum
    ) {
        return true;
    }

    // Integer scalars.
    if matches!(
        kind,
        TypeKind::CharS
            | TypeKind::CharU
            | TypeKind::SChar
            | TypeKind::UChar
            | TypeKind::Short
            | TypeKind::UShort
            | TypeKind::Int
            | TypeKind::UInt
            | TypeKind::Long
            | TypeKind::ULong
            | TypeKind::LongLong
            | TypeKind::ULongLong
    ) {
        return canonical.get_sizeof().is_ok_and(|s| s <= pointer_size);
    }

    false
}

/// For x64: returns the size of a type, classifying aggregates.
/// Aggregates of 1, 2, 4, or 8 bytes are passed as integers.
/// Other sizes are passed by pointer (indirect).
fn x64_param_class(ty: &Type<'_>) -> X64ParamClass {
    if is_float(ty) {
        return X64ParamClass::Float;
    }

    if is_register_int(ty, 8) {
        return X64ParamClass::Integer;
    }

    // Aggregate / struct / union / __m64
    let canonical = ty.get_canonical_type();
    match canonical.get_sizeof() {
        Ok(1 | 2 | 4 | 8) => X64ParamClass::Aggregate,
        Ok(size) => X64ParamClass::IndirectAggregate(size),
        // If we can't determine size, treat as indirect (safe default).
        Err(_) => X64ParamClass::IndirectAggregate(0),
    }
}

enum X64ParamClass {
    /// Passed in a GPR (or on stack as 8-byte slot).
    Integer,
    /// Passed in an XMM register (or on stack as 8-byte slot).
    Float,
    /// Small aggregate (1/2/4/8 bytes) — treated like integer.
    Aggregate,
    /// Large aggregate — passed by pointer (indirect).
    IndirectAggregate(usize),
}

/* ─────────────────── Microsoft x64 calling convention ──────────────────── */

fn assign_x64_microsoft(param_types: &[Type<'_>]) -> Vec<ParamLocation> {
    // Stack offsets are relative to RSP at callee entry (after CALL pushed
    // the return address, before any prologue instructions execute).
    //
    //   [RSP+0x00]  = return address
    //   [RSP+0x08]  = shadow space for RCX  (param 0 home)
    //   [RSP+0x10]  = shadow space for RDX  (param 1 home)
    //   [RSP+0x18]  = shadow space for R8   (param 2 home)
    //   [RSP+0x20]  = shadow space for R9   (param 3 home)
    //   [RSP+0x28]  = 5th param (position 4)
    //   [RSP+0x30]  = 6th param (position 5)
    //   ...
    //
    // NOTE: MSVC x64 does NOT reliably set up an RBP frame pointer, so
    // RSP-at-entry is the only prologue-independent reference.
    // To get the post-prologue RSP offset, add the prologue's total
    // stack adjustment (pushes + sub rsp, N).
    let rsp = Register::X64Gpr(X64Gpr::Rsp);

    param_types
        .iter()
        .enumerate()
        .map(|(i, ty)| {
            let class = x64_param_class(ty);

            if i < 4 {
                // Positions 0–3: register assignment.
                match class {
                    X64ParamClass::Float => ParamLocation::Direct {
                        locations: vec![MemoryOperand::Reg(Register::X64Xmm(
                            X64_FLOAT_PARAM_REGS[i],
                        ))],
                        size: ty.get_sizeof().unwrap_or(8),
                    },
                    X64ParamClass::Integer | X64ParamClass::Aggregate => {
                        ParamLocation::Direct {
                            locations: vec![MemoryOperand::Reg(Register::X64Gpr(
                                X64_INT_PARAM_REGS[i],
                            ))],
                            size: ty.get_sizeof().unwrap_or(8),
                        }
                    }
                    X64ParamClass::IndirectAggregate(size) => ParamLocation::Indirect {
                        pointer: MemoryOperand::Reg(Register::X64Gpr(X64_INT_PARAM_REGS[i])),
                        size,
                    },
                }
            } else {
                // Position >= 4: on the stack.
                // 0x08 (return addr) + 0x20 (shadow) + (position - 4) * 8
                let offset = 0x28_i64 + ((i as i64) - 4) * 8;
                match class {
                    X64ParamClass::IndirectAggregate(size) => ParamLocation::Indirect {
                        pointer: MemoryOperand::RegImm { base: rsp, offset },
                        size,
                    },
                    _ => ParamLocation::Direct {
                        locations: vec![MemoryOperand::RegImm { base: rsp, offset }],
                        size: ty.get_sizeof().unwrap_or(8),
                    },
                }
            }
        })
        .collect()
}

/* ─────────────────────────── x86 cdecl ─────────────────────────────────── */

fn assign_x86_cdecl(param_types: &[Type<'_>]) -> Vec<ParamLocation> {
    // Stack offsets are relative to ESP at callee entry (after CALL pushed
    // the return address, before any prologue instructions execute).
    //
    //   [ESP+0x00]  = return address
    //   [ESP+0x04]  = 1st param
    //   [ESP+0x04+sizeof(1st)] = 2nd param
    //   ...
    let esp = Register::X86Gpr(X86Gpr::Esp);

    let mut offset: i64 = 0x04;
    param_types
        .iter()
        .map(|ty| {
            let size = ty.get_sizeof().unwrap_or(4);
            let loc = ParamLocation::Direct {
                locations: vec![MemoryOperand::RegImm { base: esp, offset }],
                size,
            };
            // Align each slot to 4-byte boundary (x86 stack alignment).
            offset += ((size + 3) & !3) as i64;
            loc
        })
        .collect()
}

/* ─────────────────────────── x86 stdcall ───────────────────────────────── */

fn assign_x86_stdcall(param_types: &[Type<'_>]) -> Vec<ParamLocation> {
    // Same layout as cdecl — only difference is cleanup responsibility.
    assign_x86_cdecl(param_types)
}

/* ─────────────────────────── x86 fastcall ──────────────────────────────── */

fn assign_x86_fastcall(param_types: &[Type<'_>]) -> Vec<ParamLocation> {
    // Stack offsets are relative to ESP at callee entry (see assign_x86_cdecl).
    let esp = Register::X86Gpr(X86Gpr::Esp);

    let mut reg_index: usize = 0; // next available fastcall register
    let mut stack_offset: i64 = 0x04; // [ESP+0x04] = first stack param

    param_types
        .iter()
        .map(|ty| {
            let size = ty.get_sizeof().unwrap_or(4);

            // Fastcall: first two DWORD-or-smaller args go in ECX, EDX.
            if reg_index < 2 && size <= 4 {
                let reg = X86_FASTCALL_PARAM_REGS[reg_index];
                reg_index += 1;
                ParamLocation::Direct {
                    locations: vec![MemoryOperand::Reg(Register::X86Gpr(reg))],
                    size,
                }
            } else {
                let loc = ParamLocation::Direct {
                    locations: vec![MemoryOperand::RegImm {
                        base: esp,
                        offset: stack_offset,
                    }],
                    size,
                };
                stack_offset += ((size + 3) & !3) as i64;
                loc
            }
        })
        .collect()
}

/* ─────────────────────── x64 return location ───────────────────────────── */

fn return_location_x64(return_type: &Type<'_>) -> ReturnLocation {
    if is_float(return_type) {
        return ReturnLocation::Register(Register::X64Xmm(X64Xmm::Xmm0));
    }

    let canonical = return_type.get_canonical_type();

    // Scalars and small POD aggregates (1/2/4/8 bytes) → RAX.
    match canonical.get_sizeof() {
        Ok(1..=8) => ReturnLocation::Register(Register::X64Gpr(X64Gpr::Rax)),
        // Larger types: caller passes hidden pointer, callee writes there.
        _ => ReturnLocation::Indirect,
    }
}
