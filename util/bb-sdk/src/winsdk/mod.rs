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

/// A group of related `#include` headers.
struct HeaderGroup {
    comment: &'static str,
    includes: &'static [&'static str],
}

/// Preamble includes shared across modes.
const PREAMBLE_INCLUDES: &[&str] = &["sdkddkver.h"];

/// Build a header string from structured components.
fn build_header(
    guarded_defines: &[(&str, &str)],
    raw_defines: &[(&str, &str)],
    groups: &[HeaderGroup],
) -> String {
    use std::fmt::Write;

    let mut out = String::new();

    for inc in PREAMBLE_INCLUDES {
        let _ = writeln!(out, "#include <{inc}>");
    }
    out.push('\n');

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
/// This will be later used by clang to parse the included contents.
#[must_use]
pub fn sdk_header(mode: SdkMode) -> String {
    match mode {
        SdkMode::User => build_header(user::GUARDED_DEFINES, &[], user::GROUPS),
        SdkMode::Kernel => {
            build_header(kernel::GUARDED_DEFINES, kernel::RAW_DEFINES, kernel::GROUPS)
        }
    }
}
