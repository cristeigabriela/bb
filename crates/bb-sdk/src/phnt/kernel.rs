//! Kernel-mode PHNT header configuration.

/// Mode defines.
pub(super) const DEFINES: &[(&str, &str)] = &[("_KERNEL_MODE", "1")];

/// Base includes (provides base types from WDK).
pub(super) const INCLUDES: &[&str] = &["ntdef.h"];
