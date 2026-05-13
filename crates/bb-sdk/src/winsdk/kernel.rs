//! Kernel-mode SDK header configuration.

use super::HeaderGroup;

/// `#ifndef`-guarded defines for kernel-mode.
pub(super) const GUARDED_DEFINES: &[(&str, &str)] = &[("NTDDI_VERSION", "WDK_NTDDI_VERSION")];

/// Raw (unguarded) defines for kernel-mode.
///
/// `CDECL` is normally defined by `shared/minwindef.h`, which only enters
/// the include graph through `windows.h` (a user-mode header). Kernel
/// chains don't pull it, so ks.h, ksmedia.h, and other shared headers
/// that use `CDECL` as a calling-convention marker break without this.
pub(super) const RAW_DEFINES: &[(&str, &str)] = &[
    ("_KERNEL_MODE", "1"),
    ("CDECL", "_cdecl"),
    // mmreg.h would emit tagEXBMINFOHEADER (uses BITMAPINFOHEADER from
    // wingdi.h) unless NOBITMAP gates it out. We pull mmreg.h only for
    // its _INC_MMREG marker, not for the GDI structs.
    ("NOBITMAP", ""),
];

/// Grouped `#include` sections for kernel-mode.
///
/// `ntifs.h` is the umbrella: it `#define`s `_NTIFS_` and pulls `ntddk.h`
/// (which pulls `wdm.h`). Including `ntddk.h` before `ntifs.h` causes
/// `PEPROCESS` / `PETHREAD` typedef redefinitions because `ntddk.h`
/// emits forward typedefs that `ntifs.h` later redefines as full types.
///
/// Header set is chosen to cover as many of the documented kernel/driver
/// DDIs from `bb-sparse`'s driver dataset as parse cleanly together.
pub(super) const GROUPS: &[HeaderGroup] = &[
    HeaderGroup {
        comment: "Core kernel headers (ntifs.h umbrella -> ntddk.h -> wdm.h)",
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
        comment: "USB DDK (must come before wdfusb.h: USB_REQUEST_*, PURB, USBD_STATUS)",
        includes: &["usb.h", "usbdi.h"],
    },
    HeaderGroup {
        comment: "WDF / KMDF",
        includes: &["wdf.h", "wdfusb.h"],
    },
    // Storage stack (storport.h, ata.h, classpnp.h) deliberately omitted:
    // shared/ntddstor.h ships without include guards, and the ntifs.h
    // chain has already pulled it in, so a second include via storport.h
    // re-runs all `DEFINE_GUID(GUID_DEVINTERFACE_*, …)` lines and clang
    // reports them as redefinitions.
    HeaderGroup {
        comment: "NDIS networking",
        includes: &["ndis.h"],
    },
    HeaderGroup {
        comment: "Kernel streaming + audio port-class",
        // windef.h shims in BOOL/FLOAT/RECT for ks.h/ksmedia.h.
        // mmreg.h sets _INC_MMREG, which gates KSDATAFORMAT_WAVEFORMATEX
        // and friends in ksmedia.h — portcls.h needs those types.
        includes: &["windef.h", "mmreg.h", "ks.h", "ksmedia.h", "portcls.h"],
    },
    HeaderGroup {
        comment: "HID (hidusage.h provides USAGE for hidpi.h)",
        includes: &["hidusage.h", "hidpi.h", "hidclass.h"],
    },
    HeaderGroup {
        comment: "USB driver lib",
        includes: &["usbdlib.h"],
    },
    HeaderGroup {
        comment: "NetAdapterCx (modern NDIS-on-WDF)",
        includes: &["netadaptercx.h"],
    },
    HeaderGroup {
        comment: "Bus + power + debug extras (small driver DDIs)",
        // wdbgexts.h omitted — clang resolves <wdbgexts.h> from `um/`
        // before `km/` (the -I order), and the user-mode header uses
        // LPTR which isn't defined in our kernel chain.
        includes: &["swenum.h", "pep_x.h", "ndischimney.h"],
    },
    HeaderGroup {
        comment: "HID driver-side",
        includes: &["hidsdi.h"],
    },
    // bthsdpddi.h omitted — its top-level declarations rely on a typedef
    // alias that isn't in scope (`implicit-int` errors at line 19).
    // ucxclass.h / ucmcx.h / ucmucsicx.h / poscx.h are gated behind
    // versioned KMDF-cousin paths (km/ucx/<ver>/, um/pos/<ver>/, …) that
    // we don't discover yet. Their entries stay invisible until we add a
    // generic discovery for those flavors.
    // Deliberately omitted:
    // - udecx.h: not present in all installed WDKs.
    // - d3dkmthk.h / bdasup.h / fwpsk.h: drag in the COM machinery
    //   (wtypes.h), which redefines VARENUM / VT_EMPTY against the
    //   enumerators ks.h already declared for the kernel side.
    // - video.h: references PEMULATOR_ACCESS_ENTRY / PBANKED_SECTION_ROUTINE
    //   (typedef'd in miniport.h) but miniport.h itself redefines _QUAD and
    //   _PROCESSOR_NUMBER against the kernel core types already in scope.
    // - dbghelp.h: minidumpapiset.h needs user-mode-only types.
    // dbghelp.h omitted: pulls minidumpapiset.h which references
    // VS_FIXEDFILEINFO / TIME_ZONE_INFORMATION (user-mode-only types
    // from winver.h / winbase.h). The driver dataset references it
    // for only ~12 entries; the cost-vs-coverage trade-off favors
    // leaving it for the dedicated user-mode path.
];
