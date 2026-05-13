# bb-sparse

> Embedded Windows API metadata from [sparse](https://github.com/cristeigabriela/sparse).

`bb-sparse` provides offline lookup of function-level documentation metadata extracted from MSDN: library/DLL info, version requirements, parameter directions (SAL), known constant values, and — for kernel/WDF DDIs — IRQL constraints, KMDF/UMDF version tags, `target-type`, `tech.root`, etc.

Two datasets are embedded:

| Dataset | Source repo | Function namespace |
| --- | --- | --- |
| **SDK** | [MicrosoftDocs/sdk-api](https://github.com/MicrosoftDocs/sdk-api) | user-mode Win32 (`CreateFileW`, `RegOpenKeyExA`, …) |
| **Driver** | [cristeigabriela/windows-driver-docs-ddi](https://github.com/cristeigabriela/windows-driver-docs-ddi) (fork pinned at `integration/all-open-prs`) | KMDF/UMDF + kernel DDIs (`WdfCollectionAdd`, `KeAcquireSpinLock`, …) |

Both JSON blobs are gzip-compressed at build time and decompressed lazily on first access. SDK and driver entries share one lookup space — [`lookup`] tries SDK first then driver, since the namespaces are essentially disjoint.

---

## How it works

The `build.rs` script handles data generation in two passes (one per dataset):

1. **Override env vars** — if set, points the build at a pre-generated JSON:
   - `BB_SPARSE_SDK_JSON` (or legacy `BB_SPARSE_JSON`) for the SDK dataset
   - `BB_SPARSE_DRIVER_JSON` for the driver dataset
2. **Workspace-root drop-in** — if `sdk-api.json` and/or `driver-docs.json` exists at the workspace root, uses it.
3. **Auto-generate** — runs the [sparse](https://github.com/cristeigabriela/sparse) Python tool against the relevant nested submodule:
   - Initializes `sparse/sdk-api` and/or `sparse/windows-driver-docs-ddi` if missing (~1GB clone each, first time only).
   - Prefers `uv run python sparse.py` (fast, hermetic — sparse ships a `pyproject.toml`); falls back to plain Python if `uv` isn't on PATH.
   - Caches each dataset under its own stamp file: bb-sparse only regenerates the modes whose submodule HEAD actually moved.

If a given mode can't be generated (no submodule, no Python/uv, etc.) bb-sparse embeds an empty placeholder for that mode only. The other mode still works.

### Setup

```powershell
.\update-submodules.ps1 sparse
```

This initializes both nested submodules and runs `uv sync` so subsequent `cargo build` invocations don't need to fetch dependencies.

### Opting out

To build without sparse data (faster builds, smaller binary), don't init the sparse submodule. The `build.rs` falls back to empty placeholders and `bb_sparse::is_available()` returns `false`. You can also opt out of just one mode by leaving its nested submodule un-initialized.

---

## Usage

```rust
use bb_sparse::{Entry, Metadata};

// Combined lookup: returns an Entry tagged with its source. Use `as_metadata()`
// to access shared fields without caring which dataset the entry came from.
if let Some(entry) = bb_sparse::lookup("CreateFileW") {
    let meta = entry.as_metadata();
    println!("DLL: {:?}", meta.dll_display());
    println!("Min client: {:?}", meta.min_client_str());

    if let Some(pm) = meta.params().get("dwShareMode") {
        println!("Directions: {:?}", pm.direction_strings());
        println!("Values: {:?}", pm.values);
    }

    // Driver-only fields are accessible only when the entry is from the
    // driver dataset.
    if let Some(drv) = entry.driver() {
        if let Some(irql) = &drv.irql {
            println!("IRQL: {} {}", irql.op.as_deref().unwrap_or("="), irql.level);
        }
        println!("KMDF: {:?}", drv.kmdf_ver_str());
    }
}

// Typed per-dataset lookups give back concrete types directly.
let sdk_only:    Option<&bb_sparse::SdkMetadata>    = bb_sparse::lookup_sdk("CreateFileW");
let driver_only: Option<&bb_sparse::DriverMetadata> = bb_sparse::lookup_driver("WdfCollectionAdd");

// Numeric IRQL comparisons (resolves PASSIVE_LEVEL etc. via your own
// `HashMap<String, u64>` of kernel-mode constants — bb-funcs builds this
// from a macro-preprocessed translation unit).
let filter = bb_sparse::irql::parse_constraint("<= DISPATCH_LEVEL")?;
let matches = driver_only
    .and_then(|d| d.irql.as_ref())
    .and_then(|c| c.matches(&filter, &irql_consts));

// Availability + counts.
if bb_sparse::is_available() {
    println!(
        "{} SDK + {} driver functions",
        bb_sparse::entry_count_sdk(),
        bb_sparse::entry_count_driver(),
    );
}
```

---

## Data schema

Shared fields across both datasets:

| Field | Type | Example |
| --- | --- | --- |
| `header` | string | `"fileapi.h"` |
| `dll` | string or array | `"Kernel32.dll"` |
| `lib` | string or array | `"Kernel32.lib"` |
| `min_client_version` | string | `"Windows XP [desktop apps only]"` |
| `min_server_version` | string | `"Windows Server 2003 [desktop apps only]"` |
| `metadata.api_location` | array | `["Kernel32.dll", "KernelBase.dll", ...]` |
| `metadata.api_name` | array | `["CreateFile", "CreateFileA", "CreateFileW"]` |
| `metadata.description` | string | `"Creates or opens a file or I/O device. ..."` |
| `params.<name>.directions` | array | `["in"]`, `["in", "optional"]` |
| `params.<name>.values` | object | `{"FILE_SHARE_READ": 1, ...}` |

Driver-only fields, exposed via `meta.driver()`:

| Field | Type | Example |
| --- | --- | --- |
| `include_header` | string | `"wdf.h"` |
| `target_type` | string | `"Universal"` |
| `construct_type` | string | `"function"`, `"macro"`, `"method"` |
| `kmdf_ver` | string | `"1.0"`, `"1.15"` |
| `umdf_ver` | string | `"2.0"` |
| `tech_root` | string | `"storage"`, `"audio"` |
| `irql` | `{level, op}` or null | `{"level": "PASSIVE_LEVEL", "op": "<="}` |
| `irql_raw` | string | original frontmatter text for unparseable entries |

All fields are nullable — sparse's JSON schema is permissive — and the types use `serde_json::Value` internally with typed accessor methods.
