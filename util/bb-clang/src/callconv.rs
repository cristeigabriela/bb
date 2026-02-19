//! Function calling convention representation.

use serde::Serialize;

/* ────────────────────────────────── Types ───────────────────────────────── */

/// A limited representation of [`clang::CallingConvention`] with further context,
/// and extensions that expose more information.
///
/// On AMD64, ARM64, ARM32, you might be surprised to see that the sole calling
/// convention used on WinSDK and PHNT SDKs is [`CallConv::Cdecl`].
///
/// On x86, you wouldn't be surprised to see that the only calling conventions
/// used on WinSDK and PHNT SDKs are [`CallConv::Cdecl`], [`CallConv::Fastcall`]
/// and [`CallConv::Stdcall`].
///
/// Therefore, we will be focusing on those first and foremost.
#[derive(Debug, Serialize)]
pub enum CallConv {
    /* ───────────────────────────────── Shared ───────────────────────────────── */
    Cdecl,

    /* ───────────────────── x86 — may I never see you again ──────────────────── */
    Fastcall,
    Stdcall,
}

/* ─────────────────────────────── Conversions ────────────────────────────── */

impl From<clang::CallingConvention> for CallConv {
    fn from(value: clang::CallingConvention) -> Self {
        match value {
            clang::CallingConvention::Cdecl => Self::Cdecl,
            clang::CallingConvention::Stdcall => Self::Stdcall,
            clang::CallingConvention::Fastcall => Self::Fastcall,
            // NOTE: lol
            _ => unreachable!(),
        }
    }
}
