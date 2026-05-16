#[cfg(test)]
mod tests {
    use serial_test::serial;

    use anyhow::Context;
    use bb_arch::reg::{X64Gpr, X86Gpr};
    use bb_arch::{Arch, MemoryOperand, ParamLocation, Register, ReturnLocation};
    use bb_clang::{
        Enum, Function, RecordKind, Struct, ToJson, TypedefIndex, TypedefKind,
        build_referred_components,
    };
    use bb_consts_lib::{
        ConstFilter, build_lookup_table, collect_constants, collect_enums, filter_constants_by_name,
    };
    use bb_funcs_lib::where_filter::{eval_where, parse_where};
    use bb_funcs_lib::{
        FuncFilter, FuncSort, ParamCountFilter, collect_funcs, collect_funcs_filtered,
    };
    use bb_sdk::{HeaderConfig, PhntVersion, SdkMode};
    use bb_types_lib::{
        StructFilter, collect_structs, collect_unions, find_struct_by_name, find_typedef_hits,
        find_union_by_name, iter_structs, iter_unions,
    };
    use clang::{Clang, Index};

    /// Shorthand macro to get:
    /// 1. Clang isntance
    /// 2. No-diagnostics index
    /// 3. Translation unit
    ///
    /// By default takes 3 arguments and defaults to user-mode AMD64 Windows `SDk`,
    /// but you can substitute those two using the extra arguments.
    macro_rules! winsdk {
        ($clang:ident, $index:ident, $tu:ident) => {
            winsdk!($clang, $index, $tu, Arch::Amd64, SdkMode::User);
        };
        ($clang:ident, $index:ident, $tu:ident, $arch:expr, $mode:expr) => {
            let $clang = Clang::new()
                .map_err(anyhow::Error::msg)
                .context("initializing libclang")?;
            let $index = Index::new(&$clang, false, false);
            let _cfg =
                HeaderConfig::winsdk($arch, $mode).context("creating winsdk header config")?;
            let $tu = _cfg
                .parse(&$index, true)
                .context("parsing winsdk headers")?;
        };
    }

    /// PHNT counterpart of [`winsdk!`]. Defaults to AMD64 / `Win11` /
    /// user-mode unless overridden.
    macro_rules! phnt {
        ($clang:ident, $index:ident, $tu:ident) => {
            phnt!(
                $clang,
                $index,
                $tu,
                Arch::Amd64,
                PhntVersion::Win11,
                SdkMode::User
            );
        };
        ($clang:ident, $index:ident, $tu:ident, $arch:expr, $version:expr, $mode:expr) => {
            let $clang = Clang::new()
                .map_err(anyhow::Error::msg)
                .context("initializing libclang")?;
            let $index = Index::new(&$clang, false, false);
            let _cfg = HeaderConfig::phnt($arch, $version, $mode)
                .context("creating phnt header config")?;
            let $tu = _cfg.parse(&$index, true).context("parsing phnt headers")?;
        };
    }

