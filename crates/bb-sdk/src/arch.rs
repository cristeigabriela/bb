//! SDK-level architecture extensions.
//!
//! Re-exports [`bb_arch::Arch`] and adds Windows SDK-specific methods
//! (preprocessor defines for cross-compilation).

pub use bb_arch::Arch;

/* ──────────────────────────── SDK extensions ────────────────────────────── */

/// SDK-specific preprocessor defines for each architecture.
pub trait ArchDefines {
    fn defines(self) -> &'static [&'static str];
}

impl ArchDefines for Arch {
    fn defines(self) -> &'static [&'static str] {
        match self {
            Self::X86 => &["-D_WIN32", "-D_X86_", "-D_M_IX86=600"],
            Self::Amd64 => &[
                "-D_WIN32",
                "-D_WIN64",
                "-D_AMD64_",
                "-D_M_AMD64=100",
                "-D_M_X64=100",
            ],
            Self::Arm => &["-D_WIN32", "-D_ARM_", "-D_M_ARM=7"],
            Self::Arm64 => &["-D_WIN32", "-D_WIN64", "-D_ARM64_", "-D_M_ARM64=1"],
        }
    }
}
