//! Module for working with Windows SDK from a developer command prompt environment.

mod kernel;
mod user;

use anyhow::{Result, anyhow};
use colored::Colorize;
use std::env::var;
use std::path::PathBuf;

/* ────────────────────────────────── Types ───────────────────────────────── */

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, clap::ValueEnum)]
pub enum SdkMode {
    #[default]
    User,
    Kernel,
}

#[derive(Debug, Clone)]
pub struct SdkInfo {
    include_dir: PathBuf,
    version: String,
}

impl SdkInfo {
    #[must_use]
    pub const fn get_include_dir(&self) -> &PathBuf {
        &self.include_dir
    }
    #[must_use]
    pub fn get_version(&self) -> &str {
        &self.version
    }

    /// Locate the newest installed WDF flavor under this SDK's include dir.
    ///
    /// `flavor` is `"kmdf"` for kernel-mode WDF or `"umdf"` for user-mode.
    /// The SDK lays them out under `Include/wdf/<flavor>/<major>.<minor>/`
    /// (these are NOT under the per-SDK-version `Include/<sdk-ver>/` tree).
    /// Returns `None` if no WDF of that flavor is installed.
    #[must_use]
    pub fn wdf_latest(&self, flavor: &str) -> Option<WdfLocation> {
        // `Include/wdf/<flavor>` is a sibling of `Include/<sdk-ver>`, not a
        // child — climb out of the version dir before joining `wdf`.
        let root = self.include_dir.parent()?.join("wdf").join(flavor);
        latest_versioned_dir(&root, "wdf.h")
    }

    /// Locate the newest installed `NetAdapterCx` tree under this SDK's
    /// include dir. The layout differs from WDF — `NetCx` is laid out
    /// *inside* the per-SDK-version `km/netcx/kmdf/adapter/<M>.<N>/`
    /// (siblings, public headers) plus `shared/netcx/shared/1.0/net/`
    /// (the `net/*.h` types netadaptercx.h depends on).
    ///
    /// Returns the path to add to the `-I` search list and the version
    /// numbers; the shared/net path is reported via [`netcx_shared_dir`].
    #[must_use]
    pub fn netcx_latest(&self) -> Option<WdfLocation> {
        let root = self
            .include_dir
            .join("km")
            .join("netcx")
            .join("kmdf")
            .join("adapter");
        latest_versioned_dir(&root, "netadaptercx.h")
    }

    /// Companion to [`netcx_latest`]: the `shared/netcx/shared/<ver>/`
    /// directory that supplies `net/extension.h`, `net/ring.h`, etc.
    /// Currently only `1.0` exists, but we still discover it.
    #[must_use]
    pub fn netcx_shared_dir(&self) -> Option<PathBuf> {
        let root = self.include_dir.join("shared").join("netcx").join("shared");
        latest_versioned_dir(&root, "net/extension.h").map(|loc| loc.include_dir)
    }
}

/// Walk a directory of `<major>.<minor>/` subdirs and return the one
/// containing `marker` with the highest version.
fn latest_versioned_dir(root: &PathBuf, marker: &str) -> Option<WdfLocation> {
    let mut best: Option<((u32, u32), PathBuf)> = None;
    for entry in std::fs::read_dir(root).ok()?.flatten() {
        let name = entry.file_name();
        let Some(name_str) = name.to_str() else {
            continue;
        };
        let Some((maj, min)) = name_str.split_once('.') else {
            continue;
        };
        let (Ok(maj), Ok(min)) = (maj.parse::<u32>(), min.parse::<u32>()) else {
            continue;
        };
        let path = entry.path();
        if path.join(marker).exists() {
            let v = (maj, min);
            if best.as_ref().is_none_or(|(b, _)| v > *b) {
                best = Some((v, path));
            }
        }
    }
    best.map(|((major, minor), include_dir)| WdfLocation {
        include_dir,
        major,
        minor,
    })
}

/// One installed WDF version (KMDF or UMDF) discovered under the SDK root.
#[derive(Debug, Clone)]
pub struct WdfLocation {
    /// `…/Include/wdf/<flavor>/<major>.<minor>/`
    pub include_dir: PathBuf,
    pub major: u32,
    pub minor: u32,
}

/* ──────────────────────────────── Utilities ─────────────────────────────── */

/// Generate a [`SdkInfo`] structure from the current environment.
///
/// # Arguments
///
/// * `override_version`: Optionally provide a different Windows SDK version to analyze.
///
/// # Errors
///
/// - Will return an `Err` if environment is not set up or not installed;
/// - Will return an `Err` if necessary files are not found on the file-system.
pub fn get_sdk_info(override_version: Option<&str>) -> Result<SdkInfo> {
    let sdk_dir = var("WindowsSdkDir").map_err(|_| missing_env_error())?;
    let sdk_dir = PathBuf::from(sdk_dir);

    let version = if let Some(v) = override_version {
        v.to_string()
    } else {
        var("WindowsSDKLibVersion")
            .map_err(|_| missing_env_error())?
            .trim_end_matches('\\')
            .to_string()
    };

    let include_dir = sdk_dir.join("Include").join(&version);
    if !include_dir.exists() {
        return Err(anyhow!(
            "SDK version {} not found at {}\n\n{}\n  {}",
            version.yellow(),
            include_dir.display().to_string().dimmed(),
            "Available versions can be found in:".white(),
            sdk_dir.join("Include").display().to_string().cyan()
        ));
    }

    Ok(SdkInfo {
        include_dir,
        version,
    })
}

/* ───────────────────────────────── Errors ───────────────────────────────── */

