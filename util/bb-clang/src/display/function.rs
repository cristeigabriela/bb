//! Display rendering for function declarations.
//!
//! Provides both a compact list view with tree connectors and a detailed
//! ABI breakdown showing where each parameter lives and where the return
//! value goes, matching disassembler notation.

use std::fmt::Write;

use bb_arch::location::{MemoryOperand, ParamLocation, ReturnLocation};
use bb_arch::reg::*;
use bb_arch::Register;
use colored::Colorize;

use crate::function::{CallConv, Function, Param};

/* ────────────────────────────── Register names ─────────────────────────── */

fn register_name(reg: &Register) -> &'static str {
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

/* ────────────────────────── Operand formatting ─────────────────────────── */

fn format_operand(op: &MemoryOperand) -> String {
    match op {
        MemoryOperand::Reg(r) => register_name(r).to_string(),
        MemoryOperand::RegImm { base, offset } => {
            if *offset >= 0 {
                format!("[{}+{:#X}]", register_name(base), offset)
            } else {
                format!("[{}-{:#X}]", register_name(base), offset.unsigned_abs())
            }
        }
    }
}

fn format_location(loc: &ParamLocation) -> String {
    match loc {
        ParamLocation::Direct { locations, .. } => locations
            .iter()
            .map(format_operand)
            .collect::<Vec<_>>()
            .join(":"),
        ParamLocation::Indirect { pointer, .. } => {
            format!("ptr → {}", format_operand(pointer))
        }
    }
}

fn format_return_location(loc: &ReturnLocation) -> String {
    match loc {
        ReturnLocation::Void => "void".to_string(),
        ReturnLocation::Register(r) => register_name(r).to_string(),
        ReturnLocation::Indirect => "ptr (hidden 1st arg)".to_string(),
    }
}

fn format_callconv(cc: &CallConv) -> &'static str {
    match cc {
        CallConv::Cdecl => "cdecl",
        CallConv::Stdcall => "stdcall",
        CallConv::Fastcall => "fastcall",
    }
}

fn format_arch(arch: bb_arch::Arch) -> &'static str {
    match arch {
        bb_arch::Arch::Amd64 => "x64",
        bb_arch::Arch::X86 => "x86",
        bb_arch::Arch::Arm64 => "ARM64",
        bb_arch::Arch::Arm => "ARM32",
    }
}

/// Build the tags line (arch, callconv, exported, has_body).
fn format_tags(f: &Function) -> String {
    let mut tags = Vec::new();
    tags.push(format_arch(f.get_arch()).to_string());
    tags.push(format_callconv(f.get_calling_convention()).to_string());
    if f.is_dllimport() {
        tags.push("exported".to_string());
    }
    if f.has_body() {
        tags.push("has body".to_string());
    }
    tags.join(", ")
}

/// Format a typed+named parameter string for display.
fn format_param_sig(p: &Param) -> String {
    let ty = p.get_type_name().cyan().to_string();
    match p.get_name() {
        Some(n) => format!("{ty} {}", n.white().bold()),
        None => ty,
    }
}

/* ──────────────────── Compact list item (tree connector) ───────────────── */

/// Render a single function as a tree list item with a connector.
pub fn render_function_item(f: &Function, connector: &str) -> String {
    let mut out = String::new();

    let name = f.get_name().cyan().bold();
    let ret = f.get_return_type_name().green();
    let params_str: String = f
        .get_params()
        .iter()
        .map(|p| format_param_sig(p))
        .collect::<Vec<_>>()
        .join(", ");

    let tags = format_tags(f);
    let loc = f
        .get_location()
        .map(|l| format!(" {l}").dimmed().to_string())
        .unwrap_or_default();

    let _ = writeln!(
        out,
        "{} {ret} {name}({params_str})  {}{}",
        connector.dimmed(),
        tags.dimmed(),
        loc,
    );

    out
}

/// Render a list of functions as a tree with connectors and a footer.
pub fn render_function_list(funcs: &[Function]) -> String {
    let mut out = String::new();

    for (i, f) in funcs.iter().enumerate() {
        let is_last = i == funcs.len() - 1;
        let connector = if is_last { "╰─" } else { "├─" };
        out.push_str(&render_function_item(f, connector));
        if !is_last {
            let _ = writeln!(out, "{}", "│".dimmed());
        }
    }

    let _ = writeln!(out, "{}", format!("   {} functions", funcs.len()).dimmed());
    out
}

/* ──────────────────── Detailed ABI breakdown rendering ─────────────────── */

/// Render a detailed ABI breakdown for a function.
///
/// Shows the C signature as the tree root, with architecture/tags,
/// parameter ABI locations, and return value placement as children.
pub fn render_function_detail(f: &Function) -> String {
    let mut out = String::new();

    // Header: C signature as the tree root.
    let name = f.get_name().cyan().bold();
    let ret = f.get_return_type_name().green();
    let params_str: String = f
        .get_params()
        .iter()
        .map(|p| format_param_sig(p))
        .collect::<Vec<_>>()
        .join(", ");
    let loc = f
        .get_location()
        .map(|l| format!("  {l}").dimmed().to_string())
        .unwrap_or_default();
    let _ = writeln!(out, "{ret} {name}({params_str}){loc}");

    // Tags line as first child.
    let tags = format_tags(f);
    let _ = writeln!(out, "{} {}", "│".dimmed(), tags.dimmed());

    // Parameters.
    let params = f.get_params();
    if params.is_empty() {
        let _ = writeln!(out, "{} {}", "│".dimmed(), "(no parameters)".dimmed());
    } else {
        for (i, p) in params.iter().enumerate() {
            let is_last = i == params.len() - 1;
            let connector = if is_last { "╰─" } else { "├─" };

            let idx = format!("{i}");
            let kind = match p.get_abi_location() {
                ParamLocation::Direct { locations, .. } => match locations.first() {
                    Some(MemoryOperand::Reg(_)) => "reg",
                    Some(MemoryOperand::RegImm { .. }) => "stack",
                    None => "?",
                },
                ParamLocation::Indirect { .. } => "indirect",
            };
            let loc_str = format_location(p.get_abi_location());
            let loc_styled = loc_str.yellow();
            let type_name = p.get_type_name().cyan();
            let param_name = p
                .get_name()
                .map(|n| n.white().bold().to_string())
                .unwrap_or_else(|| "<unnamed>".dimmed().to_string());
            let size = match p.get_abi_location() {
                ParamLocation::Direct { size, .. } | ParamLocation::Indirect { size, .. } => *size,
            };
            let size_str = format!("[{size}]").green();

            let _ = writeln!(
                out,
                "{} {idx}\t{kind:<8} {loc_styled:<14} {size_str}  {type_name}  {param_name}",
                connector.dimmed(),
            );
        }
    }

    // Return value — always last child.
    let ret_type = f.get_return_type_name().cyan();
    let ret_loc = format_return_location(f.get_return_location());
    let ret_styled = ret_loc.yellow();
    let _ = writeln!(out, "{} {ret_styled}  {ret_type}", "╰".dimmed());

    out
}
