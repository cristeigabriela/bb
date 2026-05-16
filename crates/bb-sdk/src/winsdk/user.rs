//! User-mode SDK header configuration.

use super::HeaderGroup;
use crate::HeaderConfigKind;

/// `#ifndef`-guarded defines for user-mode.
pub(super) const GUARDED_DEFINES: &[(&str, &str)] = &[
    ("NTDDI_VERSION", "WDK_NTDDI_VERSION"),
    ("_WIN32_WINNT", "_WIN32_WINNT_WIN10"),
    ("WINVER", "_WIN32_WINNT"),
    ("WINAPI_FAMILY", "WINAPI_FAMILY_DESKTOP_APP"),
];

/// Raw (unguarded) defines for user-mode.
///
/// `WIN32_NO_STATUS` tells `winnt.h` to skip its small inline subset
/// of `STATUS_*` macros so the full set can come from `ntstatus.h`
/// without redefinition errors.
///
/// The dedicated "NTSTATUS codes (full set)" HeaderGroup in
/// [`GROUPS`] — placed right after "Core Windows" so later headers
/// like `dbgeng.h` can see the codes it emits — undoes this via
/// `pre_lines: &["#undef WIN32_NO_STATUS"]`.
///
/// The undef is permanent for the rest of the parse; no other SDK
/// header relies on `WIN32_NO_STATUS` being set after winnt.h has
/// finished.
pub(super) const RAW_DEFINES: &[(&str, &str)] = &[("WIN32_NO_STATUS", "")];

