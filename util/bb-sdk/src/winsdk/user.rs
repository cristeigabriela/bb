//! User-mode SDK header configuration.

use super::HeaderGroup;

/// `#ifndef`-guarded defines for user-mode.
pub(super) const GUARDED_DEFINES: &[(&str, &str)] = &[
    ("NTDDI_VERSION", "WDK_NTDDI_VERSION"),
    ("_WIN32_WINNT", "_WIN32_WINNT_WIN10"),
    ("WINVER", "_WIN32_WINNT"),
    ("WINAPI_FAMILY", "WINAPI_FAMILY_DESKTOP_APP"),
];

/// Grouped `#include` sections for user-mode.
pub(super) const GROUPS: &[HeaderGroup] = &[
    HeaderGroup {
        comment: "Core Windows",
        includes: &["windows.h", "winternl.h"],
    },
    HeaderGroup {
        comment: "Networking",
        includes: &["winhttp.h", "wininet.h"],
    },
    HeaderGroup {
        comment: "Process/Thread/Memory",
        includes: &[
            "tlhelp32.h",
            "psapi.h",
            "processthreadsapi.h",
            "memoryapi.h",
            "jobapi.h",
        ],
    },
    HeaderGroup {
        comment: "File/IO",
        includes: &["fileapi.h", "ioapiset.h", "namedpipeapi.h"],
    },
    HeaderGroup {
        comment: "Security",
        includes: &[
            "securitybaseapi.h",
            "securityappcontainer.h",
            "aclapi.h",
            "sddl.h",
            "wincrypt.h",
            "bcrypt.h",
            "ncrypt.h",
        ],
    },
    HeaderGroup {
        comment: "Shell",
        includes: &["shlobj.h", "shellapi.h", "shlwapi.h"],
    },
    HeaderGroup {
        comment: "COM/OLE",
        includes: &["objbase.h", "combaseapi.h"],
    },
    HeaderGroup {
        comment: "Debugging",
        includes: &["dbghelp.h", "errhandlingapi.h"],
    },
    HeaderGroup {
        comment: "Registry",
        includes: &["winreg.h"],
    },
    HeaderGroup {
        comment: "Services",
        includes: &["winsvc.h"],
    },
    HeaderGroup {
        comment: "Synchronization",
        includes: &["synchapi.h"],
    },
    HeaderGroup {
        comment: "System Info",
        includes: &["sysinfoapi.h", "powerbase.h"],
    },
    HeaderGroup {
        comment: "Handles",
        includes: &["handleapi.h"],
    },
    HeaderGroup {
        comment: "User/GDI (minimal)",
        includes: &["winuser.h"],
    },
];
