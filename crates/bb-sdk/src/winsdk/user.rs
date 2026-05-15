//! User-mode SDK header configuration.

use super::HeaderGroup;

/// `#ifndef`-guarded defines for user-mode.
pub(super) const GUARDED_DEFINES: &[(&str, &str)] = &[
    ("NTDDI_VERSION", "WDK_NTDDI_VERSION"),
    ("_WIN32_WINNT", "_WIN32_WINNT_WIN10"),
    ("WINVER", "_WIN32_WINNT"),
    ("WINAPI_FAMILY", "WINAPI_FAMILY_DESKTOP_APP"),
];

/// Raw (unguarded) defines for user-mode.
///
/// `WIN32_NO_STATUS` tells `winnt.h` to skip its small inline subset of
/// `STATUS_*` macros so the full set can come from `ntstatus.h` without
/// redefinition errors. The final HeaderGroup in [`GROUPS`] undoes this
/// just before pulling `ntstatus.h` (via `pre_lines:
/// &["#undef WIN32_NO_STATUS"]`), so `ntstatus.h`'s body actually emits.
pub(super) const RAW_DEFINES: &[(&str, &str)] = &[("WIN32_NO_STATUS", "")];

/// Grouped `#include` sections for user-mode.
///
/// `winsock2.h` / `ws2tcpip.h` must come **before** `windows.h` — the
/// legacy `windows.h` chain pulls `winsock.h` (Winsock 1) which
/// conflicts with the Winsock 2 declarations.
pub(super) const GROUPS: &[HeaderGroup] = &[
    HeaderGroup {
        comment: "Winsock 2 (must precede windows.h)",
        pre_lines: &[],
        includes: &["winsock2.h", "ws2tcpip.h", "mswsock.h"],
    },
    HeaderGroup {
        comment: "Core Windows",
        pre_lines: &[],
        includes: &["windows.h", "winternl.h"],
    },
    // NTSTATUS codes must land *here*, not at the end of GROUPS.
    //
    // `RAW_DEFINES` set `WIN32_NO_STATUS` so winnt.h (pulled by
    // windows.h above) skipped its tiny inline `STATUS_*` defines.
    // Undo the gate now and emit `ntstatus.h`'s full ~2800-code set
    // before any downstream header references symbols that live in
    // ntstatus.h. `dbgeng.h` in particular uses `DBG_COMMAND_EXCEPTION`,
    // which only ntstatus.h provides — without this group at this
    // position, dbgeng.h's parse errors out.
    HeaderGroup {
        comment: "NTSTATUS codes (full set)",
        pre_lines: &["#undef WIN32_NO_STATUS"],
        includes: &["ntstatus.h"],
    },
    HeaderGroup {
        comment: "Networking",
        pre_lines: &[],
        includes: &["winhttp.h", "wininet.h", "iphlpapi.h", "icmpapi.h"],
    },
    HeaderGroup {
        comment: "Process/Thread/Memory",
        pre_lines: &[],
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
        pre_lines: &[],
        includes: &["fileapi.h", "ioapiset.h", "namedpipeapi.h"],
    },
    HeaderGroup {
        comment: "Security",
        pre_lines: &[],
        includes: &[
            "securitybaseapi.h",
            "securityappcontainer.h",
            "aclapi.h",
            "sddl.h",
            "wincrypt.h",
            "bcrypt.h",
            "ncrypt.h",
            "authz.h",
        ],
    },
    HeaderGroup {
        comment: "Shell",
        pre_lines: &[],
        includes: &["shlobj.h", "shellapi.h", "shlwapi.h"],
    },
    HeaderGroup {
        comment: "COM/OLE",
        pre_lines: &[],
        includes: &["objbase.h", "combaseapi.h", "oleauto.h"],
    },
    HeaderGroup {
        comment: "Debugging",
        pre_lines: &[],
        includes: &["dbghelp.h", "errhandlingapi.h"],
    },
    HeaderGroup {
        comment: "Registry",
        pre_lines: &[],
        includes: &["winreg.h"],
    },
    HeaderGroup {
        comment: "Services",
        pre_lines: &[],
        includes: &["winsvc.h"],
    },
    HeaderGroup {
        comment: "Synchronization",
        pre_lines: &[],
        includes: &["synchapi.h"],
    },
    HeaderGroup {
        comment: "System Info",
        pre_lines: &[],
        includes: &["sysinfoapi.h", "powerbase.h", "appmodel.h"],
    },
    HeaderGroup {
        comment: "Handles",
        pre_lines: &[],
        includes: &["handleapi.h"],
    },
    HeaderGroup {
        comment: "User / GDI / common controls",
        pre_lines: &[],
        includes: &["winuser.h", "commctrl.h", "commdlg.h"],
    },
    HeaderGroup {
        comment: "WTS / user-info",
        // ntsecapi.h omitted — its LSA_UNICODE_STRING / LSA_STRING typedefs
        // conflict with winternl.h's UNICODE_STRING / STRING.
        pre_lines: &[],
        includes: &["wtsapi32.h", "lmaccess.h"],
    },
    HeaderGroup {
        comment: "Setup / Cfgmgr / WMI",
        pre_lines: &[],
        includes: &["setupapi.h", "cfgmgr32.h", "wbemcli.h"],
    },
    HeaderGroup {
        comment: "Media Foundation",
        pre_lines: &[],
        includes: &["mfapi.h", "mfidl.h"],
    },
    HeaderGroup {
        comment: "Core Audio (WASAPI / device enumeration / policy)",
        pre_lines: &[],
        includes: &["mmdeviceapi.h", "audioclient.h", "audiopolicy.h"],
    },
    HeaderGroup {
        comment: "Direct3D (11/12) + DXGI",
        pre_lines: &[],
        includes: &["dxgi1_6.h", "d3d11.h", "d3d12.h"],
    },
    HeaderGroup {
        comment: "Shell objects (COM)",
        pre_lines: &[],
        includes: &["shobjidl_core.h", "shobjidl.h"],
    },
    HeaderGroup {
        comment: "UI Automation",
        pre_lines: &[],
        includes: &["uiautomation.h"],
    },
    HeaderGroup {
        comment: "Bluetooth + XInput",
        pre_lines: &[],
        includes: &["bluetoothapis.h", "xinput.h"],
    },
    HeaderGroup {
        comment: "ETW / TDH tracing",
        pre_lines: &[],
        includes: &["evntrace.h", "evntprov.h", "tdh.h"],
    },
    HeaderGroup {
        comment: "Background transfer (BITS) + Windows Update",
        pre_lines: &[],
        includes: &["bits.h", "wuapi.h"],
    },
    HeaderGroup {
        comment: "Certificate enrollment",
        pre_lines: &[],
        includes: &["certenroll.h"],
    },
    HeaderGroup {
        comment: "DirectShow",
        pre_lines: &[],
        includes: &["strmif.h"],
    },
    // TAPI omitted — tapi3if.h pulls tapi.h, which has C-mode parse errors.
    HeaderGroup {
        comment: "Windows Image Acquisition",
        pre_lines: &[],
        includes: &["wia.h"],
    },
    HeaderGroup {
        comment: "Windows Media Player",
        pre_lines: &[],
        includes: &["wmp.h"],
    },
    HeaderGroup {
        comment: "AzMan / COM+ / XPS / Tablet ink / TextObjectModel",
        pre_lines: &[],
        includes: &[
            "azroles.h",
            "comsvcs.h",
            "xpsobjectmodel.h",
            "msinkaut.h",
            "tom.h",
        ],
    },
    HeaderGroup {
        comment: "Active Directory",
        pre_lines: &[],
        includes: &["iads.h", "adshlp.h"],
    },
    HeaderGroup {
        comment: "Performance counters",
        pre_lines: &[],
        includes: &["pdh.h"],
    },
    HeaderGroup {
        comment: "Task Scheduler",
        pre_lines: &[],
        includes: &["taskschd.h"],
    },
    HeaderGroup {
        comment: "Text services framework / performance logs",
        // dwrite.h omitted — uses C++ syntax (`static_cast`, untagged
        // struct references) that won't parse in C mode.
        pre_lines: &[],
        includes: &["msctf.h", "pla.h"],
    },
    HeaderGroup {
        comment: "Property variants + intsafe helpers",
        pre_lines: &[],
        includes: &["propvarutil.h", "intsafe.h"],
    },
    HeaderGroup {
        comment: "LDAP / firewall (netfw, fwpmu)",
        // webservices.h omitted — declares two empty enums clang rejects in C mode.
        pre_lines: &[],
        includes: &["winldap.h", "netfw.h", "fwpmu.h"],
    },
    HeaderGroup {
        comment: "Video for Windows / IMAPI / Failover Cluster / Management Infra",
        pre_lines: &[],
        includes: &["vfw.h", "imapi2.h", "clusapi.h", "mi.h"],
    },
    HeaderGroup {
        comment: "Debugger engine (dbgeng) — user-mode debugging",
        pre_lines: &[],
        includes: &["dbgeng.h"],
    },
    // wudfddi.h omitted — lives only in legacy `wdf/umdf/1.x/` trees that
    // aren't on the include path; modern UMDF 2.x uses wdf.h directly.
    HeaderGroup {
        comment: "MF read/write + media engine + D3D compiler",
        pre_lines: &[],
        includes: &["mfreadwrite.h", "mfmediaengine.h", "d3dcompiler.h"],
    },
    HeaderGroup {
        comment: "Property system",
        pre_lines: &[],
        includes: &["propsys.h"],
    },
    HeaderGroup {
        comment: "WLAN + RAS",
        pre_lines: &[],
        includes: &["wlanapi.h", "ras.h", "rasdlg.h"],
    },
    HeaderGroup {
        comment: "Power + Group Policy",
        pre_lines: &[],
        includes: &["powrprof.h", "gpedit.h"],
    },
    // security.h omitted — its inner `sspi.h` requires one of
    // SECURITY_WIN32 / SECURITY_KERNEL / SECURITY_MAC be defined first.
    // We don't currently expose a place to set that per-mode.
    HeaderGroup {
        comment: "DHCP / DNS / MSTCPIP",
        pre_lines: &[],
        includes: &["dhcpsapi.h", "dhcpcsdk.h", "windns.h", "mstcpip.h"],
    },
    HeaderGroup {
        comment: "MSI (installer)",
        pre_lines: &[],
        includes: &["msi.h", "msiquery.h"],
    },
    HeaderGroup {
        comment: "MPRAPI + RTMv2",
        // p2p.h omitted — not shipped in modern SDK trees.
        pre_lines: &[],
        includes: &["mprapi.h", "rtmv2.h"],
    },
    HeaderGroup {
        comment: "HBA",
        // tapi.h / tspi.h omitted — tapi.h has C-mode parse errors that
        // cascade into anything that pulls it (tspi.h, winfax.h, tapi3if.h).
        pre_lines: &[],
        includes: &["hbaapi.h"],
    },
    HeaderGroup {
        comment: "NTMS",
        // resapi.h omitted — typedef redefinition of PHANDLER_ROUTINE
        // (PVOID vs DWORD_PTR) against services API.
        // iscsidsc.h omitted — references ISDSC_STATUS without including
        // the header that defines it.
        pre_lines: &[],
        includes: &["ntmsapi.h"],
    },
    HeaderGroup {
        comment: "Theming + DWM + Direct Composition",
        pre_lines: &[],
        includes: &["uxtheme.h", "dwmapi.h"],
    },
    HeaderGroup {
        comment: "Strsafe + SafeInt helpers",
        pre_lines: &[],
        includes: &["strsafe.h"],
    },
    HeaderGroup {
        comment: "HTTP server / WinSNMP / WinCred / Userenv",
        // winfax.h omitted — pulls tapi.h transitively.
        pre_lines: &[],
        includes: &["http.h", "winsnmp.h", "wincred.h", "userenv.h"],
    },
    HeaderGroup {
        comment: "ICM (color management) + NTDS",
        pre_lines: &[],
        includes: &["icm.h", "ntdsapi.h"],
    },
    HeaderGroup {
        comment: "Common log file system + USP10",
        pre_lines: &[],
        includes: &["clfsw32.h", "usp10.h"],
    },
    HeaderGroup {
        comment: "DRM",
        // winddi.h omitted — pulls ddrawi.h -> dvp.h whose typedefs collide
        // with the chain; also redefines HSEMAPHORE against the GDI version.
        // rpcproxy.h omitted — uses anonymous struct declarations clang
        // rejects in C mode.
        pre_lines: &[],
        includes: &["msdrm.h"],
    },
    HeaderGroup {
        comment: "Biometrics + Software Licensing + WSA SPI + Event Log",
        pre_lines: &[],
        includes: &["winbio.h", "slpublic.h", "ws2spi.h", "winevt.h"],
    },
    HeaderGroup {
        comment: "KTM / WER / SAPI / Cloud Files / Diagnostic Data",
        pre_lines: &[],
        includes: &[
            "ktmw32.h",
            "werapi.h",
            "sapi.h",
            "cfapi.h",
            "diagnosticdataquery.h",
        ],
    },
    HeaderGroup {
        comment: "Property sheets + SNMP + WDS client",
        pre_lines: &[],
        includes: &["prsht.h", "snmp.h", "wdsclientapi.h"],
    },
    HeaderGroup {
        comment: "WinUSB",
        // wsman.h omitted — requires `WSMAN_API_VERSION_1_0` or `_1_1`
        // be defined before include; not wired up.
        pre_lines: &[],
        includes: &["winusb.h"],
    },
    HeaderGroup {
        comment: "Virtual disk + Filter Manager user / Catalog / Peer Dist / DRT",
        pre_lines: &[],
        includes: &["virtdisk.h", "fltuser.h", "mscat.h", "peerdist.h", "drt.h"],
    },
    HeaderGroup {
        comment: "Network providers + EAP + Interaction context + WDS PXE",
        // rtworkq.h omitted — uses C++ syntax (untagged interface names).
        pre_lines: &[],
        includes: &[
            "npapi.h",
            "eapmethodpeerapis.h",
            "interactioncontext.h",
            "wdspxe.h",
        ],
    },
    HeaderGroup {
        comment: "Trace logging / DS / Perf",
        // imagehlp.h omitted — its `_LOADED_IMAGE`, `_MODLOAD_DATA` etc.
        // collide with dbghelp.h (which is the modern superset).
        pre_lines: &[],
        includes: &["traceloggingprovider.h", "dsgetdc.h", "perflib.h"],
    },
    HeaderGroup {
        comment: "LM DFS / Pathcch / WinWlx / High-level monitor / Traffic",
        pre_lines: &[],
        includes: &[
            "lmdfs.h",
            "pathcch.h",
            "winwlx.h",
            "highlevelmonitorconfigurationapi.h",
            "traffic.h",
        ],
    },
    HeaderGroup {
        comment: "Magnification / ProjFS / RO error / WDS TP",
        pre_lines: &[],
        includes: &[
            "magnification.h",
            "projectedfslib.h",
            "roerrorapi.h",
            "wdstpdi.h",
        ],
    },
    HeaderGroup {
        comment: "FCI / WinTrust / CryptXML",
        // sphelper.h omitted — heavy C++ speech-API templates that
        // balloon clang parse time from ~9s to >4 minutes.
        pre_lines: &[],
        includes: &["fci.h", "wintrust.h", "cryptxml.h"],
    },
];