    /// Find a struct by name.
    fn find_struct<'a, 'b>(structs: &'b [Struct<'a>], name: &str) -> Option<&'b Struct<'a>> {
        structs.iter().find(|s| s.get_name() == name)
    }

    /* ───────────────────────────────── Structs ──────────────────────────────── */

    #[test]
    #[serial]
    fn structs_populated_and_valid() -> anyhow::Result<()> {
        winsdk!(clang, index, tu);

        let structs: Vec<Struct> = iter_structs(&tu)
            .filter_map(|e| Struct::try_from(e).ok())
            .collect();

        assert!(
            structs.len() > 500,
            "expected hundreds of structs, got {}",
            structs.len()
        );

        Ok(())
    }

    #[test]
    #[serial]
    fn guid_layout() -> anyhow::Result<()> {
        winsdk!(clang, index, tu);

        let filter = StructFilter {
            name_pattern: Some("_GUID".into()),
            header_filter: None,
            case_sensitive: true,
        };
        let structs = collect_structs(&tu, &filter, None);
        let guid = structs
            .into_iter()
            .next()
            .ok_or_else(|| anyhow::anyhow!("GUID struct must exist in Windows SDK"))?;

        assert_eq!(guid.get_size(), Some(16), "GUID is always 16 bytes");
        assert_eq!(guid.get_fields().len(), 4, "GUID has exactly 4 fields");

        // Location: should be in guiddef.h
        let location = guid
            .get_location()
            .ok_or_else(|| anyhow::anyhow!("GUID should have a source location"))?;
        assert_eq!(
            location.file.as_ref().map(|x| x.to_lowercase()),
            Some("guiddef.h".into())
        );

        let f = guid.get_fields();

        // Data1: unsigned long (4 bytes) at offset 0
        assert_eq!(f[0].get_name(), "Data1");
        assert_eq!(f[0].get_size(), 4);
        assert_eq!(f[0].get_offset_bytes(), 0);
        assert_eq!(f[0].get_type_name(), Some("unsigned long"));
        assert_eq!(
            f[0].get_semantic_parent(),
            guid.get_entity(),
            "Data1 must be child fo _GUID"
        );

        // Data2: unsigned short (2 bytes) at offset 4
        assert_eq!(f[1].get_name(), "Data2");
        assert_eq!(f[1].get_size(), 2);
        assert_eq!(f[1].get_offset_bytes(), 4);
        assert_eq!(f[1].get_type_name(), Some("unsigned short"));
        assert_eq!(
            f[1].get_semantic_parent(),
            guid.get_entity(),
            "Data2 must be child of _GUID"
        );

        // Data3: unsigned short (2 bytes) at offset 6
        assert_eq!(f[2].get_name(), "Data3");
        assert_eq!(f[2].get_size(), 2);
        assert_eq!(f[2].get_offset_bytes(), 6);
        assert_eq!(f[2].get_type_name(), Some("unsigned short"));
        assert_eq!(
            f[2].get_semantic_parent(),
            guid.get_entity(),
            "Data3 must be child of _GUID"
        );

        // Data4: unsigned char[8] at offset 8
        assert_eq!(f[3].get_name(), "Data4");
        assert_eq!(f[3].get_size(), 8);
        assert_eq!(f[3].get_offset_bytes(), 8);
        assert_eq!(f[3].get_type_name(), Some("unsigned char[8]"));
        assert_eq!(
            f[3].get_semantic_parent(),
            guid.get_entity(),
            "Data4 must be child of _GUID"
        );

        Ok(())
    }

    #[test]
    #[serial]
    fn well_known_structs_exist() -> anyhow::Result<()> {
        winsdk!(clang, index, tu);

        let structs: Vec<Struct> = iter_structs(&tu)
            .filter_map(|e| Struct::try_from(e).ok())
            .collect();

        let expected: &[&str] = &[
            "_GUID",
            "_OVERLAPPED",
            "_SECURITY_ATTRIBUTES",
            "_LIST_ENTRY",
            "_FILETIME",
        ];

        for name in expected {
            assert!(find_struct(&structs, name).is_some(), "{name} not found");
        }

        // FILETIME: always 8 bytes, exactly 2 DWORD fields
        let filetime = find_struct(&structs, "_FILETIME").unwrap();

        // Location: should be in minwindef.h
        let location = filetime
            .get_location()
            .ok_or_else(|| anyhow::anyhow!("FILETIME should have a source location"))?;
        assert_eq!(
            location.file.as_ref().map(|x| x.to_lowercase()),
            Some("minwindef.h".into())
        );

        let f = filetime.get_fields();

        assert_eq!(filetime.get_size(), Some(8), "FILETIME is 8 bytes");
        assert_eq!(f.len(), 2, "FILETIME has 2 fields");

        // dwLowDateTime
        assert_eq!(f[0].get_name(), "dwLowDateTime");
        assert_eq!(f[0].get_size(), 4);
        assert_eq!(f[0].get_offset_bytes(), 0);
        assert_eq!(
            f[0].get_semantic_parent(),
            filetime.get_entity(),
            "dwLowDateTime must be child of FILETIME"
        );

        // dwHighDateTime
        assert_eq!(f[1].get_name(), "dwHighDateTime");
        assert_eq!(f[1].get_size(), 4);
        assert_eq!(f[1].get_offset_bytes(), 4);
        assert_eq!(
            f[1].get_semantic_parent(),
            filetime.get_entity(),
            "dwHighDateTime must be child of FILETIME"
        );

        Ok(())
    }

    #[test]
    #[serial]
    fn struct_filter_and_display() -> anyhow::Result<()> {
        winsdk!(clang, index, tu);

        let filter = StructFilter {
            name_pattern: Some("*OVERLAPPED*".into()),
            header_filter: None,
            case_sensitive: false,
        };
        let filtered = collect_structs(&tu, &filter, None);

        assert!(
            !filtered.is_empty(),
            "should find OVERLAPPED-related structs"
        );
        for s in &filtered {
            assert!(
                s.get_name().to_uppercase().contains("OVERLAPPED"),
                "filtered struct '{}' should match *OVERLAPPED*",
                s.get_name()
            );
        }

        // Verify display() produces meaningful output
        let overlapped =
            find_struct(&filtered, "_OVERLAPPED").expect("_OVERLAPPED must be in filtered results");

        assert!(
            !overlapped.get_fields().is_empty(),
            "OVERLAPPED should have fields"
        );

        let output = overlapped.display(1, None, None);
        assert!(!output.is_empty(), "display() should produce output");

        Ok(())
    }

    #[test]
    #[serial]
    fn struct_nested_type_extraction() -> anyhow::Result<()> {
        winsdk!(clang, index, tu);

        let filter = StructFilter {
            name_pattern: Some("*OVERLAPPED".into()),
            header_filter: None,
            case_sensitive: true,
        };
        let structs = collect_structs(&tu, &filter, None);
        let overlapped = find_struct(&structs, "_OVERLAPPED").expect("_OVERLAPPED must exist");

        // OVERLAPPED has nested anonymous struct/union types
        let (nested_structs, nested_unions) = overlapped.extract_nested_records(2);
        // Just verify the method runs without panicking and returns valid records.
        for n in &nested_structs {
            assert!(!n.get_name().is_empty(), "nested struct should have a name");
        }
        for n in &nested_unions {
            assert!(!n.get_name().is_empty(), "nested union should have a name");
        }

        // referenced_type_names returns names of named child records
        // (both structs and unions); anonymous nested records are
        // omitted because they have no string name.
        for name in overlapped.referenced_type_names() {
            assert!(
                !name.is_empty(),
                "referenced type names should be non-empty"
            );
        }

        Ok(())
    }

    #[test]
    #[serial]
    fn enums_populated_with_constants() -> anyhow::Result<()> {
        winsdk!(clang, index, tu);

        let enums = collect_enums(&tu, &no_filter());

        assert!(!enums.is_empty(), "Windows SDK should contain enums");

        // At least some enums should have constants
        let with_constants: Vec<&Enum> = enums
            .iter()
            .filter(|e| !e.get_constants().is_empty())
            .collect();

        assert!(
            !with_constants.is_empty(),
            "some enums should have constants"
        );

        // Enum constants from EnumConstantDecl are always integers
        for e in &with_constants {
            for c in e.get_constants() {
                assert!(
                    c.is_enum_child(),
                    "'{}::{}' should be an enum constant",
                    e.get_name(),
                    c.get_name()
                );
                assert!(
                    c.get_value().as_u64().is_some(),
                    "enum constant '{}::{}' should have an integer value",
                    e.get_name(),
                    c.get_name()
                );
            }
        }

        // Named (non-anonymous) enums should report their underlying type
        for e in enums.iter().filter(|e| !e.is_anonymous()) {
            assert!(
                e.get_type_name().is_some(),
                "named enum '{}' should report its underlying type",
                e.get_name()
            );
        }

        // Verify display() produces output for an enum with constants
        let output = with_constants[0].display();
        assert!(!output.is_empty(), "enum display() should produce output");

        Ok(())
    }

    #[test]
    #[serial]
    fn enum_filter_by_pattern() -> anyhow::Result<()> {
        winsdk!(clang, index, tu);

        // Collect all enums, then filtered
        let all = collect_enums(&tu, &no_filter());

        let filtered_cfg = ConstFilter {
            enum_pattern: Some("*_NOTIFY_*".into()),
            ..no_filter()
        };
        let filtered = collect_enums(&tu, &filtered_cfg);

        assert!(
            filtered.len() < all.len(),
            "filtered enums ({}) should be fewer than all enums ({})",
            filtered.len(),
            all.len()
        );

        for e in &filtered {
            assert!(
                e.get_name().contains("_NOTIFY_") || e.get_name().contains("_notify_"),
                "filtered enum '{}' should match *_NOTIFY_*",
                e.get_name()
            );
        }

        Ok(())
    }

    /* ──────────────────────────────── Constants ─────────────────────────────── */

    #[test]
    #[serial]
    fn constant_pipeline() -> anyhow::Result<()> {
        winsdk!(clang, index, tu);

        let filter = no_filter();

        let vars = collect_constants(&tu, &filter);
        assert!(
            vars.len() > 100,
            "should find many constants, got {}",
            vars.len()
        );

        // Macro constants (including composed ones) should be present
        assert!(
            vars.iter().any(bb_clang::Constant::is_macro),
            "should find macro constants"
        );

        let enums = collect_enums(&tu, &filter);
        let lookup = build_lookup_table(&enums, &vars);
        assert!(!lookup.is_empty(), "lookup table should have entries");

        Ok(())
    }

    #[test]
    #[serial]
    fn known_macro_constants() -> anyhow::Result<()> {
        winsdk!(clang, index, tu);

        let vars = collect_constants(&tu, &no_filter());

        // MAX_PATH = 260 (minwindef.h)
        let max_path = vars
            .iter()
            .find(|c| c.get_name() == "MAX_PATH")
            .expect("MAX_PATH must exist");
        assert_eq!(max_path.get_value().as_u64(), Some(260));
        assert!(max_path.is_macro(), "MAX_PATH is a #define");
        assert!(
            max_path.get_location().is_some(),
            "MAX_PATH should have a source location"
        );

        // TRUE = 1 (minwindef.h)
        let t = vars
            .iter()
            .find(|c| c.get_name() == "TRUE")
            .expect("TRUE must exist");
        assert_eq!(t.get_value().as_u64(), Some(1));

        // FALSE = 0 (minwindef.h)
        let f = vars
            .iter()
            .find(|c| c.get_name() == "FALSE")
            .expect("FALSE must exist");
        assert_eq!(f.get_value().as_u64(), Some(0));

        Ok(())
    }

    /// NTSTATUS codes (`STATUS_*`) live in `ntstatus.h` (under
    /// `shared/`), which neither `windows.h` (user mode) nor
    /// `ntddk.h` (kernel mode without WDK installed) pulls in.
    ///
    /// Issue #24: a kernel-mode user without WDK installed saw zero
    /// `STATUS_*` codes.
    ///
    /// We now include `ntstatus.h` directly in both modes via the
    /// synthetic-header coda — guarded by `WIN32_NO_STATUS` in user
    /// mode so winnt.h's tiny inline subset doesn't conflict with the
    /// full set ntstatus.h emits.
    #[test]
    #[serial]
    fn status_codes_present_user_mode() -> anyhow::Result<()> {
        winsdk!(clang, index, tu);

        let vars = collect_constants(&tu, &no_filter());

        // STATUS_SUCCESS — the canonical zero status.
        let success = vars
            .iter()
            .find(|c| c.get_name() == "STATUS_SUCCESS")
            .expect("STATUS_SUCCESS must exist in user mode (via ntstatus.h coda)");
        assert_eq!(success.get_value().as_u64(), Some(0));

        // STATUS_INVALID_HANDLE — picks up the NTSTATUS error-code high bits.
        let invalid_handle = vars
            .iter()
            .find(|c| c.get_name() == "STATUS_INVALID_HANDLE")
            .expect("STATUS_INVALID_HANDLE must exist in user mode");
        assert_eq!(invalid_handle.get_value().as_u64(), Some(0xC000_0008));

        // STATUS_OBJECT_NAME_NOT_FOUND — a different range, NT object errors.
        let obj_not_found = vars
            .iter()
            .find(|c| c.get_name() == "STATUS_OBJECT_NAME_NOT_FOUND")
            .expect("STATUS_OBJECT_NAME_NOT_FOUND must exist");
        assert_eq!(obj_not_found.get_value().as_u64(), Some(0xC000_0034));

        // Density check — ntstatus.h ships thousands of STATUS_* codes.
        // A handful means it didn't actually get included.
        let status_count = vars
            .iter()
            .filter(|c| c.get_name().starts_with("STATUS_"))
            .count();
        assert!(
            status_count > 1000,
            "expected thousands of STATUS_* codes from ntstatus.h, got {status_count}"
        );

        Ok(())
    }

    /// Kernel mode: same STATUS_* contract. With WDK installed, ntifs.h
    /// pulls ntstatus.h transitively; without WDK, the kernel-mode coda's
    /// direct `#include <ntstatus.h>` is the only path. Either way the
    /// codes must be present.
    #[test]
    #[serial]
    fn status_codes_present_kernel_mode() -> anyhow::Result<()> {
        winsdk!(clang, index, tu, Arch::Amd64, SdkMode::Kernel);

        let vars = collect_constants(&tu, &no_filter());

        let success = vars
            .iter()
            .find(|c| c.get_name() == "STATUS_SUCCESS")
            .expect("STATUS_SUCCESS must exist in kernel mode");
        assert_eq!(success.get_value().as_u64(), Some(0));

        let invalid_handle = vars
            .iter()
            .find(|c| c.get_name() == "STATUS_INVALID_HANDLE")
            .expect("STATUS_INVALID_HANDLE must exist in kernel mode");
        assert_eq!(invalid_handle.get_value().as_u64(), Some(0xC000_0008));

        let status_count = vars
            .iter()
            .filter(|c| c.get_name().starts_with("STATUS_"))
            .count();
        assert!(
            status_count > 1000,
            "expected thousands of STATUS_* codes in kernel mode, got {status_count}"
        );

        Ok(())
    }

    /// PHNT user-mode: regression test for the half of #24 / the
    /// `cast_len` type-alias-macro fix surfaced during PR review.
    ///
    /// Before the fix only the four `STATUS_SEVERITY_*`
    /// integer-literal macros came through — every
    /// `((NTSTATUS)0x…L)`-shaped code was silently dropped.
    ///
    /// Root cause: `um/powerbase.h` emits a transient
    /// `#define NTSTATUS LONG` gated on `NT_SUCCESS` not yet being
    /// defined, which is the case when `winternl.h` has been stripped
    /// for phnt. That made `cast_len` refuse to strip the
    /// `(NTSTATUS)` cast in `STATUS_*` bodies, so cexpr never
    /// evaluated them.
    ///
    /// The fix taught `cast_len` to recognize type-alias macros
    /// (a macro whose body is itself made of type-shaped tokens).
    #[test]
    #[serial]
    fn status_codes_present_phnt_user_mode() -> anyhow::Result<()> {
        phnt!(clang, index, tu);

        let vars = collect_constants(&tu, &no_filter());

        let success = vars
            .iter()
            .find(|c| c.get_name() == "STATUS_SUCCESS")
            .expect("STATUS_SUCCESS must exist in phnt user mode");
        assert_eq!(success.get_value().as_u64(), Some(0));

        // STATUS_INVALID_HANDLE uses the NTSTATUS cast — this is the
        // exact shape that pre-fix used to vanish.
        let invalid_handle = vars
            .iter()
            .find(|c| c.get_name() == "STATUS_INVALID_HANDLE")
            .expect(
                "STATUS_INVALID_HANDLE must resolve in phnt user mode \
                 (regression: cast_len had to recognize `#define NTSTATUS \
                 LONG` from powerbase.h as a type-alias macro to strip \
                 the `(NTSTATUS)` cast)",
            );
        assert_eq!(invalid_handle.get_value().as_u64(), Some(0xC000_0008));

        let status_count = vars
            .iter()
            .filter(|c| c.get_name().starts_with("STATUS_"))
            .count();
        assert!(
            status_count > 1000,
            "expected thousands of STATUS_* codes in phnt user mode, got \
             {status_count} (regression — was 4 before the cast_len fix)"
        );

        Ok(())
    }

    #[test]
    #[serial]
    fn constant_name_filter() -> anyhow::Result<()> {
        winsdk!(clang, index, tu);

        let filter = ConstFilter {
            const_pattern: Some("MAX_*".into()),
            ..no_filter()
        };

        let vars = collect_constants(&tu, &filter);
        let filtered = filter_constants_by_name(vars, &filter);
        assert!(!filtered.is_empty(), "should find MAX_* constants");

        for c in &filtered {
            assert!(
                c.get_name().starts_with("MAX_"),
                "constant '{}' should match MAX_*",
                c.get_name()
            );
        }

        // MAX_PATH should survive filtering
        assert!(
            filtered.iter().any(|c| c.get_name() == "MAX_PATH"),
            "MAX_PATH should be in filtered results"
        );

        Ok(())
    }

    #[test]
    #[serial]
    fn scoped_to_enum_skips_constants() -> anyhow::Result<()> {
        winsdk!(clang, index, tu);

        let filter = ConstFilter {
            scoped_to_enum: true,
            ..no_filter()
        };

        let vars = collect_constants(&tu, &filter);
        assert!(
            vars.is_empty(),
            "scoped_to_enum should skip constant collection"
        );

        Ok(())
    }

    #[test]
    #[serial]
    fn macro_composition_traceable() -> anyhow::Result<()> {
        winsdk!(clang, index, tu);

        let filter = no_filter();
        let vars = collect_constants(&tu, &filter);
        let enums = collect_enums(&tu, &filter);
        let lookup = build_lookup_table(&enums, &vars);

        // There should be at least some composed macro constants in the SDK
        let composed: Vec<_> = vars
            .iter()
            .filter(|c| !c.get_components().is_empty())
            .collect();

        assert!(
            !composed.is_empty(),
            "Windows SDK should contain macro constants composed of other constants"
        );

        // Every composed constant's components must be resolvable in the lookup
        for c in &composed {
            for name in c.get_components() {
                assert!(
                    lookup.contains_key(name.as_str()),
                    "component '{}' of '{}' must exist in the lookup table",
                    name,
                    c.get_name()
                );
                let val = &lookup[name.as_str()];
                assert!(
                    val.as_u64().is_some(),
                    "component '{}' of '{}' must have a numeric value",
                    name,
                    c.get_name()
                );
            }
        }

        // Spot-check: FILE_ALL_ACCESS is composed of STANDARD_RIGHTS_REQUIRED and SYNCHRONIZE
        if let Some(faa) = vars.iter().find(|c| c.get_name() == "FILE_ALL_ACCESS") {
            let components = faa.get_components();
            assert!(
                components.iter().any(|n| n == "STANDARD_RIGHTS_REQUIRED"),
                "FILE_ALL_ACCESS should reference STANDARD_RIGHTS_REQUIRED, got: {components:?}"
            );
            assert!(
                components.iter().any(|n| n == "SYNCHRONIZE"),
                "FILE_ALL_ACCESS should reference SYNCHRONIZE, got: {components:?}"
            );
        }

        Ok(())
    }

    /* ────────────────────────── Architecture details ────────────────────────── */

    #[test]
    #[serial]
    fn pointer_sizes_differ_by_arch() -> anyhow::Result<()> {
        let filter = StructFilter {
            name_pattern: Some("*LIST_ENTRY".into()),
            header_filter: None,
            case_sensitive: true,
        };

        // Parse x86 in its own scope so Clang is dropped before the next one
        let size_x86 = {
            winsdk!(clang, index, tu, Arch::X86, SdkMode::User);

            let structs = collect_structs(&tu, &filter, None);
            find_struct(&structs, "_LIST_ENTRY")
                .expect("LIST_ENTRY must exist on x86")
                .get_size()
                .expect("LIST_ENTRY must have a size")
        };

        let size_amd64 = {
            winsdk!(clang, index, tu, Arch::Amd64, SdkMode::User);

            let structs = collect_structs(&tu, &filter, None);
            find_struct(&structs, "_LIST_ENTRY")
                .expect("LIST_ENTRY must exist on amd64")
                .get_size()
                .expect("LIST_ENTRY must have a size")
        };

        // LIST_ENTRY is two pointers (Flink + Blink)
        assert_eq!(size_x86, 8, "LIST_ENTRY on x86 = 2 x 4-byte pointers");
        assert_eq!(size_amd64, 16, "LIST_ENTRY on amd64 = 2 x 8-byte pointers");

        Ok(())
    }

    /* ────────────────────────────────── JSON ────────────────────────────────── */

    #[test]
    #[serial]
    fn constant_json_structure() -> anyhow::Result<()> {
        winsdk!(clang, index, tu);

        let vars = collect_constants(&tu, &no_filter());

        // Single constant
        let max_path = vars
            .iter()
            .find(|c| c.get_name() == "MAX_PATH")
            .expect("MAX_PATH must exist");
        let j = max_path.to_json();
        assert_eq!(j["name"], "MAX_PATH");
        assert!(j["value"].is_u64(), "MAX_PATH value should be u64");
        assert!(
            j["location"]["file"].is_string(),
            "location.file should be a string"
        );

        // Slice
        let arr = vars.to_json();
        assert!(arr.is_array(), "slice to_json should produce an array");
        assert!(arr.as_array().unwrap().len() > 100);

        Ok(())
    }

    #[test]
    #[serial]
    fn struct_json_has_referenced_types() -> anyhow::Result<()> {
        winsdk!(clang, index, tu);

        let filter = StructFilter {
            name_pattern: Some("_GUID".into()),
            header_filter: None,
            case_sensitive: true,
        };
        let structs = collect_structs(&tu, &filter, None);
        let guid = structs
            .into_iter()
            .next()
            .ok_or_else(|| anyhow::anyhow!("GUID must exist"))?;

        let j = guid.to_json();
        assert_eq!(j["name"], "_GUID");
        assert_eq!(j["kind"], "struct");
        assert!(j["fields"].is_array(), "should have fields array");
        assert!(
            j["referenced_types"].is_array(),
            "to_json should include referenced_types name list"
        );

        Ok(())
    }

    #[test]
    #[serial]
    fn struct_slice_to_json_full() -> anyhow::Result<()> {
        winsdk!(clang, index, tu, Arch::Amd64, SdkMode::User);

        let filter = StructFilter {
            name_pattern: Some("_PEB".into()),
            header_filter: None,
            case_sensitive: true,
        };
        let structs = collect_structs(&tu, &filter, None);
        assert!(!structs.is_empty(), "_PEB must exist");

        let full = structs.to_json_full();
        assert!(full["types"].is_array(), "should have types array");
        assert!(
            full["referenced_types"].is_array(),
            "should have referenced_types array"
        );

        // types entries should NOT have a referenced_types field — that
        // lives at the top level when serialized via the slice path.
        let types = full["types"].as_array().unwrap();
        for t in types {
            assert!(
                t.get("referenced_types").is_none(),
                "types entries should not have referenced_types, got: {}",
                t["name"]
            );
        }

        // _PEB has many nested struct + union types.
        let referenced = full["referenced_types"].as_array().unwrap();
        assert!(
            !referenced.is_empty(),
            "_PEB should reference at least one nested record"
        );

        Ok(())
    }

    /* ────────────────────────────── Type helpers ────────────────────────────── */

    #[test]
    #[serial]
    fn full_path_available() -> anyhow::Result<()> {
        winsdk!(clang, index, tu);

        let filter = StructFilter {
            name_pattern: Some("_GUID".into()),
            header_filter: None,
            case_sensitive: true,
        };
        let structs = collect_structs(&tu, &filter, None);
        let guid = structs
            .into_iter()
            .next()
            .ok_or_else(|| anyhow::anyhow!("GUID must exist"))?;

        let loc = guid
            .get_location()
            .ok_or_else(|| anyhow::anyhow!("GUID should have a location"))?;

        // file should be just the filename
        assert_eq!(
            loc.file.as_deref().map(str::to_lowercase),
            Some("guiddef.h".into())
        );

        // path() should return the full path ending in that filename
        let full = loc.path().expect("full path should be available");
        assert!(
            full.ends_with("guiddef.h"),
            "full path should end with guiddef.h, got: {}",
            full.display()
        );
        assert!(
            full.is_absolute(),
            "full path should be absolute, got: {}",
            full.display()
        );

        Ok(())
    }

    #[test]
    #[serial]
    fn constant_scalar_to_json_full() -> anyhow::Result<()> {
        winsdk!(clang, index, tu);

        let vars = collect_constants(&tu, &no_filter());

        // FILE_ALL_ACCESS is composed of STANDARD_RIGHTS_REQUIRED | SYNCHRONIZE | ...
        let faa = match vars.iter().find(|c| c.get_name() == "FILE_ALL_ACCESS") {
            Some(c) => c,
            None => return Ok(()), // not all SDK configs expose this — skip
        };

        assert!(
            !faa.get_components().is_empty(),
            "FILE_ALL_ACCESS should have component constants"
        );

        let j = faa.to_json_full();

        // Flat JSON — no "constant" wrapper
        assert_eq!(j["name"], "FILE_ALL_ACCESS");
        assert!(j["value"].is_number(), "value should be a number");

        // referred_components must be present and non-empty
        let referred = j["referred_components"]
            .as_array()
            .expect("to_json_full must include referred_components array");
        assert!(
            !referred.is_empty(),
            "FILE_ALL_ACCESS should have non-empty referred_components"
        );

        // Each referred component has a name field
        for comp in referred {
            assert!(
                comp["name"].is_string(),
                "referred component must have a name"
            );
        }

        // Every name listed in components[] must appear in referred_components
        let referred_names: Vec<&str> =
            referred.iter().filter_map(|c| c["name"].as_str()).collect();
        for name in faa.get_components() {
            assert!(
                referred_names.contains(&name.as_str()),
                "component '{name}' should appear in referred_components"
            );
        }

        Ok(())
    }

    #[test]
    #[serial]
    fn constant_slice_to_json_full() -> anyhow::Result<()> {
        winsdk!(clang, index, tu);

        let filter = ConstFilter {
            const_pattern: Some("FILE_*".into()),
            ..no_filter()
        };
        let vars = collect_constants(&tu, &filter);
        assert!(!vars.is_empty(), "should find FILE_* constants");

        let full = vars.to_json_full();

        // Top-level shape
        let constants = full["constants"]
            .as_array()
            .expect("should have constants array");
        let referred = full["referred_components"]
            .as_array()
            .expect("should have referred_components array");

        // Every constant entry has a name
        for c in constants {
            assert!(c["name"].is_string(), "each constant must have a name");
        }

        // referred_components must not duplicate names already in constants
        let constant_names: std::collections::HashSet<&str> = constants
            .iter()
            .filter_map(|c| c["name"].as_str())
            .collect();
        for r in referred {
            let name = r["name"].as_str().unwrap_or("");
            assert!(
                !constant_names.contains(name),
                "referred_components should not contain '{name}' which is already in constants"
            );
        }

        // referred_components must not contain duplicate names
        let mut seen = std::collections::HashSet::new();
        for r in referred {
            let name = r["name"].as_str().unwrap_or("");
            assert!(
                seen.insert(name),
                "referred_components has duplicate '{name}'"
            );
        }

        Ok(())
    }

    #[test]
    #[serial]
    fn build_referred_components_is_empty_when_no_components() -> anyhow::Result<()> {
        winsdk!(clang, index, tu);

        let vars = collect_constants(&tu, &no_filter());

        // MAX_PATH is a simple numeric literal — no component constants
        let max_path = vars
            .iter()
            .find(|c| c.get_name() == "MAX_PATH")
            .expect("MAX_PATH must exist");

        assert!(
            max_path.get_components().is_empty(),
            "MAX_PATH should have no components"
        );

        let referred = build_referred_components(
            std::iter::once(max_path.get_name().to_string()),
            std::slice::from_ref(max_path).iter(),
        );
        assert!(
            referred.is_empty(),
            "build_referred_components for MAX_PATH should be empty"
        );

        let j = max_path.to_json_full();
        assert_eq!(
            j["referred_components"].as_array().map(std::vec::Vec::len),
            Some(0),
            "to_json_full on a simple constant should have empty referred_components"
        );

        Ok(())
    }

    /* ──────────────────────────────── Functions ───────────────────────────── */

    /// Find a function by name.
    fn find_func<'a, 'b>(funcs: &'b [Function<'a>], name: &str) -> Option<&'b Function<'a>> {
        funcs.iter().find(|f| f.get_name() == name)
    }

    #[test]
    #[serial]
    fn funcs_populated_and_valid() -> anyhow::Result<()> {
        winsdk!(clang, index, tu);

        let funcs = collect_funcs(&tu);

        assert!(
            funcs.len() > 100,
            "expected hundreds of functions, got {}",
            funcs.len()
        );

        // Every function should have a name and calling convention.
        for f in &funcs {
            assert!(!f.get_name().is_empty(), "function should have a name");
            assert!(
                !f.get_return_type_name().is_empty(),
                "function '{}' should have a return type name",
                f.get_name()
            );
        }

        Ok(())
    }

    #[test]
    #[serial]
    fn well_known_functions_exist() -> anyhow::Result<()> {
        winsdk!(clang, index, tu);

        let funcs = collect_funcs(&tu);

        let expected: &[&str] = &[
            "CreateFileW",
            "CloseHandle",
            "ReadFile",
            "WriteFile",
            "GetLastError",
        ];

        for name in expected {
            assert!(
                find_func(&funcs, name).is_some(),
                "{name} not found in parsed functions"
            );
        }

        Ok(())
    }

    #[test]
    #[serial]
    fn function_arch_detection_amd64() -> anyhow::Result<()> {
        winsdk!(clang, index, tu, Arch::Amd64, SdkMode::User);

        let funcs = collect_funcs(&tu);
        let f = find_func(&funcs, "CloseHandle").expect("CloseHandle must exist");

        assert_eq!(f.get_arch(), Arch::Amd64);

        Ok(())
    }

    #[test]
    #[serial]
    fn function_arch_detection_x86() -> anyhow::Result<()> {
        winsdk!(clang, index, tu, Arch::X86, SdkMode::User);

        let funcs = collect_funcs(&tu);
        let f = find_func(&funcs, "CloseHandle").expect("CloseHandle must exist");

        assert_eq!(f.get_arch(), Arch::X86);

        Ok(())
    }

    #[test]
    #[serial]
    fn closehandle_x64_param_in_rcx() -> anyhow::Result<()> {
        winsdk!(clang, index, tu, Arch::Amd64, SdkMode::User);

        let funcs = collect_funcs(&tu);
        let f = find_func(&funcs, "CloseHandle").expect("CloseHandle must exist");

        // CloseHandle(HANDLE hObject) — 1 param, integer, in RCX on x64.
        let params = f.get_params();
        assert_eq!(params.len(), 1, "CloseHandle has exactly 1 parameter");

        let p = &params[0];
        assert_eq!(p.get_name(), Some("hObject"));

        assert_eq!(
            *p.get_abi_location(),
            ParamLocation::Direct {
                locations: vec![MemoryOperand::Reg(Register::X64Gpr(X64Gpr::Rcx))],
                size: 8,
            },
            "CloseHandle's HANDLE param should be in RCX on x64"
        );

        // Return: BOOL → RAX
        assert_eq!(
            *f.get_return_location(),
            ReturnLocation::Register(Register::X64Gpr(X64Gpr::Rax)),
        );

        Ok(())
    }

    #[test]
    #[serial]
    fn createfilew_x64_params() -> anyhow::Result<()> {
        winsdk!(clang, index, tu, Arch::Amd64, SdkMode::User);

        let funcs = collect_funcs(&tu);
        let f = find_func(&funcs, "CreateFileW").expect("CreateFileW must exist");

        // CreateFileW has 7 params (callee-entry RSP):
        //   LPCWSTR lpFileName          → RCX          (pos 0)
        //   DWORD dwDesiredAccess       → RDX          (pos 1)
        //   DWORD dwShareMode           → R8           (pos 2)
        //   LPSECURITY_ATTRIBUTES ...   → R9           (pos 3)
        //   DWORD dwCreationDisposition → [RSP+0x28]   (pos 4)
        //   DWORD dwFlagsAndAttributes  → [RSP+0x30]   (pos 5)
        //   HANDLE hTemplateFile        → [RSP+0x38]   (pos 6)
        let params = f.get_params();
        assert_eq!(params.len(), 7, "CreateFileW has 7 parameters");

        // First 4 in registers.
        let expected_regs = [X64Gpr::Rcx, X64Gpr::Rdx, X64Gpr::R8, X64Gpr::R9];
        for (i, expected_reg) in expected_regs.iter().enumerate() {
            match &params[i].get_abi_location() {
                ParamLocation::Direct { locations, .. } => {
                    assert_eq!(
                        locations[0],
                        MemoryOperand::Reg(Register::X64Gpr(*expected_reg)),
                        "param {i} should be in {expected_reg:?}"
                    );
                }
                other => panic!("param {i} expected Direct, got {other:?}"),
            }
        }

        // Params 4–6 on stack (callee-entry RSP).
        // 0x08 return addr + 0x20 shadow = 0x28 base, then +8 per slot.
        let rsp = Register::X64Gpr(X64Gpr::Rsp);
        for (i, rsp_off) in [(4, 0x28_i64), (5, 0x30), (6, 0x38)] {
            match &params[i].get_abi_location() {
                ParamLocation::Direct { locations, .. } => {
                    assert_eq!(
                        locations[0],
                        MemoryOperand::RegImm {
                            base: rsp,
                            offset: rsp_off
                        },
                        "param {i} should be at [RSP+{rsp_off:#x}]"
                    );
                }
                other => panic!("param {i} expected Direct stack, got {other:?}"),
            }
        }

        Ok(())
    }

    #[test]
    #[serial]
    fn closehandle_x86_param_on_stack() -> anyhow::Result<()> {
        winsdk!(clang, index, tu, Arch::X86, SdkMode::User);

        let funcs = collect_funcs(&tu);
        let f = find_func(&funcs, "CloseHandle").expect("CloseHandle must exist");

        let params = f.get_params();
        assert_eq!(params.len(), 1);

        // On x86 stdcall, all params on stack (callee-entry ESP).
        // HANDLE at [ESP+0x04] (after return address).
        let esp = Register::X86Gpr(X86Gpr::Esp);
        match params[0].get_abi_location() {
            ParamLocation::Direct { locations, size } => {
                assert_eq!(
                    locations[0],
                    MemoryOperand::RegImm {
                        base: esp,
                        offset: 0x04,
                    },
                );
                assert_eq!(*size, 4, "HANDLE on x86 is 4 bytes");
            }
            other => panic!("expected Direct stack, got {other:?}"),
        }

        Ok(())
    }

    #[test]
    #[serial]
    fn getlasterror_void_params_rax_return() -> anyhow::Result<()> {
        winsdk!(clang, index, tu, Arch::Amd64, SdkMode::User);

        let funcs = collect_funcs(&tu);
        let f = find_func(&funcs, "GetLastError").expect("GetLastError must exist");

        // GetLastError takes no parameters.
        assert!(f.get_params().is_empty(), "GetLastError has no parameters");

        // Returns DWORD in RAX.
        assert_eq!(
            *f.get_return_location(),
            ReturnLocation::Register(Register::X64Gpr(X64Gpr::Rax)),
        );

        Ok(())
    }

    #[test]
    #[serial]
    fn function_json_structure() -> anyhow::Result<()> {
        winsdk!(clang, index, tu, Arch::Amd64, SdkMode::User);

        let funcs = collect_funcs(&tu);
        let f = find_func(&funcs, "CloseHandle").expect("CloseHandle must exist");

        let j = f.to_json();
        assert_eq!(j["name"], "CloseHandle");
        assert!(j["params"].is_array(), "should have params array");
        assert!(
            j["calling_convention"].is_string(),
            "should have calling_convention"
        );
        assert!(
            j["return_location"].is_object() || j["return_location"].is_string(),
            "should have return_location"
        );
        assert!(j["arch"].is_string(), "should have arch");

        // params[0] should have abi_location
        let p0 = &j["params"][0];
        assert!(
            p0["abi_location"].is_object(),
            "param should have abi_location"
        );

        // Slice
        let arr = funcs.to_json();
        assert!(arr.is_array(), "slice to_json should produce an array");
        assert!(arr.as_array().unwrap().len() > 100);

        Ok(())
    }

    /* ────────────────────────── Function filtering ────────────────────────── */

    fn base_func_filter() -> FuncFilter {
        FuncFilter {
            name_pattern: None,
            header_filter: Some("handleapi.h".into()),
            case_sensitive: true,
            dllimport_only: false,
            param_count: None,
            param_type_pattern: None,
            return_type_pattern: None,
            has_body: None,
            sort: None,
            sort_dir: bb_funcs_lib::SortDir::Asc,
            where_clause: None,
            irql_filter: None,
        }
    }

    #[test]
    #[serial]
    fn filter_by_param_count_exact() -> anyhow::Result<()> {
        winsdk!(clang, index, tu);

        let filter = FuncFilter {
            param_count: Some(ParamCountFilter::Exact(1)),
            ..base_func_filter()
        };
        let funcs = collect_funcs_filtered(&tu, &filter).map_err(anyhow::Error::msg)?;

        assert!(
            !funcs.is_empty(),
            "should find 1-param functions in handleapi.h"
        );
        for f in &funcs {
            assert_eq!(
                f.get_params().len(),
                1,
                "'{}' should have exactly 1 param",
                f.get_name()
            );
        }
        assert!(
            find_func(&funcs, "CloseHandle").is_some(),
            "CloseHandle has 1 param"
        );

        Ok(())
    }

    #[test]
    #[serial]
    fn filter_by_param_count_range() -> anyhow::Result<()> {
        winsdk!(clang, index, tu);

        let filter = FuncFilter {
            param_count: Some(ParamCountFilter::Range { min: 5, max: None }),
            ..base_func_filter()
        };
        let funcs = collect_funcs_filtered(&tu, &filter).map_err(anyhow::Error::msg)?;

        for f in &funcs {
            assert!(
                f.get_params().len() >= 5,
                "'{}' has {} params, expected >= 5",
                f.get_name(),
                f.get_params().len()
            );
        }
        assert!(
            find_func(&funcs, "DuplicateHandle").is_some(),
            "DuplicateHandle has 7 params"
        );

        Ok(())
    }

    #[test]
    #[serial]
    fn filter_by_param_type_positional() -> anyhow::Result<()> {
        winsdk!(clang, index, tu);

        // First param must be HANDLE
        let filter = FuncFilter {
            param_type_pattern: Some("HANDLE".into()),
            ..base_func_filter()
        };
        let funcs = collect_funcs_filtered(&tu, &filter).map_err(anyhow::Error::msg)?;

        assert!(
            !funcs.is_empty(),
            "should find functions with HANDLE as first param"
        );
        for f in &funcs {
            assert_eq!(
                f.get_params()[0].get_type_name(),
                "HANDLE",
                "'{}' first param should be HANDLE",
                f.get_name()
            );
        }

        Ok(())
    }

    #[test]
    #[serial]
    fn filter_by_param_type_positional_skip() -> anyhow::Result<()> {
        winsdk!(clang, index, tu);

        // 4th param (index 3) must be LPHANDLE, using explicit _ slots.
        // Trailing ... because DuplicateHandle has 7 params.
        let filter = FuncFilter {
            param_type_pattern: Some("_,_,_,LPHANDLE,...".into()),
            ..base_func_filter()
        };
        let funcs = collect_funcs_filtered(&tu, &filter).map_err(anyhow::Error::msg)?;

        assert_eq!(
            funcs.len(),
            1,
            "only DuplicateHandle has LPHANDLE at position 3"
        );
        assert_eq!(funcs[0].get_name(), "DuplicateHandle");

        Ok(())
    }

    #[test]
    #[serial]
    fn filter_by_param_type_floating() -> anyhow::Result<()> {
        winsdk!(clang, index, tu);

        // ...,LPHANDLE → LPHANDLE at any position.
        let filter = FuncFilter {
            param_type_pattern: Some("...,LPHANDLE,...".into()),
            ..base_func_filter()
        };
        let funcs = collect_funcs_filtered(&tu, &filter).map_err(anyhow::Error::msg)?;

        assert!(
            find_func(&funcs, "DuplicateHandle").is_some(),
            "DuplicateHandle has LPHANDLE at position 3"
        );
        for f in &funcs {
            assert!(
                f.get_params()
                    .iter()
                    .any(|p| p.get_type_name() == "LPHANDLE"),
                "'{}' should have an LPHANDLE param",
                f.get_name()
            );
        }

        Ok(())
    }

    #[test]
    #[serial]
    fn filter_by_param_type_floating_pair() -> anyhow::Result<()> {
        winsdk!(clang, index, tu);

        // ...,HANDLE,HANDLE,... → consecutive HANDLE pair at any position.
        let filter = FuncFilter {
            param_type_pattern: Some("...,HANDLE,HANDLE,...".into()),
            ..base_func_filter()
        };
        let funcs = collect_funcs_filtered(&tu, &filter).map_err(anyhow::Error::msg)?;

        assert!(
            find_func(&funcs, "DuplicateHandle").is_some(),
            "DuplicateHandle has HANDLE,HANDLE,HANDLE at positions 0-2"
        );
        assert!(
            find_func(&funcs, "CompareObjectHandles").is_some(),
            "CompareObjectHandles has HANDLE,HANDLE at positions 0-1"
        );
        assert!(
            find_func(&funcs, "CloseHandle").is_none(),
            "CloseHandle has only 1 param, can't match HANDLE,HANDLE"
        );

        Ok(())
    }

    #[test]
    #[serial]
    fn filter_by_param_type_open_tail() -> anyhow::Result<()> {
        winsdk!(clang, index, tu);

        // HANDLE,... → HANDLE at position 0, any trailing params OK.
        let filter = FuncFilter {
            param_type_pattern: Some("HANDLE,...".into()),
            ..base_func_filter()
        };
        let funcs = collect_funcs_filtered(&tu, &filter).map_err(anyhow::Error::msg)?;

        assert!(
            find_func(&funcs, "CloseHandle").is_some(),
            "CloseHandle(HANDLE) matches — open tail allows 0 trailing"
        );
        assert!(
            find_func(&funcs, "DuplicateHandle").is_some(),
            "DuplicateHandle(HANDLE, ...) matches"
        );

        Ok(())
    }

    #[test]
    #[serial]
    fn filter_by_param_type_middle_gap() -> anyhow::Result<()> {
        winsdk!(clang, index, tu);

        // HANDLE,...,DWORD,... → HANDLE at 0, then DWORD at some later position.
        let filter = FuncFilter {
            param_type_pattern: Some("HANDLE,...,DWORD,...".into()),
            ..base_func_filter()
        };
        let funcs = collect_funcs_filtered(&tu, &filter).map_err(anyhow::Error::msg)?;

        assert!(
            find_func(&funcs, "SetHandleInformation").is_some(),
            "SetHandleInformation(HANDLE, DWORD, DWORD) matches"
        );
        assert!(
            find_func(&funcs, "DuplicateHandle").is_some(),
            "DuplicateHandle(HANDLE, ..., DWORD, ...) matches"
        );
        assert!(
            find_func(&funcs, "CompareObjectHandles").is_none(),
            "CompareObjectHandles(HANDLE, HANDLE) has no DWORD"
        );

        Ok(())
    }

    #[test]
    #[serial]
    fn filter_by_return_type() -> anyhow::Result<()> {
        winsdk!(clang, index, tu);

        let filter = FuncFilter {
            return_type_pattern: Some("BOOL".into()),
            ..base_func_filter()
        };
        let funcs = collect_funcs_filtered(&tu, &filter).map_err(anyhow::Error::msg)?;

        assert!(!funcs.is_empty(), "should find BOOL-returning functions");
        for f in &funcs {
            assert_eq!(
                f.get_return_type_name(),
                "BOOL",
                "'{}' should return BOOL",
                f.get_name()
            );
        }

        Ok(())
    }

    #[test]
    #[serial]
    fn filter_by_exported() -> anyhow::Result<()> {
        winsdk!(clang, index, tu);

        let filter = FuncFilter {
            dllimport_only: true,
            ..base_func_filter()
        };
        let funcs = collect_funcs_filtered(&tu, &filter).map_err(anyhow::Error::msg)?;

        for f in &funcs {
            assert!(f.is_dllimport(), "'{}' should be dllimport", f.get_name());
        }

        Ok(())
    }

    #[test]
    #[serial]
    fn sort_by_params() -> anyhow::Result<()> {
        winsdk!(clang, index, tu);

        let filter = FuncFilter {
            sort: Some(FuncSort::Params),
            dllimport_only: true,
            ..base_func_filter()
        };
        let funcs = collect_funcs_filtered(&tu, &filter).map_err(anyhow::Error::msg)?;

        assert!(funcs.len() > 1, "need multiple functions to verify sort");
        for w in funcs.windows(2) {
            assert!(
                w[0].get_params().len() <= w[1].get_params().len(),
                "'{}' ({} params) should come before '{}' ({} params)",
                w[0].get_name(),
                w[0].get_params().len(),
                w[1].get_name(),
                w[1].get_params().len(),
            );
        }

        Ok(())
    }

    #[test]
    #[serial]
    fn sort_by_name() -> anyhow::Result<()> {
        winsdk!(clang, index, tu);

        let filter = FuncFilter {
            sort: Some(FuncSort::Name),
            dllimport_only: true,
            ..base_func_filter()
        };
        let funcs = collect_funcs_filtered(&tu, &filter).map_err(anyhow::Error::msg)?;

        assert!(funcs.len() > 1, "need multiple functions to verify sort");
        for w in funcs.windows(2) {
            assert!(
                w[0].get_name() <= w[1].get_name(),
                "'{}' should come before '{}'",
                w[0].get_name(),
                w[1].get_name(),
            );
        }

        Ok(())
    }

    #[test]
    #[serial]
    fn filter_combined_param_count_and_return() -> anyhow::Result<()> {
        winsdk!(clang, index, tu);

        // BOOL-returning functions with exactly 2 params in handleapi.h
        let filter = FuncFilter {
            param_count: Some(ParamCountFilter::Exact(2)),
            return_type_pattern: Some("BOOL".into()),
            dllimport_only: true,
            ..base_func_filter()
        };
        let funcs = collect_funcs_filtered(&tu, &filter).map_err(anyhow::Error::msg)?;

        for f in &funcs {
            assert_eq!(f.get_params().len(), 2);
            assert_eq!(f.get_return_type_name(), "BOOL");
            assert!(f.is_dllimport());
        }

        Ok(())
    }

    /* ────────────────────────── WHERE clause filter ──────────────────────── */

    #[test]
    #[serial]
    fn where_param_count_gt() -> anyhow::Result<()> {
        winsdk!(clang, index, tu);

        let funcs = collect_funcs(&tu);
        let expr = parse_where("params > 5").unwrap();
        let filtered: Vec<_> = funcs.iter().filter(|f| eval_where(&expr, f)).collect();

        assert!(
            !filtered.is_empty(),
            "should find functions with > 5 params"
        );
        for f in &filtered {
            assert!(
                f.get_params().len() > 5,
                "'{}' has {} params, expected > 5",
                f.get_name(),
                f.get_params().len()
            );
        }

        Ok(())
    }

    #[test]
    #[serial]
    fn where_return_type_eq() -> anyhow::Result<()> {
        winsdk!(clang, index, tu);

        let filter = FuncFilter {
            header_filter: Some("handleapi.h".into()),
            ..base_func_filter()
        };
        let funcs = collect_funcs_filtered(&tu, &filter).map_err(anyhow::Error::msg)?;
        let expr = parse_where("return_type = 'BOOL'").unwrap();
        let filtered: Vec<_> = funcs.iter().filter(|f| eval_where(&expr, f)).collect();

        assert!(!filtered.is_empty());
        for f in &filtered {
            assert_eq!(f.get_return_type_name(), "BOOL");
        }

        Ok(())
    }

    #[test]
    #[serial]
    fn where_name_like() -> anyhow::Result<()> {
        winsdk!(clang, index, tu);

        let filter = FuncFilter {
            header_filter: Some("handleapi.h".into()),
            ..base_func_filter()
        };
        let funcs = collect_funcs_filtered(&tu, &filter).map_err(anyhow::Error::msg)?;
        let expr = parse_where("name LIKE '%Handle%'").unwrap();
        let filtered: Vec<_> = funcs.iter().filter(|f| eval_where(&expr, f)).collect();

        assert!(
            filtered.len() >= 3,
            "should find multiple *Handle* functions"
        );
        for f in &filtered {
            assert!(
                f.get_name().to_lowercase().contains("handle"),
                "'{}' should contain 'Handle'",
                f.get_name()
            );
        }

        Ok(())
    }

    #[test]
    #[serial]
    fn where_compound_and_or() -> anyhow::Result<()> {
        winsdk!(clang, index, tu);

        let filter = FuncFilter {
            header_filter: Some("handleapi.h".into()),
            ..base_func_filter()
        };
        let funcs = collect_funcs_filtered(&tu, &filter).map_err(anyhow::Error::msg)?;
        let expr =
            parse_where("params > 3 AND (return_type = 'BOOL' OR return_type = 'HANDLE')").unwrap();
        let filtered: Vec<_> = funcs.iter().filter(|f| eval_where(&expr, f)).collect();

        for f in &filtered {
            assert!(f.get_params().len() > 3);
            assert!(
                f.get_return_type_name() == "BOOL" || f.get_return_type_name() == "HANDLE",
                "'{}' returns '{}', expected BOOL or HANDLE",
                f.get_name(),
                f.get_return_type_name()
            );
        }

        Ok(())
    }

    #[test]
    #[serial]
    fn where_is_exported() -> anyhow::Result<()> {
        winsdk!(clang, index, tu);

        let filter = FuncFilter {
            header_filter: Some("handleapi.h".into()),
            ..base_func_filter()
        };
        let funcs = collect_funcs_filtered(&tu, &filter).map_err(anyhow::Error::msg)?;
        let expr = parse_where("is_exported = true").unwrap();
        let filtered: Vec<_> = funcs.iter().filter(|f| eval_where(&expr, f)).collect();

        for f in &filtered {
            assert!(f.is_dllimport(), "'{}' should be exported", f.get_name());
        }

        Ok(())
    }

    #[test]
    #[serial]
    fn where_between() -> anyhow::Result<()> {
        winsdk!(clang, index, tu);

        let funcs = collect_funcs(&tu);
        let expr = parse_where("params BETWEEN 2 AND 4").unwrap();
        let filtered: Vec<_> = funcs.iter().filter(|f| eval_where(&expr, f)).collect();

        assert!(!filtered.is_empty());
        for f in &filtered {
            let n = f.get_params().len();
            assert!(
                (2..=4).contains(&n),
                "'{}' has {} params, expected 2..=4",
                f.get_name(),
                n
            );
        }

        Ok(())
    }

    #[test]
    #[serial]
    fn where_in_list() -> anyhow::Result<()> {
        winsdk!(clang, index, tu);

        let filter = FuncFilter {
            header_filter: Some("handleapi.h".into()),
            ..base_func_filter()
        };
        let funcs = collect_funcs_filtered(&tu, &filter).map_err(anyhow::Error::msg)?;
        let expr = parse_where("name IN ('CloseHandle', 'DuplicateHandle')").unwrap();
        let filtered: Vec<_> = funcs.iter().filter(|f| eval_where(&expr, f)).collect();

        assert_eq!(filtered.len(), 2);
        assert!(filtered.iter().any(|f| f.get_name() == "CloseHandle"));
        assert!(filtered.iter().any(|f| f.get_name() == "DuplicateHandle"));

        Ok(())
    }

    #[test]
    #[serial]
    fn where_not_negation() -> anyhow::Result<()> {
        winsdk!(clang, index, tu);

        let filter = FuncFilter {
            header_filter: Some("handleapi.h".into()),
            ..base_func_filter()
        };
        let funcs = collect_funcs_filtered(&tu, &filter).map_err(anyhow::Error::msg)?;
        let all_count = funcs.len();
        let expr = parse_where("NOT name LIKE '%Close%'").unwrap();
        let filtered: Vec<_> = funcs.iter().filter(|f| eval_where(&expr, f)).collect();

        assert!(filtered.len() < all_count);
        for f in &filtered {
            assert!(
                !f.get_name().to_lowercase().contains("close"),
                "'{}' should not contain 'close' (case-insensitive)",
                f.get_name()
            );
        }

        Ok(())
    }

    #[test]
    #[serial]
    fn where_header_filter() -> anyhow::Result<()> {
        winsdk!(clang, index, tu);

        let funcs = collect_funcs(&tu);
        let expr = parse_where("header = 'handleapi.h'").unwrap();
        let filtered: Vec<_> = funcs.iter().filter(|f| eval_where(&expr, f)).collect();

        assert!(!filtered.is_empty());
        for f in &filtered {
            let file = f
                .get_location()
                .and_then(|l| l.file.clone())
                .unwrap_or_default();
            assert_eq!(
                file.to_lowercase(),
                "handleapi.h",
                "'{}' should be in handleapi.h",
                f.get_name()
            );
        }

        Ok(())
    }

    #[test]
    #[serial]
    fn where_invalid_sql_returns_err() {
        assert!(parse_where("???invalid!!!").is_err());
        assert!(parse_where("").is_err());
    }

    #[test]
    #[serial]
    fn where_invalid_sql_propagates_through_filter() -> anyhow::Result<()> {
        winsdk!(clang, index, tu);

        let filter = FuncFilter {
            where_clause: Some("???invalid!!!".into()),
            ..base_func_filter()
        };
        let result = collect_funcs_filtered(&tu, &filter);
        assert!(
            result.is_err(),
            "invalid WHERE should propagate as an error, not silently return all results"
        );

        Ok(())
    }

    /* ─────────────────────── Sort keys (stack/param) ───────────────────── */

    #[test]
    #[serial]
    fn sort_by_stack_size() -> anyhow::Result<()> {
        winsdk!(clang, index, tu);

        let filter = FuncFilter {
            sort: Some(FuncSort::StackSize),
            dllimport_only: true,
            ..base_func_filter()
        };
        let funcs = collect_funcs_filtered(&tu, &filter).map_err(anyhow::Error::msg)?;

        assert!(funcs.len() > 1);
        let sizes: Vec<usize> = funcs
            .iter()
            .map(|f| {
                f.get_params()
                    .iter()
                    .filter_map(|p| match p.get_abi_location() {
                        ParamLocation::Direct { locations, size }
                            if locations
                                .first()
                                .is_some_and(|l| matches!(l, MemoryOperand::RegImm { .. })) =>
                        {
                            Some(*size)
                        }
                        _ => None,
                    })
                    .sum()
            })
            .collect();

        for w in sizes.windows(2) {
            assert!(w[0] <= w[1], "stack sizes should be ascending");
        }

        Ok(())
    }

    #[test]
    #[serial]
    fn sort_max_stack_param_desc() -> anyhow::Result<()> {
        winsdk!(clang, index, tu, Arch::X86, SdkMode::User);

        let filter = FuncFilter {
            header_filter: Some("fileapi.h".into()),
            sort: Some(FuncSort::MaxStackParam),
            sort_dir: bb_funcs_lib::SortDir::Desc,
            dllimport_only: true,
            ..base_func_filter()
        };
        let funcs = collect_funcs_filtered(&tu, &filter).map_err(anyhow::Error::msg)?;

        assert!(funcs.len() > 1);
        let sizes: Vec<usize> = funcs
            .iter()
            .map(|f| {
                f.get_params()
                    .iter()
                    .filter_map(|p| match p.get_abi_location() {
                        ParamLocation::Direct { locations, size }
                            if locations
                                .first()
                                .is_some_and(|l| matches!(l, MemoryOperand::RegImm { .. })) =>
                        {
                            Some(*size)
                        }
                        _ => None,
                    })
                    .max()
                    .unwrap_or(0)
            })
            .collect();

        for w in sizes.windows(2) {
            assert!(w[0] >= w[1], "max stack param sizes should be descending");
        }

        Ok(())
    }

    /* ──────────────────────────── --first limit ────────────────────────── */

    #[test]
    #[serial]
    fn first_limits_results() -> anyhow::Result<()> {
        winsdk!(clang, index, tu);

        let all = collect_funcs(&tu);
        assert!(all.len() > 10, "should have many functions");

        // Simulate --first 3 by truncating.
        let mut limited = all;
        limited.truncate(3);
        assert_eq!(limited.len(), 3);

        Ok(())
    }

    /* ──────────────── Constant expression field (issue #9) ──────────────── */

    #[test]
    #[serial]
    fn macro_constant_has_expression() -> anyhow::Result<()> {
        winsdk!(clang, index, tu);

        let vars = collect_constants(&tu, &no_filter());

        // MAX_PATH is a simple #define MAX_PATH 260 — the expression is just "260".
        let max_path = vars
            .iter()
            .find(|c| c.get_name() == "MAX_PATH")
            .expect("MAX_PATH must exist");
        assert!(max_path.is_macro(), "MAX_PATH should be a macro");
        let expr = max_path.get_expression();
        assert!(expr.is_some(), "macro constants should have an expression");
        assert!(!expr.unwrap().is_empty(), "expression should not be empty");

        Ok(())
    }

    #[test]
    #[serial]
    fn macro_expression_in_json() -> anyhow::Result<()> {
        winsdk!(clang, index, tu);

        let vars = collect_constants(&tu, &no_filter());
        let max_path = vars
            .iter()
            .find(|c| c.get_name() == "MAX_PATH")
            .expect("MAX_PATH must exist");

        let j = max_path.to_json();
        assert!(
            j["expression"].is_string(),
            "JSON should have expression field for macros"
        );

        Ok(())
    }

    #[test]
    #[serial]
    fn enum_constant_expression() -> anyhow::Result<()> {
        winsdk!(clang, index, tu);

        let enums = collect_enums(&tu, &no_filter());

        // Find any enum with children — most have explicit values.
        let enum_with_values = enums.iter().find(|e| {
            e.get_constants()
                .iter()
                .any(|c| c.get_expression().is_some())
        });
        assert!(
            enum_with_values.is_some(),
            "should find at least one enum constant with an expression"
        );

        Ok(())
    }

    /* ──────────────── Field type metadata (issue #10) ─────────────────── */

    #[test]
    #[serial]
    fn field_json_has_type_metadata() -> anyhow::Result<()> {
        winsdk!(clang, index, tu);

        let filter = StructFilter {
            name_pattern: Some("_GUID".into()),
            header_filter: None,
            case_sensitive: true,
        };
        let structs = collect_structs(&tu, &filter, None);
        let guid = structs
            .into_iter()
            .next()
            .ok_or_else(|| anyhow::anyhow!("GUID must exist"))?;

        let j = guid.to_json();
        let fields = j["fields"].as_array().expect("should have fields");
        assert!(!fields.is_empty());

        // Every field should have the type metadata properties.
        for field in fields {
            assert!(
                field["is_const"].is_boolean(),
                "field should have is_const boolean: {field}"
            );
            assert!(
                field["is_pointer"].is_boolean(),
                "field should have is_pointer boolean: {field}"
            );
            assert!(
                field["is_array"].is_boolean(),
                "field should have is_array boolean: {field}"
            );
        }

        Ok(())
    }

    #[test]
    #[serial]
    fn field_array_detection() -> anyhow::Result<()> {
        winsdk!(clang, index, tu);

        // _GUID has Data4[8] which is a fixed-size array.
        let filter = StructFilter {
            name_pattern: Some("_GUID".into()),
            header_filter: None,
            case_sensitive: true,
        };
        let structs = collect_structs(&tu, &filter, None);
        let guid = structs
            .into_iter()
            .next()
            .ok_or_else(|| anyhow::anyhow!("GUID must exist"))?;

        let j = guid.to_json();
        let fields = j["fields"].as_array().expect("should have fields");

        let data4 = fields
            .iter()
            .find(|f| f["name"] == "Data4")
            .expect("GUID should have Data4 field");
        assert_eq!(data4["is_array"], true, "Data4 should be an array");
        assert_eq!(data4["array_size"], 8, "Data4 should have 8 elements");
        assert_eq!(data4["is_pointer"], false, "Data4 should not be a pointer");

        Ok(())
    }

    #[test]
    #[serial]
    fn field_pointer_detection() -> anyhow::Result<()> {
        winsdk!(clang, index, tu, Arch::Amd64, SdkMode::User);

        // Find a struct with a pointer field. _PEB has many.
        let filter = StructFilter {
            name_pattern: Some("_PEB".into()),
            header_filter: None,
            case_sensitive: true,
        };
        let structs = collect_structs(&tu, &filter, None);
        let peb = structs
            .into_iter()
            .next()
            .ok_or_else(|| anyhow::anyhow!("PEB must exist"))?;

        let j = peb.to_json();
        let fields = j["fields"].as_array().expect("should have fields");

        // PEB has pointer fields.
        let has_pointer = fields.iter().any(|f| f["is_pointer"] == true);
        assert!(has_pointer, "PEB should have at least one pointer field");

        // Pointer fields with underlying_type show what they point to.
        let pointer_with_underlying = fields
            .iter()
            .find(|f| f["is_pointer"] == true && f["underlying_type"].is_string());
        assert!(
            pointer_with_underlying.is_some(),
            "should find a pointer field with underlying_type set"
        );

        Ok(())
    }

    /* ──────────────────────────────── Typedefs ──────────────────────────────── */

    /// `TypedefIndex` resolves `LARGE_INTEGER` to the canonical
    /// `_LARGE_INTEGER` struct, with the full chain captured.
    #[test]
    #[serial]
    fn typedef_index_resolves_large_integer() -> anyhow::Result<()> {
        winsdk!(clang, index, tu);

        let idx = TypedefIndex::build(&tu);
        let td = idx
            .lookup("LARGE_INTEGER")
            .ok_or_else(|| anyhow::anyhow!("LARGE_INTEGER typedef must exist"))?;

        assert_eq!(td.name, "LARGE_INTEGER");
        // LARGE_INTEGER is a union in the Windows headers.
        assert!(
            matches!(td.kind, TypedefKind::Struct | TypedefKind::Union),
            "LARGE_INTEGER should resolve to a struct/union, got {:?}",
            td.kind
        );
        assert_eq!(
            td.canonical_decl_name.as_deref(),
            Some("_LARGE_INTEGER"),
            "canonical_decl_name should be the underlying record's name"
        );
        assert!(
            td.chain.iter().any(|s| s.contains("_LARGE_INTEGER")),
            "chain should end at _LARGE_INTEGER, got {:?}",
            td.chain
        );

        // Reverse mapping: aliases_for(_LARGE_INTEGER) must include LARGE_INTEGER.
        let aliases = idx.aliases_for("_LARGE_INTEGER");
        assert!(
            aliases.iter().any(|a| a == "LARGE_INTEGER"),
            "_LARGE_INTEGER should have LARGE_INTEGER as an alias, got {aliases:?}"
        );

        Ok(())
    }

    /// `HANDLE` is a pointer typedef: chain ends at `void *` and the kind
    /// is `Pointer`. No canonical struct to point to.
    #[test]
    #[serial]
    fn typedef_index_resolves_handle_to_void_pointer() -> anyhow::Result<()> {
        winsdk!(clang, index, tu);

        let idx = TypedefIndex::build(&tu);
        let handle = idx
            .lookup("HANDLE")
            .ok_or_else(|| anyhow::anyhow!("HANDLE typedef must exist"))?;

        assert_eq!(handle.name, "HANDLE");
        assert_eq!(handle.kind, TypedefKind::Pointer);
        assert!(
            handle.canonical_decl_name.is_none(),
            "HANDLE should not have a canonical_decl_name (it bottoms out at void *), got {:?}",
            handle.canonical_decl_name
        );
        assert!(
            handle.canonical.contains("void"),
            "HANDLE canonical should mention void, got {:?}",
            handle.canonical
        );
        assert!(
            !handle.chain.is_empty(),
            "HANDLE chain must not be empty, got {:?}",
            handle.chain
        );
        assert!(
            handle.chain.last().is_some_and(|s| s.contains("void")),
            "HANDLE chain should end at void *, got {:?}",
            handle.chain
        );

        Ok(())
    }

    /// `bb-types -s FILETIME` resolves through the typedef and returns
    /// the canonical `_FILETIME` struct with the alias attached.
    /// (`_FILETIME` is a struct with a real typedef alias; the prior
    /// `_LARGE_INTEGER` fixture was a union and is no longer findable
    /// as a struct after the Union refactor.)
    #[test]
    #[serial]
    fn typedef_lookup_finds_canonical_struct() -> anyhow::Result<()> {
        winsdk!(clang, index, tu);

        let idx = TypedefIndex::build(&tu);
        let filter = StructFilter {
            name_pattern: Some("FILETIME".into()),
            header_filter: None,
            case_sensitive: true,
        };
        let structs = collect_structs(&tu, &filter, Some(&idx));

        let canonical = find_struct(&structs, "_FILETIME")
            .ok_or_else(|| anyhow::anyhow!("typedef search should hit _FILETIME"))?;
        assert!(
            canonical.get_aliases().iter().any(|a| a == "FILETIME"),
            "_FILETIME should have aliases including FILETIME, got {:?}",
            canonical.get_aliases()
        );

        Ok(())
    }

    /// Searching by canonical name (`_FILETIME`) still attaches the
    /// typedef aliases to the struct, so JSON consumers always see both
    /// names regardless of which one the user typed.
    #[test]
    #[serial]
    fn canonical_lookup_still_attaches_aliases() -> anyhow::Result<()> {
        winsdk!(clang, index, tu);

        let idx = TypedefIndex::build(&tu);
        let filter = StructFilter {
            name_pattern: Some("_FILETIME".into()),
            header_filter: None,
            case_sensitive: true,
        };
        let structs = collect_structs(&tu, &filter, Some(&idx));
        let canonical = find_struct(&structs, "_FILETIME")
            .ok_or_else(|| anyhow::anyhow!("canonical search should find _FILETIME"))?;
        assert!(
            canonical.get_aliases().iter().any(|a| a == "FILETIME"),
            "aliases should be attached regardless of which name was searched, got {:?}",
            canonical.get_aliases()
        );

        Ok(())
    }

    /// The struct JSON includes a top-level `aliases` array (empathic
    /// API shape — programmers don't have to follow a reference to learn
    /// the typedef name).
    #[test]
    #[serial]
    fn struct_json_includes_aliases_field() -> anyhow::Result<()> {
        winsdk!(clang, index, tu);

        let idx = TypedefIndex::build(&tu);
        let filter = StructFilter {
            name_pattern: Some("_FILETIME".into()),
            header_filter: None,
            case_sensitive: true,
        };
        let structs = collect_structs(&tu, &filter, Some(&idx));
        let canonical = find_struct(&structs, "_FILETIME")
            .ok_or_else(|| anyhow::anyhow!("must find _FILETIME"))?;

        let j = canonical.to_json();
        let aliases = j["aliases"]
            .as_array()
            .expect("aliases must be an array in JSON");
        let alias_names: Vec<&str> = aliases
            .iter()
            .filter_map(serde_json::Value::as_str)
            .collect();
        assert!(
            alias_names.contains(&"FILETIME"),
            "JSON aliases array should contain FILETIME, got {alias_names:?}"
        );

        Ok(())
    }

    /// Typedefs that resolve to a struct expose `canonical_decl_name`,
    /// which is how API consumers cross-reference back to the canonical
    /// struct's `name` field. Symmetric with `Struct.aliases`.
    #[test]
    #[serial]
    fn typedef_serializes_with_canonical_decl_name() -> anyhow::Result<()> {
        winsdk!(clang, index, tu);

        let idx = TypedefIndex::build(&tu);
        let td = idx
            .lookup("LARGE_INTEGER")
            .ok_or_else(|| anyhow::anyhow!("LARGE_INTEGER must exist"))?;

        let j = serde_json::to_value(td).expect("Typedef should serialize");
        assert_eq!(j["name"], "LARGE_INTEGER");
        assert_eq!(j["canonical"], "_LARGE_INTEGER");
        assert_eq!(j["canonical_decl_name"], "_LARGE_INTEGER");
        // chain.first() carries what typedef_of used to be; chain.last()
        // equals canonical.
        assert_eq!(j["chain"][0], "_LARGE_INTEGER");
        // SDK encodes LARGE_INTEGER as a union; older or different SDKs
        // could use a struct. Accept both.
        let kind_str = j["kind"].as_str().expect("kind should be a string");
        assert!(
            kind_str == "struct" || kind_str == "union",
            "LARGE_INTEGER kind should be struct or union, got {kind_str:?}"
        );
        let chain = j["chain"].as_array().expect("chain should be an array");
        assert!(!chain.is_empty(), "chain should not be empty");

        Ok(())
    }

    /// HANDLE's JSON shape is the pointer-typedef shape: `canonical` is
    /// `void *`, no `canonical_decl_name`, `kind` is `pointer`.
    #[test]
    #[serial]
    fn typedef_handle_serializes_as_pointer() -> anyhow::Result<()> {
        winsdk!(clang, index, tu);

        let idx = TypedefIndex::build(&tu);
        let handle = idx
            .lookup("HANDLE")
            .ok_or_else(|| anyhow::anyhow!("HANDLE must exist"))?;

        let j = serde_json::to_value(handle).expect("HANDLE should serialize");
        assert_eq!(j["name"], "HANDLE");
        assert_eq!(j["kind"], "pointer");
        assert!(
            j["canonical"].as_str().is_some_and(|s| s.contains("void")),
            "canonical should mention void, got {:?}",
            j["canonical"]
        );
        // canonical_decl_name should be absent (skipped because None).
        assert!(
            j["canonical_decl_name"].is_null() || j.get("canonical_decl_name").is_none(),
            "pointer typedef should omit canonical_decl_name, got {:?}",
            j["canonical_decl_name"]
        );

        Ok(())
    }

    /// `find_typedef_hits` returns HANDLE when searched by its name —
    /// this is how `bb-types -s HANDLE` surfaces a typedef-only result.
    #[test]
    #[serial]
    fn typedef_only_search_finds_handle() -> anyhow::Result<()> {
        winsdk!(clang, index, tu);

        let idx = TypedefIndex::build(&tu);
        let filter = StructFilter {
            name_pattern: Some("HANDLE".into()),
            header_filter: None,
            case_sensitive: true,
        };
        let hits = find_typedef_hits(&idx, &filter);
        assert!(
            hits.iter().any(|t| t.name == "HANDLE"),
            "search for HANDLE should yield a HANDLE typedef hit"
        );

        Ok(())
    }

    /// The `display()` renderer annotates HANDLE / PVOID / typedef'd
    /// fields with their canonical form. This is the "expand HANDLE in
    /// dt" check: rendered output must show both `HANDLE` and `void *`.
    #[test]
    #[serial]
    fn display_annotates_pointer_typedef_fields() -> anyhow::Result<()> {
        winsdk!(clang, index, tu);

        let idx = TypedefIndex::build(&tu);
        let filter = StructFilter {
            name_pattern: Some("_OVERLAPPED".into()),
            header_filter: None,
            case_sensitive: true,
        };
        let structs = collect_structs(&tu, &filter, Some(&idx));
        let overlapped = find_struct(&structs, "_OVERLAPPED")
            .ok_or_else(|| anyhow::anyhow!("_OVERLAPPED must exist"))?;

        let rendered = overlapped.display(0, None, Some(&idx));

        // _OVERLAPPED has a HANDLE field (hEvent) — output must show
        // both the typedef name and the canonical form.
        assert!(
            rendered.contains("HANDLE"),
            "render must mention HANDLE field type"
        );
        assert!(
            rendered.contains("void"),
            "render must annotate HANDLE with its canonical void * form, got:\n{rendered}"
        );

        Ok(())
    }

    /// Rendered struct header shows the `[aka …]` chip when aliases are
    /// attached. Visible in both colored and stripped output. Uses
    /// `_FILETIME` (struct + typedef `FILETIME`) since named unions
    /// now go through the parallel `collect_unions` path.
    #[test]
    #[serial]
    fn display_renders_aliases_chip() -> anyhow::Result<()> {
        winsdk!(clang, index, tu);

        let idx = TypedefIndex::build(&tu);
        let filter = StructFilter {
            name_pattern: Some("_FILETIME".into()),
            header_filter: None,
            case_sensitive: true,
        };
        let structs = collect_structs(&tu, &filter, Some(&idx));
        let canonical = find_struct(&structs, "_FILETIME")
            .ok_or_else(|| anyhow::anyhow!("must find _FILETIME"))?;

        let rendered = canonical.display(0, None, Some(&idx));
        assert!(
            rendered.contains("[aka") && rendered.contains("FILETIME"),
            "header should show `[aka FILETIME]` chip, got:\n{rendered}"
        );

        Ok(())
    }

    /* ───────────────────────────────── Unions ───────────────────────────────── */

    /// `_LARGE_INTEGER` is a top-level named union. It must be findable
    /// via `collect_unions` (the union counterpart of `collect_structs`)
    /// but NOT via `collect_structs` — the Struct/Union types are
    /// strictly disjoint.
    #[test]
    #[serial]
    fn named_union_findable_via_collect_unions() -> anyhow::Result<()> {
        winsdk!(clang, index, tu);

        let idx = TypedefIndex::build(&tu);
        let filter = StructFilter {
            name_pattern: Some("_LARGE_INTEGER".into()),
            header_filter: None,
            case_sensitive: true,
        };
        let unions = collect_unions(&tu, &filter, Some(&idx));
        let li = unions
            .iter()
            .find(|u| u.get_name() == "_LARGE_INTEGER")
            .ok_or_else(|| anyhow::anyhow!("_LARGE_INTEGER must be findable as a union"))?;

        assert_eq!(li.kind(), RecordKind::Union);
        assert!(
            li.get_aliases().iter().any(|a| a == "LARGE_INTEGER"),
            "LARGE_INTEGER must be attached as an alias"
        );

        // The same name must NOT resolve via the struct path.
        let structs = collect_structs(&tu, &filter, Some(&idx));
        assert!(
            structs.is_empty(),
            "_LARGE_INTEGER must not appear in collect_structs (it's a union)"
        );

        Ok(())
    }

    /// Typedef-name search (`LARGE_INTEGER`) reaches the canonical union
    /// `_LARGE_INTEGER`. Mirrors `typedef_lookup_finds_canonical_struct`
    /// for the union path.
    #[test]
    #[serial]
    fn typedef_lookup_finds_canonical_union() -> anyhow::Result<()> {
        winsdk!(clang, index, tu);

        let idx = TypedefIndex::build(&tu);
        let filter = StructFilter {
            name_pattern: Some("LARGE_INTEGER".into()),
            header_filter: None,
            case_sensitive: true,
        };
        let unions = collect_unions(&tu, &filter, Some(&idx));
        let li = unions
            .iter()
            .find(|u| u.get_name() == "_LARGE_INTEGER")
            .ok_or_else(|| anyhow::anyhow!("typedef LARGE_INTEGER should hit _LARGE_INTEGER"))?;
        assert!(li.get_aliases().iter().any(|a| a == "LARGE_INTEGER"));

        Ok(())
    }

    /// Union JSON shape mirrors Struct: `name`, `kind: "union"`,
    /// `aliases`, `fields`, `referenced_types`.
    #[test]
    #[serial]
    fn union_json_shape() -> anyhow::Result<()> {
        winsdk!(clang, index, tu);

        let idx = TypedefIndex::build(&tu);
        let li = find_union_by_name(&tu, "_LARGE_INTEGER", Some(&idx))
            .ok_or_else(|| anyhow::anyhow!("_LARGE_INTEGER must exist"))?;

        let j = li.to_json();
        assert_eq!(j["name"], "_LARGE_INTEGER");
        assert_eq!(j["kind"], "union");
        assert!(j["fields"].is_array(), "union must serialize fields array");
        assert!(
            j["referenced_types"].is_array(),
            "union must include referenced_types slot"
        );
        let aliases = j["aliases"].as_array().expect("aliases must be an array");
        let names: Vec<&str> = aliases
            .iter()
            .filter_map(serde_json::Value::as_str)
            .collect();
        assert!(
            names.contains(&"LARGE_INTEGER"),
            "aliases should contain LARGE_INTEGER, got {names:?}"
        );

        Ok(())
    }

    /// `_OVERLAPPED` has a nameless inner anonymous union (and inside
    /// that, a nameless anonymous struct). The new design represents
    /// these via synthetic `<anonymous_N>` names + an `anon_ref` on
    /// the field that points into the parent struct's
    /// `referenced_types` slot. The two anonymous entries (union and
    /// inner struct) coexist in the single `referenced_types` array,
    /// distinguished by per-entry `kind`.
    #[test]
    #[serial]
    fn overlapped_anon_records_in_referenced_types() -> anyhow::Result<()> {
        winsdk!(clang, index, tu);

        let overlapped = find_struct_by_name(&tu, "_OVERLAPPED", None)
            .ok_or_else(|| anyhow::anyhow!("_OVERLAPPED must exist"))?;

        // The anonymous union field is nameless in C — Field carries a
        // synthetic `<anonymous_N>` name and the `is_anonymous` flag.
        let anon_field = overlapped
            .get_fields()
            .iter()
            .find(|f| f.is_anonymous() && f.get_anon_ref().is_some())
            .ok_or_else(|| {
                anyhow::anyhow!("_OVERLAPPED must have an anonymous union field with anon_ref")
            })?;

        let aref = anon_field
            .get_anon_ref()
            .expect("filter selected only fields with anon_ref");
        assert_eq!(aref.kind, RecordKind::Union);
        assert_eq!(aref.enclosing_record, "_OVERLAPPED");
        assert_eq!(aref.field_path.len(), 1);
        assert!(
            aref.field_path[0].starts_with("<anonymous_"),
            "field_path element should be a synthetic identifier, got {:?}",
            aref.field_path[0]
        );

        // Full JSON exposes BOTH the anonymous union and the inner
        // anonymous struct in the single `referenced_types` slot,
        // distinguished by per-entry `kind`.
        let full = overlapped.to_json_full();
        let referenced = full["referenced_types"]
            .as_array()
            .expect("referenced_types must be an array");
        assert!(
            referenced
                .iter()
                .any(|u| u["enclosing_record"] == "_OVERLAPPED" && u["kind"] == "union"),
            "_OVERLAPPED's anonymous union must surface in referenced_types, got:\n{}",
            serde_json::to_string_pretty(&full["referenced_types"]).unwrap()
        );
        assert!(
            referenced
                .iter()
                .any(|s| s["enclosing_record"] == "_OVERLAPPED" && s["kind"] == "struct"),
            "_OVERLAPPED's nested anonymous struct must surface in referenced_types, got:\n{}",
            serde_json::to_string_pretty(&full["referenced_types"]).unwrap()
        );

        Ok(())
    }

    /// `iter_unions` yields only top-level (named) unions. Anonymous
    /// unions never appear at the TU root.
    #[test]
    #[serial]
    fn iter_unions_yields_named_only() -> anyhow::Result<()> {
        winsdk!(clang, index, tu);

        let names: Vec<String> = iter_unions(&tu).filter_map(|e| e.get_name()).collect();
        assert!(
            !names.is_empty(),
            "expected at least some named unions (e.g. _LARGE_INTEGER)"
        );
        assert!(
            names.iter().any(|n| n == "_LARGE_INTEGER"),
            "iter_unions must include _LARGE_INTEGER"
        );

        Ok(())
    }

    /// `_OVERLAPPED`'s field list must contain exactly 4 entries
    /// after the anon-record synthesis: three named C FieldDecls and
    /// the synthesized union slot at the right offset. This guards
    /// against silent regressions in `build_anon_record_field` or
    /// `anon_record_offset_in_parent` — both of which would cause
    /// the synthetic entry to vanish, land at offset 0, or duplicate.
    #[test]
    #[serial]
    fn overlapped_synthesizes_anon_union_at_offset_16() -> anyhow::Result<()> {
        winsdk!(clang, index, tu);

        let overlapped = find_struct_by_name(&tu, "_OVERLAPPED", None)
            .ok_or_else(|| anyhow::anyhow!("_OVERLAPPED must exist"))?;

        let fields = overlapped.get_fields();
        assert_eq!(
            fields.len(),
            4,
            "_OVERLAPPED must have exactly 4 member slots (3 named + 1 synthetic anon union), got {}",
            fields.len()
        );

        // Three named FieldDecls.
        assert_eq!(fields[0].get_name(), "Internal");
        assert_eq!(fields[0].get_offset_bytes(), 0);
        assert!(!fields[0].is_anonymous());
        assert!(fields[0].get_field_decl().is_some());

        assert_eq!(fields[1].get_name(), "InternalHigh");
        assert_eq!(fields[1].get_offset_bytes(), 8);

        assert_eq!(fields[3].get_name(), "hEvent");
        assert_eq!(fields[3].get_offset_bytes(), 24);

        // Slot 2 is the synthetic anon union — non-zero offset is the
        // critical correctness bit (the unwrap_or(0) fallback bug
        // would have placed it at 0).
        let anon = &fields[2];
        assert!(anon.is_anonymous());
        assert!(anon.get_name().starts_with("<anonymous_"));
        assert_eq!(
            anon.get_offset_bytes(),
            16,
            "synthetic anon union must sit at offset 16, got {}",
            anon.get_offset_bytes()
        );
        assert_eq!(anon.get_size(), 8);

        // Synthetic fields have no FieldDecl; their `entity` is the
        // anon record decl itself.
        assert!(
            anon.get_field_decl().is_none(),
            "synthetic anon entry must not expose a FieldDecl"
        );

        // anon_ref carries the cross-reference identity.
        let aref = anon
            .get_anon_ref()
            .ok_or_else(|| anyhow::anyhow!("synthetic anon entry must carry anon_ref"))?;
        assert_eq!(aref.kind, RecordKind::Union);
        assert_eq!(aref.enclosing_record, "_OVERLAPPED");
        assert_eq!(aref.field_path, vec!["<anonymous_0>"]);

        Ok(())
    }

    /// The inner anonymous struct inside `_OVERLAPPED`'s anonymous
    /// union appears in `referenced_types` with field_path
    /// `["<anonymous_0>", "<anonymous_0>"]` — proving the per-parent
    /// counter resets correctly at each nesting level (the design
    /// invariant from the spec).
    #[test]
    #[serial]
    fn overlapped_inner_anon_struct_has_nested_field_path() -> anyhow::Result<()> {
        winsdk!(clang, index, tu);

        let overlapped = find_struct_by_name(&tu, "_OVERLAPPED", None)
            .ok_or_else(|| anyhow::anyhow!("_OVERLAPPED must exist"))?;

        let full = overlapped.to_json_full();
        let referenced = full["referenced_types"]
            .as_array()
            .expect("referenced_types must be an array");

        let inner_struct = referenced
            .iter()
            .find(|e| e["kind"] == "struct" && e["enclosing_record"] == "_OVERLAPPED")
            .ok_or_else(|| anyhow::anyhow!("inner anon struct must surface in referenced_types"))?;
        let path: Vec<&str> = inner_struct["field_path"]
            .as_array()
            .expect("field_path must be array")
            .iter()
            .map(|v| v.as_str().unwrap_or(""))
            .collect();
        assert_eq!(
            path,
            vec!["<anonymous_0>", "<anonymous_0>"],
            "inner anon struct must have a two-element field_path matching the nesting depth, got {path:?}"
        );

        Ok(())
    }

    /// `collect_unions` returns `_LARGE_INTEGER` with the
    /// `LARGE_INTEGER` typedef alias attached.
    #[test]
    #[serial]
    fn collect_unions_surfaces_large_integer_with_alias() -> anyhow::Result<()> {
        winsdk!(clang, index, tu);

        let idx = TypedefIndex::build(&tu);
        let filter = StructFilter {
            name_pattern: Some("_LARGE_INTEGER".into()),
            header_filter: None,
            case_sensitive: true,
        };
        let unions = collect_unions(&tu, &filter, Some(&idx));
        let li = unions
            .iter()
            .find(|u| u.get_name() == "_LARGE_INTEGER")
            .ok_or_else(|| anyhow::anyhow!("collect_unions must surface _LARGE_INTEGER"))?;
        assert_eq!(li.kind(), RecordKind::Union);
        assert!(
            li.get_aliases().iter().any(|a| a == "LARGE_INTEGER"),
            "LARGE_INTEGER alias must be attached, got {:?}",
            li.get_aliases()
        );

        Ok(())
    }

    /// `Union::to_json_full` emits the same `{type, referenced_types}`
    /// shape as `Struct::to_json_full`.
    #[test]
    #[serial]
    fn union_to_json_full_shape() -> anyhow::Result<()> {
        winsdk!(clang, index, tu);

        let li = find_union_by_name(&tu, "_LARGE_INTEGER", None)
            .ok_or_else(|| anyhow::anyhow!("_LARGE_INTEGER must exist"))?;
        let full = li.to_json_full();
        assert_eq!(full["type"]["name"], "_LARGE_INTEGER");
        assert_eq!(full["type"]["kind"], "union");
        assert!(
            full["referenced_types"].is_array(),
            "to_json_full must include referenced_types"
        );

        Ok(())
    }

    /// `format_abi_param` and `render_function_detail` annotate HANDLE
    /// param types with their canonical form. This is the bb-funcs side
    /// of the dt expansion.
    #[test]
    #[serial]
    fn function_render_detail_annotates_handle_param() -> anyhow::Result<()> {
        winsdk!(clang, index, tu);

        let funcs: Vec<Function> = collect_funcs(&tu)
            .into_iter()
            .filter(|f| f.get_name() == "CloseHandle")
            .collect();
        let f = funcs
            .first()
            .ok_or_else(|| anyhow::anyhow!("CloseHandle must be parseable"))?;

        let idx = TypedefIndex::build(&tu);
        let rendered = f.display_detail(Some(&idx));

        assert!(
            rendered.contains("HANDLE"),
            "CloseHandle ABI render must mention HANDLE"
        );
        assert!(
            rendered.contains("void"),
            "CloseHandle ABI render must annotate HANDLE with `(void *)`, got:\n{rendered}"
        );

        Ok(())
    }

    /// `find_typedef_hits` returns empty when the pattern is `None`.
    #[test]
    #[serial]
    fn find_typedef_hits_empty_without_pattern() -> anyhow::Result<()> {
        winsdk!(clang, index, tu);

        let idx = TypedefIndex::build(&tu);
        let filter = StructFilter {
            name_pattern: None,
            header_filter: None,
            case_sensitive: false,
        };
        assert!(find_typedef_hits(&idx, &filter).is_empty());
        Ok(())
    }

    /// Field JSON now carries the **primitive** at the bottom of the
    /// canonical chain in `underlying_type` (e.g. `BOOL` → `int`,
    /// `DWORD` → `unsigned long`). This is the new
    /// `TypeProperties.underlying_type` semantics.
    #[test]
    #[serial]
    fn field_underlying_type_is_primitive() -> anyhow::Result<()> {
        winsdk!(clang, index, tu);

        let idx = TypedefIndex::build(&tu);
        let filter = StructFilter {
            name_pattern: Some("_SECURITY_ATTRIBUTES".into()),
            header_filter: None,
            case_sensitive: true,
        };
        let structs = collect_structs(&tu, &filter, Some(&idx));
        let sa = find_struct(&structs, "_SECURITY_ATTRIBUTES")
            .ok_or_else(|| anyhow::anyhow!("_SECURITY_ATTRIBUTES must exist"))?;

        let j = sa.to_json();
        let fields = j["fields"].as_array().expect("fields array");

        let n_length = fields
            .iter()
            .find(|f| f["name"] == "nLength")
            .expect("nLength field");
        assert_eq!(
            n_length["underlying_type"], "unsigned long",
            "DWORD's primitive should be unsigned long"
        );

        let b_inherit = fields
            .iter()
            .find(|f| f["name"] == "bInheritHandle")
            .expect("bInheritHandle field");
        assert_eq!(
            b_inherit["underlying_type"], "int",
            "BOOL's primitive should be int"
        );

        Ok(())
    }

    /// Typedef JSON gains a flattened `TypeProperties` so the shape
    /// matches Field/Param entries: `is_pointer`, `pointer_depth`,
    /// `underlying_type` (primitive), `underlying_record` (record name),
    /// etc. all appear at the top level.
    #[test]
    #[serial]
    fn typedef_json_includes_flattened_type_properties() -> anyhow::Result<()> {
        winsdk!(clang, index, tu);

        let idx = TypedefIndex::build(&tu);
        let handle = idx.lookup("HANDLE").expect("HANDLE must exist");
        let j = serde_json::to_value(handle).expect("Typedef serializes");

        // Pointer-typedef shape: is_pointer + underlying_type primitive,
        // no underlying_record.
        assert_eq!(j["is_pointer"], true);
        assert_eq!(j["pointer_depth"], 1);
        assert_eq!(j["underlying_type"], "void");
        assert!(
            j.get("underlying_record")
                .is_none_or(serde_json::Value::is_null),
            "HANDLE should not have underlying_record (void has no record name), got {:?}",
            j["underlying_record"]
        );
        assert_eq!(j["is_const"], false);
        assert_eq!(j["is_function_pointer"], false);

        // Struct-typedef shape: inverse.
        let li = idx
            .lookup("LARGE_INTEGER")
            .expect("LARGE_INTEGER must exist");
        let j2 = serde_json::to_value(li).expect("Typedef serializes");
        assert_eq!(j2["is_pointer"], false);
        assert_eq!(j2["underlying_record"], "_LARGE_INTEGER");
        assert!(
            j2.get("underlying_type")
                .is_none_or(serde_json::Value::is_null),
            "LARGE_INTEGER should not have a primitive underlying_type (leaf is a union), got {:?}",
            j2["underlying_type"]
        );

        Ok(())
    }

    /// Auto-expansion: searching a pointer typedef like
    /// `LPSECURITY_ATTRIBUTES` should let the caller pull in the
    /// `_SECURITY_ATTRIBUTES` struct it points to, so the response is
    /// self-contained. Mirrors the workflow `bb-types`'s main uses to
    /// expand `types[]` from `typedefs[]`.
    #[test]
    #[serial]
    fn pointer_typedef_search_expands_to_canonical_struct() -> anyhow::Result<()> {
        winsdk!(clang, index, tu);

        let idx = TypedefIndex::build(&tu);
        let filter = StructFilter {
            name_pattern: Some("LPSECURITY_ATTRIBUTES".into()),
            header_filter: None,
            case_sensitive: true,
        };

        // Initial struct search — pointer typedef name doesn't match any
        // struct decl name, so this comes back empty.
        let mut structs = collect_structs(&tu, &filter, Some(&idx));
        assert!(
            structs.is_empty(),
            "no struct is literally named LPSECURITY_ATTRIBUTES"
        );

        // Typedef hits surface the pointer typedef directly.
        let hits = find_typedef_hits(&idx, &filter);
        let lp = hits
            .iter()
            .find(|t| t.name == "LPSECURITY_ATTRIBUTES")
            .ok_or_else(|| anyhow::anyhow!("LPSECURITY_ATTRIBUTES should be a typedef hit"))?;

        // The hit's flattened TypeProperties point at the canonical struct.
        assert!(lp.properties.is_pointer);
        assert_eq!(
            lp.properties.underlying_record.as_deref(),
            Some("_SECURITY_ATTRIBUTES"),
            "pointer typedef should expose its underlying record"
        );

        // Expansion step: pull that struct in.
        if let Some(record_name) = lp.properties.underlying_record.as_deref()
            && let Some(s) = find_struct_by_name(&tu, record_name, Some(&idx))
        {
            structs.push(s);
        }

        let expanded = find_struct(&structs, "_SECURITY_ATTRIBUTES")
            .ok_or_else(|| anyhow::anyhow!("expansion should pull in _SECURITY_ATTRIBUTES"))?;
        assert!(
            expanded
                .get_aliases()
                .iter()
                .any(|a| a == "SECURITY_ATTRIBUTES"),
            "expanded struct should carry the direct typedef alias, got {:?}",
            expanded.get_aliases()
        );
        assert!(
            !expanded.get_fields().is_empty(),
            "expanded struct should have its fields populated"
        );

        Ok(())
    }

    /// `find_struct_by_name` resolves a canonical name to a single
    /// struct with aliases attached.
    #[test]
    #[serial]
    fn find_struct_by_name_attaches_aliases() -> anyhow::Result<()> {
        winsdk!(clang, index, tu);

        let idx = TypedefIndex::build(&tu);
        let s = find_struct_by_name(&tu, "_SECURITY_ATTRIBUTES", Some(&idx))
            .ok_or_else(|| anyhow::anyhow!("_SECURITY_ATTRIBUTES must resolve"))?;
        assert_eq!(s.get_name(), "_SECURITY_ATTRIBUTES");
        assert!(
            s.get_aliases().iter().any(|a| a == "SECURITY_ATTRIBUTES"),
            "aliases should include SECURITY_ATTRIBUTES, got {:?}",
            s.get_aliases()
        );

        Ok(())
    }

    /// `TypedefKind` serializes to lowercase snake_case so JSON consumers
    /// can switch/match on string values cleanly.
    #[test]
    fn typedef_kind_serializes_snake_case() {
        let v = serde_json::to_value(TypedefKind::FunctionPointer).unwrap();
        assert_eq!(v, "function_pointer");

        let v = serde_json::to_value(TypedefKind::Struct).unwrap();
        assert_eq!(v, "struct");

        let v = serde_json::to_value(TypedefKind::Pointer).unwrap();
        assert_eq!(v, "pointer");
    }

    /* ───────────────────────────────── Helpers ──────────────────────────────── */

    fn no_filter() -> ConstFilter {
        ConstFilter {
            header_filter: None,
            enum_pattern: None,
            const_pattern: None,
            case_sensitive: true,
            scoped_to_enum: false,
        }
    }
}
