//! Header configuration and parsing orchestration.
//!
//! This module provides a high-level API for configuring and parsing Windows headers.

use crate::arch::{Arch, ArchDefines};
use crate::parser::{parse_phnt, parse_winsdk};
use crate::phnt::PhntVersion;
use crate::winsdk::{SdkInfo, SdkMode, check_wdk_installed, get_sdk_info};
use anyhow::Result;
use clang::{Index, TranslationUnit};

/* ────────────────────────────────── Types ───────────────────────────────── */

/// Const-friendly discriminator over [`HeaderConfig`].
///
/// Mirrors the variant set without any runtime data so it can live in
/// `const` initializers — used by
/// [`crate::winsdk::HeaderGroup::skip_for`] to opt groups out of one
/// build kind or the other. Get the kind for a runtime config via
/// [`HeaderConfig::kind`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HeaderConfigKind {
    /// Plain `--winsdk` build (no phnt overlay).
    WinSdk,
    /// `--phnt` build (phnt.h overlaid on the SDK chain).
    Phnt,
}

/// Configuration for parsing Windows headers.
///
/// This enum encapsulates all the necessary configuration for parsing either
/// Windows SDK headers or PHNT (Process Hacker NT) headers.
#[derive(Debug, Clone)]
pub enum HeaderConfig {
    /// Windows SDK header configuration.
    WinSdk {
        /// SDK information (include paths, version).
        sdk: SdkInfo,
        /// Target architecture.
        arch: Arch,
        /// User or kernel mode.
        mode: SdkMode,
    },
    /// PHNT header configuration.
    Phnt {
        /// SDK information (needed for base Windows types).
        sdk: SdkInfo,
        /// Target architecture.
        arch: Arch,
        /// PHNT version (corresponds to Windows release).
        version: PhntVersion,
        /// User or kernel mode.
        mode: SdkMode,
    },
}

impl HeaderConfig {
    /// Create a Windows SDK configuration with default version from environment.
    ///
    /// # Arguments
    ///
    /// * `arch` - Target architecture (see [`Arch`]).
    /// * `mode` - User or kernel mode.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - Windows SDK is not found in environment
    /// - Kernel mode is requested but WDK is not installed
    pub fn winsdk(arch: Arch, mode: SdkMode) -> Result<Self> {
        let sdk = get_sdk_info(None)?;
        if mode == SdkMode::Kernel {
            check_wdk_installed(&sdk)?;
        }
        Ok(Self::WinSdk { sdk, arch, mode })
    }

    /// Create a Windows SDK configuration with a specific version.
    ///
    /// # Arguments
    ///
    /// * `version` - SDK version string (e.g., "10.0.22621.0").
    /// * `arch` - Target architecture.
    /// * `mode` - User or kernel mode.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - Specified SDK version is not found
    /// - Kernel mode is requested but WDK is not installed
    pub fn winsdk_version(version: &str, arch: Arch, mode: SdkMode) -> Result<Self> {
        let sdk = get_sdk_info(Some(version))?;
        if mode == SdkMode::Kernel {
            check_wdk_installed(&sdk)?;
        }
        Ok(Self::WinSdk { sdk, arch, mode })
    }

    /// Create a PHNT configuration with the specified version.
    ///
    /// PHNT (Process Hacker NT) provides internal Windows structure definitions
    /// not available in the public SDK.
    ///
    /// # Arguments
    ///
    /// * `arch` - Target architecture.
    /// * `version` - PHNT version (see [`PhntVersion`]).
    /// * `mode` - User or kernel mode.
    ///
    /// # Errors
    ///
    /// Returns an error if Windows SDK is not found (needed for base types).
    pub fn phnt(arch: Arch, version: PhntVersion, mode: SdkMode) -> Result<Self> {
        let sdk = get_sdk_info(None)?;
        Ok(Self::Phnt {
            sdk,
            arch,
            version,
            mode,
        })
    }

    /// Get the target architecture for this configuration.
    #[must_use]
    pub const fn arch(&self) -> Arch {
        match self {
            Self::WinSdk { arch, .. } | Self::Phnt { arch, .. } => *arch,
        }
    }

    /// Get the SDK mode (user or kernel) for this configuration.
    #[must_use]
    pub const fn mode(&self) -> SdkMode {
        match self {
            Self::WinSdk { mode, .. } | Self::Phnt { mode, .. } => *mode,
        }
    }

