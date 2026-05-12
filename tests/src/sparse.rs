//! Unit-style tests for `bb-sparse`. These don't need a clang translation
//! unit; integration tests that parse real Windows headers live in
//! `integration.rs`.
//!
//! Many tests can only assert real invariants when the relevant dataset was
//! embedded at build time. On a clean checkout without submodules (or in
//! environments without uv/Python), bb-sparse embeds an empty placeholder
//! and `is_available_*` returns false — those tests degrade to compile-time
//! + smoke checks rather than failing. CI initializes both submodules, so
//! the data-bearing assertions run there.

#![cfg(test)]

use bb_sparse::Source;

/* ───────────────────────────── Smoke invariants ─────────────────────────── */

#[test]
fn counts_match_combined_count() {
    assert_eq!(
        bb_sparse::entry_count(),
        bb_sparse::entry_count_sdk() + bb_sparse::entry_count_driver(),
    );
}

#[test]
fn availability_flags_consistent_with_counts() {
    assert_eq!(
        bb_sparse::is_available_sdk(),
        bb_sparse::entry_count_sdk() > 0,
    );
    assert_eq!(
        bb_sparse::is_available_driver(),
        bb_sparse::entry_count_driver() > 0,
    );
    assert_eq!(
        bb_sparse::is_available(),
        bb_sparse::is_available_sdk() || bb_sparse::is_available_driver(),
    );
}

#[test]
fn missing_function_returns_none() {
    let name = "__bb_sparse_definitely_not_a_real_function__";
    assert!(bb_sparse::lookup(name).is_none());
    assert!(bb_sparse::lookup_sdk(name).is_none());
    assert!(bb_sparse::lookup_driver(name).is_none());
}

/* ─────────────────────────── SDK content assertions ─────────────────────── */

#[test]
fn sdk_createfilew_has_expected_metadata() {
    if !bb_sparse::is_available_sdk() {
        eprintln!("skipping — SDK dataset not embedded");
        return;
    }
    let m = bb_sparse::lookup_sdk("CreateFileW").expect("CreateFileW missing from SDK dataset");

    assert_eq!(m.source, Some(Source::Sdk));
    assert!(
        m.driver().is_none(),
        "SDK entry should not expose driver metadata"
    );

    // Shared fields — anchored on the well-known MSDN values.
    assert_eq!(m.header_str(), Some("fileapi.h"));
    assert_eq!(m.lib_display().as_deref(), Some("Kernel32.lib"));
    assert!(
        m.dll_display()
            .as_deref()
            .is_some_and(|d| d.contains("Kernel32")),
        "expected Kernel32-family DLL on CreateFileW, got {:?}",
        m.dll_display(),
    );

    // Parameters mirror the documented signature.
    for p in [
        "lpFileName",
        "dwDesiredAccess",
        "dwShareMode",
        "lpSecurityAttributes",
        "dwCreationDisposition",
        "dwFlagsAndAttributes",
        "hTemplateFile",
    ] {
        assert!(
            m.params.contains_key(p),
            "CreateFileW missing parameter {p}"
        );
    }

    // dwShareMode carries the well-known FILE_SHARE_* constant set.
    let share = m
        .params
        .get("dwShareMode")
        .expect("dwShareMode parameter");
    for v in ["FILE_SHARE_READ", "FILE_SHARE_WRITE", "FILE_SHARE_DELETE"] {
        assert!(
            share.values.contains_key(v),
            "CreateFileW dwShareMode missing known constant {v}"
        );
    }

    // ApiMetadata: names contain the A/W/base variants; description present.
    let api = m.metadata.as_ref().expect("CreateFileW has metadata");
    let names = api.names();
    for v in ["CreateFile", "CreateFileA", "CreateFileW"] {
        assert!(
            names.iter().any(|n| n == v),
            "CreateFileW api_name missing {v}, got {names:?}"
        );
    }
    assert!(
        api.description_str()
            .is_some_and(|d| d.contains("file") || d.contains("File")),
        "expected non-trivial description on CreateFileW"
    );
}

/* ────────────────────────── Driver content assertions ──────────────────── */

#[test]
fn driver_entry_exposes_driver_metadata() {
    if !bb_sparse::is_available_driver() {
        eprintln!("skipping — driver dataset not embedded");
        return;
    }
    // GET_VENDOR_ID_FROM_PARAMSET is a small, stable macro entry — useful
    // for asserting the basic driver fields (tech_root, construct_type).
    let m = bb_sparse::lookup_driver("GET_VENDOR_ID_FROM_PARAMSET")
        .expect("GET_VENDOR_ID_FROM_PARAMSET missing from driver dataset");

    assert_eq!(m.source, Some(Source::Driver));
    assert_eq!(m.header_str(), Some("a2dpsidebandaudio.h"));

    let drv = m
        .driver()
        .expect("driver-source entry should expose driver()");
    assert_eq!(drv.tech_root_str(), Some("audio"));
    assert_eq!(drv.construct_type_str(), Some("function"));
}

#[test]
fn driver_entry_has_irql_and_kmdf_when_documented() {
    if !bb_sparse::is_available_driver() {
        eprintln!("skipping — driver dataset not embedded");
        return;
    }
    // MBB_DEVICE_CONFIG_INIT is documented with PASSIVE_LEVEL IRQL, KMDF
    // 1.27, and Universal target-type — a good anchor for the full driver
    // field set.
    let m = bb_sparse::lookup_driver("MBB_DEVICE_CONFIG_INIT")
        .expect("MBB_DEVICE_CONFIG_INIT missing from driver dataset");
    let drv = m.driver().expect("driver metadata expected");

    let irql = drv.irql.as_ref().expect("expected an IRQL constraint");
    assert_eq!(irql.level, "PASSIVE_LEVEL");

    assert_eq!(drv.kmdf_ver_str(), Some("1.27"));
    assert_eq!(drv.target_type_str(), Some("Universal"));
    assert_eq!(drv.tech_root_str(), Some("netvista"));

    let api = m.metadata.as_ref().expect("has API metadata");
    assert!(
        api.description_str()
            .is_some_and(|d| d.contains("MBB_DEVICE_CONFIG")),
        "expected description to mention the struct name"
    );
}

#[test]
fn combined_lookup_prefers_sdk() {
    if !bb_sparse::is_available_sdk() {
        return;
    }
    let Some(m) = bb_sparse::lookup("CreateFileW") else {
        return;
    };
    assert_eq!(m.source, Some(Source::Sdk));
}
