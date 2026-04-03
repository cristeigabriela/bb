//! Display rendering for function declarations.
//!
//! Provides both a compact list view with tree connectors and a detailed
//! ABI breakdown showing where each parameter lives and where the return
//! value goes, matching disassembler notation.

use std::fmt::Write;

use bb_arch::display::register_name;
use bb_arch::location::{MemoryOperand, ParamLocation, ReturnLocation};
use colored::Colorize;

use crate::function::{CallConv, Function, Param};

/* ────────────────────────── Operand formatting ─────────────────────────── */

#[must_use]
pub fn format_operand(op: &MemoryOperand) -> String {
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

#[must_use]
pub fn format_location(loc: &ParamLocation) -> String {
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

#[must_use]
pub fn format_return_location(loc: &ReturnLocation) -> String {
    match loc {
        ReturnLocation::Void => "void".to_string(),
        ReturnLocation::Register(r) => register_name(r).to_string(),
        ReturnLocation::Indirect => "ptr (hidden 1st arg)".to_string(),
    }
}

#[must_use]
pub const fn format_callconv(cc: &CallConv) -> &'static str {
    match cc {
        CallConv::Cdecl => "cdecl",
        CallConv::Stdcall => "stdcall",
        CallConv::Fastcall => "fastcall",
    }
}

#[must_use]
pub const fn format_arch(arch: bb_arch::Arch) -> &'static str {
    match arch {
        bb_arch::Arch::Amd64 => "x64",
        bb_arch::Arch::X86 => "x86",
        bb_arch::Arch::Arm64 => "ARM64",
        bb_arch::Arch::Arm => "ARM32",
    }
}

/// Build the base tags for a function (arch, callconv, exported, `has_body`).
///
/// Returns a `Vec` so callers can extend with additional tags before joining.
#[must_use]
pub fn format_tags(f: &Function) -> Vec<String> {
    let mut tags = Vec::new();
    tags.push(format_arch(f.get_arch()).to_string());
    tags.push(format_callconv(f.get_calling_convention()).to_string());
    if f.is_dllimport() {
        tags.push("exported".to_string());
    }
    if f.has_body() {
        tags.push("has body".to_string());
    }
    tags
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

/// Format a single ABI parameter row: index, kind, location, size, type, name.
///
/// Shared between `render_function_detail` and enriched rendering.
#[must_use]
pub fn format_abi_param(i: usize, p: &Param) -> String {
    let kind = match p.get_abi_location() {
        ParamLocation::Direct { locations, .. } => match locations.first() {
            Some(MemoryOperand::Reg(_)) => "reg",
            Some(MemoryOperand::RegImm { .. }) => "stack",
            None => "?",
        },
        ParamLocation::Indirect { .. } => "indirect",
    };
    let loc_str = format_location(p.get_abi_location()).yellow();
    let type_name = p.get_type_name().cyan();
    let param_name = p.get_name().map_or_else(
        || "<unnamed>".dimmed().to_string(),
        |n| n.white().bold().to_string(),
    );
    let size = match p.get_abi_location() {
        ParamLocation::Direct { size, .. } | ParamLocation::Indirect { size, .. } => *size,
    };
    let size_str = format!("[{size}]").green();

    let idx = i + 1;
    format!("{idx}\t{kind:<8} {loc_str:<14} {size_str}  {type_name}  {param_name}")
}

/// Render a single function as a tree list item with a connector.
#[must_use]
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

    let tags = format_tags(f).join(", ");
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
#[must_use]
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
#[must_use]
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
    let tags = format_tags(f).join(", ");
    let _ = writeln!(out, "{} {}", "│".dimmed(), tags.dimmed());

    // Stack offset note — shown when any param is on the stack.
    let params = f.get_params();
    if params.iter().any(Param::is_stack) {
        let _ = writeln!(
            out,
            "{} {}",
            "│".dimmed(),
            "stack offsets are callee-entry (before prologue)".bright_black(),
        );
    }

    // Blank line before parameters.
    let _ = writeln!(out, "{}", "│".dimmed());

    // Parameters.
    if params.is_empty() {
        let _ = writeln!(out, "{} {}", "│".dimmed(), "(no parameters)".dimmed());
    } else {
        for (i, p) in params.iter().enumerate() {
            let is_last = i == params.len() - 1;
            let connector = if is_last { "╰─" } else { "├─" };
            let _ = writeln!(out, "{} {}", connector.dimmed(), format_abi_param(i, p));
        }
    }

    // Return value — always last child.
    let ret_type = f.get_return_type_name().cyan();
    let ret_loc = format_return_location(f.get_return_location());
    let ret_styled = ret_loc.yellow();
    let _ = writeln!(out, "{} {ret_styled}  {ret_type}", "╰".dimmed());

    out
}
