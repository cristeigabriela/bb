//! Module for working with Windows SDK from a developer command prompt environment.

use anyhow::{Result, anyhow};
use colored::Colorize;
use std::env::var;
use std::path::PathBuf;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, clap::ValueEnum)]
pub enum SdkMode {
    #[default]
    User,
    Kernel,
}

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

/// Generate a [`SdkInfo`] structure from the current environment.
///
/// Will fail if environment is not set up with Windows SDK.
///
/// # Arguments
///
/// * `override_version`: Optionally provide a different Windows SDK version to analyze.
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

/// Responsible for verifying if WDK is installed (or present in environment).
///
/// Will warn the user through error propagation if it is not, and they are
/// trying to invoke with [`SdkMode::Kernel`] over the Windows SDK, as the files
/// will be empty.
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

/// Obtain a string of an unsaved header file which sets up the appropriate environment.
///
/// For [`SdkMode::User`], it will set up a build for user-mode, using user-mode headers
/// and defines.
///
/// For [`SdkMode::Kernel`], it will set up a build for kernel, using kernel headers
/// and defines.
///
/// This will be later used by clang to parse the included contents.
#[must_use]
pub const fn sdk_header(mode: SdkMode) -> &'static str {
    match mode {
        SdkMode::User => USER_MODE_HEADER,
        SdkMode::Kernel => KERNEL_MODE_HEADER,
    }
}

const USER_MODE_HEADER: &str = r"
#include <sdkddkver.h>

#ifndef NTDDI_VERSION
#define NTDDI_VERSION WDK_NTDDI_VERSION
#endif
#ifndef _WIN32_WINNT
#define _WIN32_WINNT _WIN32_WINNT_WIN10
#endif
#ifndef WINVER
#define WINVER _WIN32_WINNT
#endif
#ifndef WINAPI_FAMILY
#define WINAPI_FAMILY WINAPI_FAMILY_DESKTOP_APP
#endif

// Core Windows
#include <windows.h>
#include <winternl.h>

// Networking
#include <winhttp.h>
#include <wininet.h>

// Process/Thread/Memory
#include <tlhelp32.h>
#include <psapi.h>
#include <processthreadsapi.h>
#include <memoryapi.h>
#include <jobapi.h>

// File/IO
#include <fileapi.h>
#include <ioapiset.h>
#include <namedpipeapi.h>

// Security
#include <securitybaseapi.h>
#include <securityappcontainer.h>
#include <aclapi.h>
#include <sddl.h>
#include <wincrypt.h>
#include <bcrypt.h>
#include <ncrypt.h>

// Shell
#include <shlobj.h>
#include <shellapi.h>
#include <shlwapi.h>

// COM/OLE
#include <objbase.h>
#include <combaseapi.h>

// Debugging
#include <dbghelp.h>
#include <errhandlingapi.h>

// Registry
#include <winreg.h>

// Services
#include <winsvc.h>

// Synchronization
#include <synchapi.h>

// System Info
#include <sysinfoapi.h>
#include <powerbase.h>

// Handles
#include <handleapi.h>

// User/GDI (minimal)
#include <winuser.h>
";

const KERNEL_MODE_HEADER: &str = r"
#include <sdkddkver.h>

#ifndef NTDDI_VERSION
#define NTDDI_VERSION WDK_NTDDI_VERSION
#endif

// Kernel mode indicator
#define _KERNEL_MODE 1

// Core kernel headers
#include <ntddk.h>
#include <wdm.h>

// File systems and more internal structures
#include <ntifs.h>

// Safe string functions
#include <ntstrsafe.h>

// Winsock Kernel
#include <wsk.h>

// Filter Manager (minifilters)
#include <fltkernel.h>

// Auxiliary Kernel-Mode Library
#include <aux_klib.h>

// Process/thread access
#include <ntddk.h>
";
