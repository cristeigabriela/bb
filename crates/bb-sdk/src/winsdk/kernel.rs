//! Kernel-mode SDK header configuration.

use super::HeaderGroup;

/// `#ifndef`-guarded defines for kernel-mode.
pub(super) const GUARDED_DEFINES: &[(&str, &str)] = &[("NTDDI_VERSION", "WDK_NTDDI_VERSION")];

/// Raw (unguarded) defines for kernel-mode.
pub(super) const RAW_DEFINES: &[(&str, &str)] = &[("_KERNEL_MODE", "1")];

/// Grouped `#include` sections for kernel-mode.
pub(super) const GROUPS: &[HeaderGroup] = &[
    HeaderGroup {
        comment: "Core kernel headers",
        includes: &["ntddk.h", "wdm.h"],
    },
    HeaderGroup {
        comment: "File systems and more internal structures",
        includes: &["ntifs.h"],
    },
    HeaderGroup {
        comment: "Safe string functions",
        includes: &["ntstrsafe.h"],
    },
    HeaderGroup {
        comment: "Winsock Kernel",
        includes: &["wsk.h"],
    },
    HeaderGroup {
        comment: "Filter Manager (minifilters)",
        includes: &["fltkernel.h"],
    },
    HeaderGroup {
        comment: "Auxiliary Kernel-Mode Library",
        includes: &["aux_klib.h"],
    },
    HeaderGroup {
        comment: "Process/thread access",
        includes: &["ntddk.h"],
    },
];
