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
///
/// `winsock2.h` / `ws2tcpip.h` must come **before** `windows.h` — the
/// legacy `windows.h` chain pulls `winsock.h` (Winsock 1) which
/// conflicts with the Winsock 2 declarations.
pub(super) const GROUPS: &[HeaderGroup] = &[
    HeaderGroup {
        comment: "Winsock 2 (must precede windows.h)",
        includes: &["winsock2.h", "ws2tcpip.h", "mswsock.h"],
    },
    HeaderGroup {
        comment: "Core Windows",
        includes: &["windows.h", "winternl.h"],
    },
    HeaderGroup {
        comment: "Networking",
        includes: &["winhttp.h", "wininet.h", "iphlpapi.h", "icmpapi.h"],
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
            "authz.h",
        ],
    },
    HeaderGroup {
        comment: "Shell",
        includes: &["shlobj.h", "shellapi.h", "shlwapi.h"],
    },
    HeaderGroup {
        comment: "COM/OLE",
        includes: &["objbase.h", "combaseapi.h", "oleauto.h"],
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
        includes: &["sysinfoapi.h", "powerbase.h", "appmodel.h"],
    },
    HeaderGroup {
        comment: "Handles",
        includes: &["handleapi.h"],
    },
    HeaderGroup {
        comment: "User / GDI / common controls",
        includes: &["winuser.h", "commctrl.h", "commdlg.h"],
    },
    HeaderGroup {
        comment: "WTS / user-info",
        // ntsecapi.h omitted — its LSA_UNICODE_STRING / LSA_STRING typedefs
        // conflict with winternl.h's UNICODE_STRING / STRING.
        includes: &["wtsapi32.h", "lmaccess.h"],
    },
    HeaderGroup {
        comment: "Setup / Cfgmgr / WMI",
        includes: &["setupapi.h", "cfgmgr32.h", "wbemcli.h"],
    },
    HeaderGroup {
        comment: "Media Foundation",
        includes: &["mfapi.h", "mfidl.h"],
    },
    HeaderGroup {
        comment: "Core Audio (WASAPI / device enumeration / policy)",
        includes: &["mmdeviceapi.h", "audioclient.h", "audiopolicy.h"],
    },
    HeaderGroup {
        comment: "Direct3D (11/12) + DXGI",
        includes: &["dxgi1_6.h", "d3d11.h", "d3d12.h"],
    },
    HeaderGroup {
        comment: "Shell objects (COM)",
        includes: &["shobjidl_core.h", "shobjidl.h"],
    },
    HeaderGroup {
        comment: "UI Automation",
        includes: &["uiautomation.h"],
    },
    HeaderGroup {
        comment: "Bluetooth + XInput",
        includes: &["bluetoothapis.h", "xinput.h"],
    },
    HeaderGroup {
        comment: "ETW / TDH tracing",
        includes: &["evntrace.h", "evntprov.h", "tdh.h"],
    },
    HeaderGroup {
        comment: "Background transfer (BITS) + Windows Update",
        includes: &["bits.h", "wuapi.h"],
    },
    HeaderGroup {
        comment: "Certificate enrollment",
        includes: &["certenroll.h"],
    },
    HeaderGroup {
        comment: "DirectShow",
        includes: &["strmif.h"],
    },
    // TAPI omitted — tapi3if.h pulls tapi.h, which has C-mode parse errors.
    HeaderGroup {
        comment: "Windows Image Acquisition",
        includes: &["wia.h"],
    },
    HeaderGroup {
        comment: "Windows Media Player",
        includes: &["wmp.h"],
    },
    HeaderGroup {
        comment: "AzMan / COM+ / XPS / Tablet ink / TextObjectModel",
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
        includes: &["iads.h", "adshlp.h"],
    },
    HeaderGroup {
        comment: "Performance counters",
        includes: &["pdh.h"],
    },
    HeaderGroup {
        comment: "Task Scheduler",
        includes: &["taskschd.h"],
    },
    HeaderGroup {
        comment: "Text services framework / performance logs",
        // dwrite.h omitted — uses C++ syntax (`static_cast`, untagged
        // struct references) that won't parse in C mode.
        includes: &["msctf.h", "pla.h"],
    },
    HeaderGroup {
        comment: "Property variants + intsafe helpers",
        includes: &["propvarutil.h", "intsafe.h"],
    },
    HeaderGroup {
        comment: "LDAP / firewall (netfw, fwpmu)",
        // webservices.h omitted — declares two empty enums clang rejects in C mode.
        includes: &["winldap.h", "netfw.h", "fwpmu.h"],
    },
    HeaderGroup {
        comment: "Video for Windows / IMAPI / Failover Cluster / Management Infra",
        includes: &["vfw.h", "imapi2.h", "clusapi.h", "mi.h"],
    },
    HeaderGroup {
        comment: "Debugger engine (dbgeng) — user-mode debugging",
        includes: &["dbgeng.h"],
    },
    // wudfddi.h omitted — lives only in legacy `wdf/umdf/1.x/` trees that
    // aren't on the include path; modern UMDF 2.x uses wdf.h directly.
    HeaderGroup {
        comment: "MF read/write + media engine + D3D compiler",
        includes: &["mfreadwrite.h", "mfmediaengine.h", "d3dcompiler.h"],
    },
    HeaderGroup {
        comment: "Property system",
        includes: &["propsys.h"],
    },
    HeaderGroup {
        comment: "WLAN + RAS",
        includes: &["wlanapi.h", "ras.h", "rasdlg.h"],
    },
    HeaderGroup {
        comment: "Power + Group Policy",
        includes: &["powrprof.h", "gpedit.h"],
    },
    // security.h omitted — its inner `sspi.h` requires one of
    // SECURITY_WIN32 / SECURITY_KERNEL / SECURITY_MAC be defined first.
    // We don't currently expose a place to set that per-mode.
    HeaderGroup {
        comment: "DHCP / DNS / MSTCPIP",
        includes: &["dhcpsapi.h", "dhcpcsdk.h", "windns.h", "mstcpip.h"],
    },
    HeaderGroup {
        comment: "MSI (installer)",
        includes: &["msi.h", "msiquery.h"],
    },
    HeaderGroup {
        comment: "MPRAPI + RTMv2",
        // p2p.h omitted — not shipped in modern SDK trees.
        includes: &["mprapi.h", "rtmv2.h"],
    },
    HeaderGroup {
        comment: "HBA",
        // tapi.h / tspi.h omitted — tapi.h has C-mode parse errors that
        // cascade into anything that pulls it (tspi.h, winfax.h, tapi3if.h).
        includes: &["hbaapi.h"],
    },
    HeaderGroup {
        comment: "NTMS",
        // resapi.h omitted — typedef redefinition of PHANDLER_ROUTINE
        // (PVOID vs DWORD_PTR) against services API.
        // iscsidsc.h omitted — references ISDSC_STATUS without including
        // the header that defines it.
        includes: &["ntmsapi.h"],
    },
    HeaderGroup {
        comment: "Theming + DWM + Direct Composition",
        includes: &["uxtheme.h", "dwmapi.h"],
    },
    HeaderGroup {
        comment: "Strsafe + SafeInt helpers",
        includes: &["strsafe.h"],
    },
    HeaderGroup {
        comment: "HTTP server / WinSNMP / WinCred / Userenv",
        // winfax.h omitted — pulls tapi.h transitively.
        includes: &["http.h", "winsnmp.h", "wincred.h", "userenv.h"],
    },
    HeaderGroup {
        comment: "ICM (color management) + NTDS",
        includes: &["icm.h", "ntdsapi.h"],
    },
    HeaderGroup {
        comment: "Common log file system + USP10",
        includes: &["clfsw32.h", "usp10.h"],
    },
    HeaderGroup {
        comment: "DRM",
        // winddi.h omitted — pulls ddrawi.h -> dvp.h whose typedefs collide
        // with the chain; also redefines HSEMAPHORE against the GDI version.
        // rpcproxy.h omitted — uses anonymous struct declarations clang
        // rejects in C mode.
        includes: &["msdrm.h"],
    },
    HeaderGroup {
        comment: "Biometrics + Software Licensing + WSA SPI + Event Log",
        includes: &["winbio.h", "slpublic.h", "ws2spi.h", "winevt.h"],
    },
    HeaderGroup {
        comment: "KTM / WER / SAPI / Cloud Files / Diagnostic Data",
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
        includes: &["prsht.h", "snmp.h", "wdsclientapi.h"],
    },
    HeaderGroup {
        comment: "WinUSB",
        // wsman.h omitted — requires `WSMAN_API_VERSION_1_0` or `_1_1`
        // be defined before include; not wired up.
        includes: &["winusb.h"],
    },
    HeaderGroup {
        comment: "Virtual disk + Filter Manager user / Catalog / Peer Dist / DRT",
        includes: &["virtdisk.h", "fltuser.h", "mscat.h", "peerdist.h", "drt.h"],
    },
    HeaderGroup {
        comment: "Network providers + EAP + Interaction context + WDS PXE",
        // rtworkq.h omitted — uses C++ syntax (untagged interface names).
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
        includes: &["traceloggingprovider.h", "dsgetdc.h", "perflib.h"],
    },
    HeaderGroup {
        comment: "LM DFS / Pathcch / WinWlx / High-level monitor / Traffic",
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
        includes: &["fci.h", "wintrust.h", "cryptxml.h"],
    },
];