/// Responsible for verifying if Windows SDK is installed (or present in environment).
///
/// Will warn the user through error propagation if it is not, as we have no other way to
/// get Windows SDK.
///
/// Advises user to restart through "Developer Command Prompt for VS" or run "vcvarsall.bat."
fn missing_env_error() -> anyhow::Error {
    let msg = format!(
        "{}\n\n\
         {}\n\
         {}\n\n\
         {}\n\
         {}\n\n\
         {}\n\
         {}",
        "Windows SDK not installed (or not present in environment)"
            .red()
            .bold(),
        "The following environment variables are required:".white(),
        "  • WindowsSdkDir\n  • WindowsSDKLibVersion".yellow(),
        "To fix this, run one of:".white(),
        format!(
            "  {} {}\n  {} {}",
            "›".dimmed(),
            r"vcvarsall.bat x64".cyan(),
            "›".dimmed(),
            r"Developer Command Prompt for VS".cyan()
        ),
        "Or launch your terminal from Visual Studio.".dimmed(),
        format!(
            "See: {}",
            "https://learn.microsoft.com/en-us/cpp/build/building-on-the-command-line"
        )
        .dimmed()
    );
    anyhow!(msg)
}

/// Responsible for verifying if WDK is installed (or present in environment).
///
/// # Errors
///
/// Will warn the user through `Err` if it is not, and they are trying to invoke
/// with [`SdkMode::Kernel`] over the Windows SDK, as the files will be empty.
pub fn check_wdk_installed(sdk: &SdkInfo) -> Result<()> {
    let ntddk_path = sdk.get_include_dir().join("km").join("ntddk.h");
    if !ntddk_path.exists() {
        return Err(anyhow!(
            "{}\n\n\
             {}\n\
             {}\n\n\
             {}\n\
             {}\n\n\
             {}",
            "Windows Driver Kit (WDK) not installed (or present in environment)"
                .red()
                .bold(),
            "Kernel mode headers require WDK, which is separate from the SDK.".white(),
            format!("  Expected: {}", ntddk_path.display()).dimmed(),
            "To install WDK:".white(),
            format!(
                "  {} {}",
                "›".dimmed(),
                "https://learn.microsoft.com/en-us/windows-hardware/drivers/download-the-wdk"
                    .cyan()
            ),
            "Make sure to install the WDK version matching your SDK.".dimmed()
        ));
    }
    Ok(())
}

/* ---------------------------- Header generation --------------------------- */

/// A group of related `#include` headers, optionally preceded by raw
/// preprocessor directives.
///
/// `pre_lines` lets a group emit arbitrary preprocessor text — typically
/// `#undef X` — *between* the previous group's includes and this group's
/// includes. The user-mode `ntstatus.h` group uses it for the
/// `#undef WIN32_NO_STATUS` dance that lets `ntstatus.h`'s body emit
/// after the windows.h chain has already been processed with the gate on.
/// Kept empty (`&[]`) for groups that just `#include` headers.
struct HeaderGroup {
    comment: &'static str,
    pre_lines: &'static [&'static str],
    includes: &'static [&'static str],
}

/// Build a header string from structured components.
///
/// Order: guarded `#define`s, raw `#define`s, then grouped `#include`s.
/// Each group emits in order: a `//`-comment header, its `pre_lines` raw
/// directives, and its `#include` lines.
///
/// Defines must come first so they apply when each header chain is parsed;
/// in particular, kernel-mode `sdkddkver.h` (pulled in transitively by
/// `ntddk.h` → `ntdef.h`) sees `DECLSPEC_DEPRECATED_DDK` already defined
/// by `ntdef.h` and so populates the `DECLSPEC_DEPRECATED_DDK_WINXP`
/// family wdm.h depends on. Pulling `sdkddkver.h` in via a preamble
/// `#include` ran it before `ntdef.h` and left those macros undefined.
fn build_header(
    guarded_defines: &[(&str, &str)],
    raw_defines: &[(&str, &str)],
    groups: &[HeaderGroup],
) -> String {
    use std::fmt::Write;

    let mut out = String::new();

    for &(name, value) in guarded_defines {
        let _ = writeln!(out, "#ifndef {name}");
        let _ = writeln!(out, "#define {name} {value}");
        let _ = writeln!(out, "#endif");
    }

    for &(name, value) in raw_defines {
        let _ = writeln!(out, "#define {name} {value}");
    }
    out.push('\n');

    for group in groups {
        let _ = writeln!(out, "// {}", group.comment);
        for line in group.pre_lines {
            let _ = writeln!(out, "{line}");
        }
        for inc in group.includes {
            let _ = writeln!(out, "#include <{inc}>");
        }
        out.push('\n');
    }

    out
}

/// Generate the SDK header string for the given mode.
///
/// For [`SdkMode::User`], sets up user-mode headers and defines.
/// For [`SdkMode::Kernel`], sets up kernel headers and defines.
///
/// Both modes terminate with an `ntstatus.h` HeaderGroup that ensures
/// the full set of `STATUS_*` codes is available — user mode uses the
/// `WIN32_NO_STATUS` dance (see `user::GROUPS`); kernel mode pulls
/// `ntstatus.h` directly as a fallback for when ntifs.h's chain isn't
/// reachable (no WDK installed).
#[must_use]
pub fn sdk_header(mode: SdkMode) -> String {
    match mode {
        SdkMode::User => build_header(user::GUARDED_DEFINES, user::RAW_DEFINES, user::GROUPS),
        SdkMode::Kernel => {
            build_header(kernel::GUARDED_DEFINES, kernel::RAW_DEFINES, kernel::GROUPS)
        }
    }
}
