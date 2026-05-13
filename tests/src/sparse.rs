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

use bb_sparse::{Entry, Metadata};

/* ─────────── One-off coverage probe (not part of CI) ──────────────────────
 *
 * Run with:
 *   cargo test -p bb-tests sparse_coverage_dump -- --ignored --nocapture --test-threads=1
 *
 * For each sparse entry whose header is known, ask bb-funcs whether it's
 * visible in the current default bb-sdk umbrella (parsed via the standard
 * config + the macro-preprocessed TU). Aggregate misses by header to show
 * where the next batch of `bb-sdk` header additions would pay off.
 */

#[test]
#[ignore]
fn sparse_coverage_dump() -> anyhow::Result<()> {
    use bb_sdk::{Arch, HeaderConfig, SdkMode};
    use clang::{Clang, EntityKind, Index};
    use std::collections::{BTreeMap, HashSet};

    fn collect_visible(mode: SdkMode) -> HashSet<String> {
        let cfg = HeaderConfig::winsdk(Arch::Amd64, mode).expect("sdk");
        let clang = Clang::new().unwrap();
        let index = Index::new(&clang, false, false);
        let tu = cfg.parse(&index, true).expect("parse");
        let mut names = HashSet::new();
        for e in tu.get_entity().get_children() {
            if matches!(e.get_kind(), EntityKind::FunctionDecl)
                && let Some(n) = e.get_name()
            {
                names.insert(n);
            }
        }
        names
    }

    fn report(
        label: &str,
        visible: &HashSet<String>,
        iter: impl Iterator<Item = (String, String)>,
    ) {
        let mut per_header: BTreeMap<String, (usize, usize, usize)> = BTreeMap::new();
        let mut total_have = 0usize;
        let mut total_miss_method = 0usize;
        let mut total_miss_other = 0usize;
        let mut sample_other: Vec<(String, String)> = Vec::new();
        for (name, header) in iter {
            let key = header
                .split(',')
                .next()
                .unwrap_or(&header)
                .trim()
                .to_ascii_lowercase();
            let entry = per_header.entry(key.clone()).or_insert((0, 0, 0));
            if visible.contains(&name) {
                entry.0 += 1;
                total_have += 1;
            } else if name.contains("::") {
                entry.1 += 1;
                total_miss_method += 1;
            } else {
                entry.2 += 1;
                total_miss_other += 1;
                if sample_other.len() < 40 {
                    sample_other.push((name, key));
                }
            }
        }
        let total = total_have + total_miss_method + total_miss_other;
        println!("===== {label} =====");
        println!("  have:       {total_have}");
        println!("  miss (::):  {total_miss_method}   <- COM interface methods");
        println!("  miss (free):{total_miss_other}   <- actual free-function misses");
        println!(
            "  ratio incl. methods: {:.1}%",
            100.0 * total_have as f64 / total.max(1) as f64
        );
        println!(
            "  ratio free-only:     {:.1}%",
            100.0 * total_have as f64 / (total_have + total_miss_other).max(1) as f64
        );
        let mut rows: Vec<_> = per_header.into_iter().collect();
        rows.sort_by(|a, b| b.1.2.cmp(&a.1.2));
        println!("Top headers by free-function miss:");
        println!(
            "{:<32}{:>8}{:>10}{:>10}",
            "header", "have", "miss(::)", "miss(free)"
        );
        for (h, (have, method_miss, free_miss)) in rows.iter().take(40) {
            if *free_miss == 0 {
                continue;
            }
            println!("{h:<32}{have:>8}{method_miss:>10}{free_miss:>10}");
        }
        println!("Sample of free-function misses (name <- header):");
        for (n, h) in sample_other.iter().take(30) {
            println!("  {n:<40} {h}");
        }
    }

    let visible_user = collect_visible(SdkMode::User);
    report(
        "USER vs sparse SDK",
        &visible_user,
        bb_sparse::iter_sdk()
            .filter_map(|(name, m)| m.header_str().map(|h| (name.to_string(), h.to_string()))),
    );

    let visible_kernel = collect_visible(SdkMode::Kernel);
    report(
        "KERNEL vs sparse driver",
        &visible_kernel,
        bb_sparse::iter_driver().filter_map(|(name, m)| {
            m.include_header_str()
                .map(|h| (name.to_string(), h.to_string()))
        }),
    );

    Ok(())
}

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
            m.params().contains_key(p),
            "CreateFileW missing parameter {p}"
        );
    }

    // dwShareMode carries the well-known FILE_SHARE_* constant set.
    let share = m
        .params()
        .get("dwShareMode")
        .expect("dwShareMode parameter");
    for v in ["FILE_SHARE_READ", "FILE_SHARE_WRITE", "FILE_SHARE_DELETE"] {
        assert!(
            share.values.contains_key(v),
            "CreateFileW dwShareMode missing known constant {v}"
        );
    }

    // ApiMetadata: names contain the A/W/base variants; description present.
    let api = m.api_metadata().expect("CreateFileW has metadata");
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

    assert_eq!(m.header_str(), Some("a2dpsidebandaudio.h"));
    assert_eq!(m.tech_root_str(), Some("audio"));
    assert_eq!(m.construct_type_str(), Some("function"));
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

    let irql = m.irql.as_ref().expect("expected an IRQL constraint");
    assert_eq!(irql.level, "PASSIVE_LEVEL");

    assert_eq!(m.kmdf_ver_str(), Some("1.27"));
    assert_eq!(m.target_type_str(), Some("Universal"));
    assert_eq!(m.tech_root_str(), Some("netvista"));

    let api = m.api_metadata().expect("has API metadata");
    assert!(
        api.description_str()
            .is_some_and(|d| d.contains("MBB_DEVICE_CONFIG")),
        "expected description to mention the struct name"
    );
}

/* ────────────────────────── Entry / lookup precedence ───────────────────── */

#[test]
fn combined_lookup_prefers_sdk() {
    if !bb_sparse::is_available_sdk() {
        return;
    }
    let Some(entry) = bb_sparse::lookup("CreateFileW") else {
        return;
    };
    assert!(matches!(entry, Entry::Sdk(_)));
    // SDK entry has no `driver()`.
    assert!(entry.driver().is_none());
}

#[test]
fn entry_driver_returns_some_for_driver_source() {
    if !bb_sparse::is_available_driver() {
        return;
    }
    // Pick a name that lives in the driver dataset only.
    let Some(entry) = bb_sparse::lookup("MBB_DEVICE_CONFIG_INIT") else {
        return;
    };
    assert!(matches!(entry, Entry::Driver(_)));
    let drv = entry.driver().expect("driver entry exposes .driver()");
    assert!(drv.irql.is_some());
}

/* ─────────────────────────── IrqlConstraint smoke ───────────────────────── */

#[test]
fn irql_constraint_parses_and_matches() {
    use std::collections::HashMap;

    let filter = bb_sparse::irql::parse_constraint("<= DISPATCH_LEVEL").unwrap();
    let lookup = HashMap::from([
        ("PASSIVE_LEVEL".to_string(), 0u64),
        ("DISPATCH_LEVEL".to_string(), 2u64),
        ("HIGH_LEVEL".to_string(), 31u64),
    ]);

    let passive = bb_sparse::IrqlConstraint {
        level: "PASSIVE_LEVEL".into(),
        op: None,
    };
    let high = bb_sparse::IrqlConstraint {
        level: "HIGH_LEVEL".into(),
        op: None,
    };
    assert_eq!(passive.matches(&filter, &lookup), Some(true));
    assert_eq!(high.matches(&filter, &lookup), Some(false));
}
