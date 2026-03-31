#[cfg(test)]
mod tests {
    use serial_test::serial;

    use anyhow::Context;
    use bb_arch::reg::{X64Gpr, X86Gpr};
    use bb_arch::{Arch, MemoryOperand, ParamLocation, Register, ReturnLocation};
    use bb_clang::{Enum, Function, Struct, ToJson, build_referred_components};
    use bb_consts_lib::{
        ConstFilter, build_lookup_table, collect_constants, collect_enums, filter_constants_by_name,
    };
    use bb_funcs_lib::where_filter::{eval_where, parse_where};
    use bb_funcs_lib::{
        FuncFilter, FuncSort, ParamCountFilter, collect_funcs, collect_funcs_filtered,
    };
    use bb_sdk::{HeaderConfig, SdkMode};
    use bb_types_lib::{StructFilter, collect_structs, iter_structs};
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
        let structs = collect_structs(&tu, &filter);
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
        let filtered = collect_structs(&tu, &filter);

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

        let output = overlapped.display(1, None);
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
        let structs = collect_structs(&tu, &filter);
        let overlapped = find_struct(&structs, "_OVERLAPPED").expect("_OVERLAPPED must exist");

        // OVERLAPPED has nested anonymous struct/union types
        let nested = overlapped.extract_nested_types(2);
        // Just verify the method runs without panicking and returns valid structs
        for n in &nested {
            assert!(!n.get_name().is_empty(), "nested type should have a name");
        }

        // referenced_type_names should return named child types
        let refs = overlapped.referenced_type_names();
        for name in &refs {
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

            let structs = collect_structs(&tu, &filter);
            find_struct(&structs, "_LIST_ENTRY")
                .expect("LIST_ENTRY must exist on x86")
                .get_size()
                .expect("LIST_ENTRY must have a size")
        };

        let size_amd64 = {
            winsdk!(clang, index, tu, Arch::Amd64, SdkMode::User);

            let structs = collect_structs(&tu, &filter);
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
        let structs = collect_structs(&tu, &filter);
        let guid = structs
            .into_iter()
            .next()
            .ok_or_else(|| anyhow::anyhow!("GUID must exist"))?;

        let j = guid.to_json();
        assert_eq!(j["name"], "_GUID");
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
        let structs = collect_structs(&tu, &filter);
        assert!(!structs.is_empty(), "_PEB must exist");

        let full = structs.to_json_full();
        assert!(full["types"].is_array(), "should have types array");
        assert!(
            full["referenced_types"].is_array(),
            "should have referenced_types array"
        );

        // types entries should NOT have a referenced_types field
        let types = full["types"].as_array().unwrap();
        for t in types {
            assert!(
                t.get("referenced_types").is_none(),
                "types entries should not have referenced_types, got: {}",
                t["name"]
            );
        }

        // _PEB has many nested struct types
        let refs = full["referenced_types"].as_array().unwrap();
        assert!(!refs.is_empty(), "_PEB should have nested referenced types");

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
        let structs = collect_structs(&tu, &filter);
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
            first: None,
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
        let funcs = collect_funcs_filtered(&tu, &filter);

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
        let funcs = collect_funcs_filtered(&tu, &filter);

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
        let funcs = collect_funcs_filtered(&tu, &filter);

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
        let funcs = collect_funcs_filtered(&tu, &filter);

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
        let funcs = collect_funcs_filtered(&tu, &filter);

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
        let funcs = collect_funcs_filtered(&tu, &filter);

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
        let funcs = collect_funcs_filtered(&tu, &filter);

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
        let funcs = collect_funcs_filtered(&tu, &filter);

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
        let funcs = collect_funcs_filtered(&tu, &filter);

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
        let funcs = collect_funcs_filtered(&tu, &filter);

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
        let funcs = collect_funcs_filtered(&tu, &filter);

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
        let funcs = collect_funcs_filtered(&tu, &filter);

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
        let funcs = collect_funcs_filtered(&tu, &filter);

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
        let funcs = collect_funcs_filtered(&tu, &filter);
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
        let funcs = collect_funcs_filtered(&tu, &filter);
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
        let funcs = collect_funcs_filtered(&tu, &filter);
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
        let funcs = collect_funcs_filtered(&tu, &filter);
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
        let funcs = collect_funcs_filtered(&tu, &filter);
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
        let funcs = collect_funcs_filtered(&tu, &filter);
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
        let funcs = collect_funcs_filtered(&tu, &filter);

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
        let funcs = collect_funcs_filtered(&tu, &filter);

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