    /// Get the discriminator kind (WinSdk vs Phnt) for this configuration.
    ///
    /// Used to filter [`crate::winsdk::HeaderGroup`]s with `skip_for`
    /// against the active build context without exposing the runtime
    /// payload (`SdkInfo`, `PhntVersion`, …) to the matcher.
    #[must_use]
    pub const fn kind(&self) -> HeaderConfigKind {
        match self {
            Self::WinSdk { .. } => HeaderConfigKind::WinSdk,
            Self::Phnt { .. } => HeaderConfigKind::Phnt,
        }
    }

    /// Get the SDK information for this configuration.
    #[must_use]
    pub const fn sdk(&self) -> &SdkInfo {
        match self {
            Self::WinSdk { sdk, .. } | Self::Phnt { sdk, .. } => sdk,
        }
    }

    /// Build the Clang arguments for this configuration.
    ///
    /// This includes:
    /// - Target triple for the architecture
    /// - Include paths for SDK directories (and the newest installed WDF
    ///   flavor matching the mode — KMDF for kernel, UMDF for user)
    /// - Architecture-specific preprocessor defines
    /// - `KMDF_VERSION_MAJOR` / `KMDF_VERSION_MINOR` (or UMDF_*) defines when
    ///   the matching WDF flavor is installed
    #[must_use]
    pub fn clang_args(&self) -> Vec<String> {
        let (arch, sdk, mode) = match self {
            Self::WinSdk { arch, sdk, mode }
            | Self::Phnt {
                arch, sdk, mode, ..
            } => (*arch, sdk, *mode),
        };

        let mut args = vec!["-target".into(), arch.target_triple().into()];

        // Add SDK include paths
        for subdir in ["shared", "um", "ucrt", "km"] {
            args.push("-I".into());
            args.push(sdk.get_include_dir().join(subdir).to_string_lossy().into());
        }

        // Add WDF include path + version defines for whichever flavor
        // matches the mode. KMDF/UMDF have separate version namespaces.
        let (flavor, prefix) = match mode {
            crate::SdkMode::Kernel => ("kmdf", "KMDF"),
            crate::SdkMode::User => ("umdf", "UMDF"),
        };
        if let Some(wdf) = sdk.wdf_latest(flavor) {
            args.push("-I".into());
            args.push(wdf.include_dir.to_string_lossy().into());
            args.push(format!("-D{prefix}_VERSION_MAJOR={}", wdf.major));
            args.push(format!("-D{prefix}_VERSION_MINOR={}", wdf.minor));
        }

        // NetAdapterCx (kernel only). Adds two include paths: the per-version
        // public headers and the `shared/netcx/shared/<ver>/` tree that owns
        // the `net/*.h` types. `netfuncenum.h` also requires NET_VERSION_*
        // be defined before include.
        if mode == crate::SdkMode::Kernel
            && let Some(netcx) = sdk.netcx_latest()
        {
            args.push("-I".into());
            args.push(netcx.include_dir.to_string_lossy().into());
            args.push(format!("-DNET_VERSION_MAJOR={}", netcx.major));
            args.push(format!("-DNET_VERSION_MINOR={}", netcx.minor));
            if let Some(shared) = sdk.netcx_shared_dir() {
                args.push("-I".into());
                args.push(shared.to_string_lossy().into());
            }
        }

        // Add architecture-specific defines
        args.extend(arch.defines().iter().map(|&s| s.into()));

        args
    }

    /// Parse headers with this configuration.
    ///
    /// When `detailed_preprocessing` is true, the translation unit records preprocessor
    /// macro definitions.
    ///
    /// This is needed for constant/macro extraction but not for struct-only parsing.
    ///
    /// # Errors
    ///
    /// Will return an `Err` in scenarios where any of the SDK prerequisites are missing.
    pub fn parse<'a>(
        &self,
        index: &'a Index,
        detailed_preprocessing: bool,
    ) -> Result<TranslationUnit<'a>> {
        let args = self.clang_args();
        let kind = self.kind();
        match self {
            Self::WinSdk { sdk, mode, .. } => {
                parse_winsdk(index, sdk, &args, *mode, kind, detailed_preprocessing)
            }
            Self::Phnt { version, mode, .. } => {
                parse_phnt(index, &args, *version, *mode, detailed_preprocessing)
            }
        }
    }
}