/// Grouped `#include` sections for user-mode.
///
/// `winsock2.h` / `ws2tcpip.h` must come **before** `windows.h` — the
/// legacy `windows.h` chain pulls `winsock.h` (Winsock 1) which
/// conflicts with the Winsock 2 declarations.
pub(super) const GROUPS: &[HeaderGroup] = &[
    HeaderGroup::new(
        "Winsock 2 (must precede windows.h)",
        &["winsock2.h", "ws2tcpip.h", "mswsock.h"],
    ),
    HeaderGroup::new("Core Windows", &["windows.h"]),
    // `winternl.h` lives in its own group and skips PHNT builds —
    // phnt.h's preamble errors out with "Do not mix Winternl.h and
    // phnt.h" because its private NT API set is a superset of
    // winternl's redacted public surface.
    HeaderGroup::new("NT internals (user-mode subset)", &["winternl.h"])
        .skip_for(&[HeaderConfigKind::Phnt]),
    // NTSTATUS codes must land *here*, not at the end of GROUPS.
    //
    // `RAW_DEFINES` set `WIN32_NO_STATUS` so winnt.h (pulled by
    // windows.h above) skipped its tiny inline `STATUS_*` defines.
    //
    // Undo the gate now and emit `ntstatus.h`'s full ~2800-code set
    // before any downstream header references symbols that live in
    // ntstatus.h.
    //
    // `dbgeng.h` in particular uses `DBG_COMMAND_EXCEPTION`, which
    // only ntstatus.h provides — without this group at this
    // position, dbgeng.h's parse errors out.
    HeaderGroup::new("NTSTATUS codes (full set)", &["ntstatus.h"])
        .with_pre_lines(&["#undef WIN32_NO_STATUS"]),
    HeaderGroup::new(
        "Networking",
        &["winhttp.h", "wininet.h", "iphlpapi.h", "icmpapi.h"],
    ),
    HeaderGroup::new(
        "Process/Thread/Memory",
        &[
            "tlhelp32.h",
            "psapi.h",
            "processthreadsapi.h",
            "memoryapi.h",
            "jobapi.h",
        ],
    ),
    HeaderGroup::new("File/IO", &["fileapi.h", "ioapiset.h", "namedpipeapi.h"]),
    HeaderGroup::new(
        "Security",
        &[
            "securitybaseapi.h",
            "securityappcontainer.h",
            "aclapi.h",
            "sddl.h",
            "wincrypt.h",
            "bcrypt.h",
            "ncrypt.h",
            "authz.h",
        ],
    ),
    HeaderGroup::new("Shell", &["shlobj.h", "shellapi.h", "shlwapi.h"]),
    HeaderGroup::new("COM/OLE", &["objbase.h", "combaseapi.h", "oleauto.h"]),
    HeaderGroup::new("Debugging", &["dbghelp.h", "errhandlingapi.h"]),
    HeaderGroup::new("Registry", &["winreg.h"]),
    HeaderGroup::new("Services", &["winsvc.h"]),
    HeaderGroup::new("Synchronization", &["synchapi.h"]),
    HeaderGroup::new(
        "System Info",
        &["sysinfoapi.h", "powerbase.h", "appmodel.h"],
    ),
    HeaderGroup::new("Handles", &["handleapi.h"]),
    HeaderGroup::new(
        "User / GDI / common controls",
        &["winuser.h", "commctrl.h", "commdlg.h"],
    ),
    // ntsecapi.h omitted — its LSA_UNICODE_STRING / LSA_STRING typedefs
    // conflict with winternl.h's UNICODE_STRING / STRING.
    HeaderGroup::new("WTS / user-info", &["wtsapi32.h", "lmaccess.h"]),
    HeaderGroup::new(
        "Setup / Cfgmgr / WMI",
        &["setupapi.h", "cfgmgr32.h", "wbemcli.h"],
    ),
    HeaderGroup::new("Media Foundation", &["mfapi.h", "mfidl.h"]),
    HeaderGroup::new(
        "Core Audio (WASAPI / device enumeration / policy)",
        &["mmdeviceapi.h", "audioclient.h", "audiopolicy.h"],
    ),
    HeaderGroup::new(
        "Direct3D (11/12) + DXGI",
        &["dxgi1_6.h", "d3d11.h", "d3d12.h"],
    ),
    HeaderGroup::new("Shell objects (COM)", &["shobjidl_core.h", "shobjidl.h"]),
    HeaderGroup::new("UI Automation", &["uiautomation.h"]),
    HeaderGroup::new("Bluetooth + XInput", &["bluetoothapis.h", "xinput.h"]),
    HeaderGroup::new("ETW / TDH tracing", &["evntrace.h", "evntprov.h", "tdh.h"]),
    HeaderGroup::new(
        "Background transfer (BITS) + Windows Update",
        &["bits.h", "wuapi.h"],
    ),
    HeaderGroup::new("Certificate enrollment", &["certenroll.h"]),
    HeaderGroup::new("DirectShow", &["strmif.h"]),
    // TAPI omitted — tapi3if.h pulls tapi.h, which has C-mode parse errors.
    HeaderGroup::new("Windows Image Acquisition", &["wia.h"]),
    HeaderGroup::new("Windows Media Player", &["wmp.h"]),
    HeaderGroup::new(
        "AzMan / COM+ / XPS / Tablet ink / TextObjectModel",
        &[
            "azroles.h",
            "comsvcs.h",
            "xpsobjectmodel.h",
            "msinkaut.h",
            "tom.h",
        ],
    ),
    HeaderGroup::new("Active Directory", &["iads.h", "adshlp.h"]),
    HeaderGroup::new("Performance counters", &["pdh.h"]),
    HeaderGroup::new("Task Scheduler", &["taskschd.h"]),
    // dwrite.h omitted — uses C++ syntax (`static_cast`, untagged
    // struct references) that won't parse in C mode.
    HeaderGroup::new(
        "Text services framework / performance logs",
        &["msctf.h", "pla.h"],
    ),
    HeaderGroup::new(
        "Property variants + intsafe helpers",
        &["propvarutil.h", "intsafe.h"],
    ),
    // webservices.h omitted — declares two empty enums clang rejects in C mode.
    HeaderGroup::new(
        "LDAP / firewall (netfw, fwpmu)",
        &["winldap.h", "netfw.h", "fwpmu.h"],
    ),
    HeaderGroup::new(
        "Video for Windows / IMAPI / Failover Cluster / Management Infra",
        &["vfw.h", "imapi2.h", "clusapi.h", "mi.h"],
    ),
    HeaderGroup::new(
        "Debugger engine (dbgeng) — user-mode debugging",
        &["dbgeng.h"],
    ),
    // wudfddi.h omitted — lives only in legacy `wdf/umdf/1.x/` trees that
    // aren't on the include path; modern UMDF 2.x uses wdf.h directly.
    HeaderGroup::new(
        "MF read/write + media engine + D3D compiler",
        &["mfreadwrite.h", "mfmediaengine.h", "d3dcompiler.h"],
    ),
    HeaderGroup::new("Property system", &["propsys.h"]),
    HeaderGroup::new("WLAN + RAS", &["wlanapi.h", "ras.h", "rasdlg.h"]),
    HeaderGroup::new("Power + Group Policy", &["powrprof.h", "gpedit.h"]),
    // security.h omitted — its inner `sspi.h` requires one of
    // SECURITY_WIN32 / SECURITY_KERNEL / SECURITY_MAC be defined first.
    // We don't currently expose a place to set that per-mode.
    HeaderGroup::new(
        "DHCP / DNS / MSTCPIP",
        &["dhcpsapi.h", "dhcpcsdk.h", "windns.h", "mstcpip.h"],
    ),
    HeaderGroup::new("MSI (installer)", &["msi.h", "msiquery.h"]),
    // p2p.h omitted — not shipped in modern SDK trees.
    HeaderGroup::new("MPRAPI + RTMv2", &["mprapi.h", "rtmv2.h"]),
    // tapi.h / tspi.h omitted — tapi.h has C-mode parse errors that
    // cascade into anything that pulls it (tspi.h, winfax.h, tapi3if.h).
    HeaderGroup::new("HBA", &["hbaapi.h"]),
    // resapi.h omitted — typedef redefinition of PHANDLER_ROUTINE
    // (PVOID vs DWORD_PTR) against services API.
    // iscsidsc.h omitted — references ISDSC_STATUS without including
    // the header that defines it.
    HeaderGroup::new("NTMS", &["ntmsapi.h"]),
    HeaderGroup::new(
        "Theming + DWM + Direct Composition",
        &["uxtheme.h", "dwmapi.h"],
    ),
    HeaderGroup::new("Strsafe + SafeInt helpers", &["strsafe.h"]),
    // winfax.h omitted — pulls tapi.h transitively.
    HeaderGroup::new(
        "HTTP server / WinSNMP / WinCred / Userenv",
        &["http.h", "winsnmp.h", "wincred.h", "userenv.h"],
    ),
    HeaderGroup::new("ICM (color management) + NTDS", &["icm.h", "ntdsapi.h"]),
    HeaderGroup::new("Common log file system + USP10", &["clfsw32.h", "usp10.h"]),
    // winddi.h omitted — pulls ddrawi.h -> dvp.h whose typedefs collide
    // with the chain; also redefines HSEMAPHORE against the GDI version.
    // rpcproxy.h omitted — uses anonymous struct declarations clang
    // rejects in C mode.
    HeaderGroup::new("DRM", &["msdrm.h"]),
    HeaderGroup::new(
        "Biometrics + Software Licensing + WSA SPI + Event Log",
        &["winbio.h", "slpublic.h", "ws2spi.h", "winevt.h"],
    ),
    HeaderGroup::new(
        "KTM / WER / SAPI / Cloud Files / Diagnostic Data",
        &[
            "ktmw32.h",
            "werapi.h",
            "sapi.h",
            "cfapi.h",
            "diagnosticdataquery.h",
        ],
    ),
    HeaderGroup::new(
        "Property sheets + SNMP + WDS client",
        &["prsht.h", "snmp.h", "wdsclientapi.h"],
    ),
    // wsman.h omitted — requires `WSMAN_API_VERSION_1_0` or `_1_1`
    // be defined before include; not wired up.
    // winusb pulls `shared/usb.h` whose stub `typedef PVOID PIRP`
    // conflicts with phnt's real `typedef struct _IRP *PIRP`.
    HeaderGroup::new("WinUSB", &["winusb.h"]).skip_for(&[HeaderConfigKind::Phnt]),
    HeaderGroup::new(
        "Virtual disk + Filter Manager user / Catalog / Peer Dist / DRT",
        &["virtdisk.h", "fltuser.h", "mscat.h", "peerdist.h", "drt.h"],
    ),
    // rtworkq.h omitted — uses C++ syntax (untagged interface names).
    HeaderGroup::new(
        "Network providers + EAP + Interaction context + WDS PXE",
        &[
            "npapi.h",
            "eapmethodpeerapis.h",
            "interactioncontext.h",
            "wdspxe.h",
        ],
    ),
    // imagehlp.h omitted — its `_LOADED_IMAGE`, `_MODLOAD_DATA` etc.
    // collide with dbghelp.h (which is the modern superset).
    HeaderGroup::new(
        "Trace logging / DS / Perf",
        &["traceloggingprovider.h", "dsgetdc.h", "perflib.h"],
    ),
    HeaderGroup::new(
        "LM DFS / Pathcch / WinWlx / High-level monitor / Traffic",
        &[
            "lmdfs.h",
            "pathcch.h",
            "winwlx.h",
            "highlevelmonitorconfigurationapi.h",
            "traffic.h",
        ],
    ),
    HeaderGroup::new(
        "Magnification / ProjFS / RO error / WDS TP",
        &[
            "magnification.h",
            "projectedfslib.h",
            "roerrorapi.h",
            "wdstpdi.h",
        ],
    ),
    // sphelper.h omitted — heavy C++ speech-API templates that
    // balloon clang parse time from ~9s to >4 minutes.
    HeaderGroup::new(
        "FCI / WinTrust / CryptXML",
        &["fci.h", "wintrust.h", "cryptxml.h"],
    ),
];
