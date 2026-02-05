//! Header configuration and parsing orchestration.
//!
//! This module provides a high-level API for configuring and parsing Windows headers.

use crate::arch::Arch;
use crate::parser::{parse_phnt, parse_winsdk};
use crate::phnt::PhntVersion;
use crate::winsdk::{SdkInfo, SdkMode, check_wdk_installed, get_sdk_info};
use anyhow::Result;
use clang::{Index, TranslationUnit};

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

    /// Create a PHNT configuration with the default version (Win11).
    ///
    /// This is a convenience method equivalent to `phnt(arch, PhntVersion::default(), mode)`.
    pub fn phnt_default(arch: Arch, mode: SdkMode) -> Result<Self> {
        Self::phnt(arch, PhntVersion::default(), mode)
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
    /// - Include paths for SDK directories
    /// - Architecture-specific preprocessor defines
    #[must_use]
    pub fn clang_args(&self) -> Vec<String> {
        let (arch, sdk) = match self {
            Self::WinSdk { arch, sdk, .. } => (*arch, sdk),
            Self::Phnt { arch, sdk, .. } => (*arch, sdk),
        };

        let mut args = vec!["-target".into(), arch.target_triple().into()];

        // Add SDK include paths
        for subdir in ["shared", "um", "ucrt", "km"] {
            args.push("-I".into());
            args.push(sdk.get_include_dir().join(subdir).to_string_lossy().into());
        }

        // Add architecture-specific defines
        args.extend(arch.defines().iter().map(|&s| s.into()));

        args
    }

    /// Parse headers with this configuration.
    ///
    /// # Arguments
    ///
    /// * `index` - A Clang index to use for parsing.
    ///
    /// Returns a `TranslationUnit` containing the parsed AST.
    pub fn parse<'a>(&self, index: &'a Index) -> Result<TranslationUnit<'a>> {
        let args = self.clang_args();
        match self {
            Self::WinSdk { sdk, mode, .. } => parse_winsdk(index, sdk, &args, *mode),
            Self::Phnt { version, mode, .. } => parse_phnt(index, &args, *version, *mode),
        }
    }
}
