use anyhow::{anyhow, Result};
use colored::Colorize;
use std::env::var;
use std::path::PathBuf;

pub struct SdkInfo {
    include_dir: PathBuf,
    version: String,
}

impl SdkInfo {
    pub fn get_include_dir(&self) -> &PathBuf { &self.include_dir }
    pub fn get_version(&self) -> &str { &self.version }
}

fn missing_env_error() -> anyhow::Error {
    let msg = format!(
        "\n{}\n\n\
         {}\n\
         {}\n\n\
         {}\n\
         {}\n\n\
         {}\n\
         {}",
        "Windows SDK environment not configured".red().bold(),
        "The following environment variables are required:".white(),
        "  • WindowsSdkDir\n  • WindowsSDKLibVersion".yellow(),
        "To fix this, run one of:".white(),
        format!(
            "  {} {}\n  {} {}",
            "›".dimmed(),
            r#"vcvarsall.bat x64"#.cyan(),
            "›".dimmed(),
            r#"Developer Command Prompt for VS"#.cyan()
        ),
        "Or launch your terminal from Visual Studio.".dimmed(),
        format!(
            "See: {}",
            "https://learn.microsoft.com/en-us/cpp/build/building-on-the-command-line"
        ).dimmed()
    );
    anyhow!(msg)
}

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
            "{} SDK version {} not found at {}\n\n{}\n  {}",
            "Error:".red().bold(),
            version.yellow(),
            include_dir.display().to_string().dimmed(),
            "Available versions can be found in:".white(),
            sdk_dir.join("Include").display().to_string().cyan()
        ));
    }

    Ok(SdkInfo { include_dir, version })
}

pub fn sdk_header() -> &'static str {
    r#"
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

#include <windows.h>
#include <winternl.h>
#include <winsock2.h>
#include <ws2tcpip.h>
#include <tlhelp32.h>
#include <psapi.h>
#include <shlobj.h>
#include <shellapi.h>
#include <objbase.h>
#include <dbghelp.h>
"#
}
