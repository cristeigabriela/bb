//! Module for working with PHNT from a developer command prompt environment.

use crate::winsdk::{SdkMode, sdk_header};
use clap::ValueEnum;

/* ────────────────────────────────── Types ───────────────────────────────── */

/// PHNT version targets, corresponding to Windows releases.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, ValueEnum)]
#[allow(non_camel_case_types)]
pub enum PhntVersion {
    Win2k,
    WinXP,
    WS03,
    Vista,
    Win7,
    Win8,
    WinBlue,
    Threshold,
    Threshold2,
    Redstone,
    Redstone2,
    Redstone3,
    Redstone4,
    Redstone5,
    #[value(name = "19H1")]
    V19H1,
    #[value(name = "19H2")]
    V19H2,
    #[value(name = "20H1")]
    V20H1,
    #[value(name = "20H2")]
    V20H2,
    #[value(name = "21H1")]
    V21H1,
    #[value(name = "Win10-21H2")]
    Win10_21H2,
    #[value(name = "Win10-22H2")]
    Win10_22H2,
    #[default]
    Win11,
    #[value(name = "Win11-22H2")]
    Win11_22H2,
}

impl PhntVersion {
    /// Returns the `PHNT_VERSION` macro name for this version.
    #[must_use]
    pub const fn macro_name(&self) -> &'static str {
        match self {
            Self::Win2k => "PHNT_WIN2K",
            Self::WinXP => "PHNT_WINXP",
            Self::WS03 => "PHNT_WS03",
            Self::Vista => "PHNT_VISTA",
            Self::Win7 => "PHNT_WIN7",
            Self::Win8 => "PHNT_WIN8",
            Self::WinBlue => "PHNT_WINBLUE",
            Self::Threshold => "PHNT_THRESHOLD",
            Self::Threshold2 => "PHNT_THRESHOLD2",
            Self::Redstone => "PHNT_REDSTONE",
            Self::Redstone2 => "PHNT_REDSTONE2",
            Self::Redstone3 => "PHNT_REDSTONE3",
            Self::Redstone4 => "PHNT_REDSTONE4",
            Self::Redstone5 => "PHNT_REDSTONE5",
            Self::V19H1 => "PHNT_19H1",
            Self::V19H2 => "PHNT_19H2",
            Self::V20H1 => "PHNT_20H1",
            Self::V20H2 => "PHNT_20H2",
            Self::V21H1 => "PHNT_21H1",
            Self::Win10_21H2 => "PHNT_WIN10_21H2",
            Self::Win10_22H2 => "PHNT_WIN10_22H2",
            Self::Win11 => "PHNT_WIN11",
            Self::Win11_22H2 => "PHNT_WIN11_22H2",
        }
    }
}

/* ──────────────────────────── Header generation ─────────────────────────── */

/// The embedded PHNT header file, resolved at build time from:
/// 1. `BB_PHNT_HEADER` env var, or
/// 2. `phnt.h` next to the crate, or
/// 3. Generated from the phnt-single-header submodule.
pub const PHNT_HEADER: &str = include_str!(concat!(env!("OUT_DIR"), "/phnt.h"));

/// Utility universal includes required before PHNT.
const PHNT_INCLUDES: &[&str] = &["assert.h"];

/// Generate a synthetic header that includes PHNT with the specified version.
///
/// PHNT mode = the same SDK umbrella as plain `--mode user` / `--mode kernel`
/// (via [`sdk_header`]) with `PHNT_VERSION` plus the phnt.h superset on top.
/// Coverage stays in sync with the winsdk groups.
#[must_use]
pub fn phnt_synthetic_header(version: PhntVersion, kernel_mode: bool) -> String {
    use std::fmt::Write;

    let mode = if kernel_mode {
        SdkMode::Kernel
    } else {
        SdkMode::User
    };

    // Start with the full mode-appropriate SDK umbrella (defines + every
    // group of public headers we curate in winsdk/{user,kernel}.rs), then
    // strip a few headers that fight phnt.h directly:
    //
    // - `winternl.h`: phnt.h errors out with "Do not mix Winternl.h and
    //   phnt.h" — its private NT API set is a superset of winternl's
    //   redacted public surface.
    // - `winusb.h`: pulls `shared/usb.h` whose stub `typedef PVOID PIRP`
    //   conflicts with phnt's real `typedef struct _IRP *PIRP`.
    let mut out = sdk_header(mode)
        .replace("#include <winternl.h>\n", "")
        .replace("#include <winusb.h>\n", "");

    let _ = writeln!(out, "#define PHNT_VERSION {}", version.macro_name());

    for inc in PHNT_INCLUDES {
        let _ = writeln!(out, "#include <{inc}>");
    }

    let _ = writeln!(out, r#"#line 1 "phnt.h""#);
    out.push_str(PHNT_HEADER);

    out
}
