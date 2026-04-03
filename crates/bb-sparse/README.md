# bb-sparse

> Embedded Windows API metadata from [sparse](https://github.com/cristeigabriela/sparse).

`bb-sparse` provides offline lookup of function-level documentation metadata extracted from Microsoft's sdk-api repository: library/DLL info, version requirements, parameter directions (SAL), and known constant values.

The JSON data is gzip-compressed at build time (~38MB raw → ~1.6MB compressed) and decompressed lazily on first access.

---

## How it works

The `build.rs` script handles data generation:

1. **`BB_SPARSE_JSON` env var** — if set, uses a pre-generated JSON file directly.
2. **`sparse.json` file** — if found next to the workspace root or crate, uses it.
3. **Auto-generate** — runs the [sparse](https://github.com/cristeigabriela/sparse) Python tool against the sdk-api submodule:
   - Initializes the `sparse/sdk-api` git submodule if needed (~1GB clone, first time only).
   - Runs `sparse.py` to parse MSDN markdown files (~8s).
   - Caches the result: subsequent builds are instant unless sdk-api is updated (tracked via git rev stamp).

If none of the above succeed (no Python, no submodule, etc.), an empty placeholder is embedded and `bb_sparse::is_available()` returns `false`.

### Setup

The recommended way:

```powershell
.\update-submodules.ps1 sparse
```

### Opting out

To build without sparse data (faster builds, smaller binary), simply don't init the sparse submodule. The build.rs gracefully degrades: if the submodule or Python is missing, it embeds an empty placeholder. `bb-funcs` falls back to the plain ABI detail view.

---

## Usage

```rust
// Look up metadata for a function.
if let Some(meta) = bb_sparse::lookup("CreateFileW") {
    println!("DLL: {:?}", meta.dll_display());
    println!("Min client: {:?}", meta.min_client_str());

    if let Some(pm) = meta.params.get("dwShareMode") {
        println!("Directions: {:?}", pm.direction_strings());
        println!("Values: {:?}", pm.values);
    }
}

// Check if data is available.
if bb_sparse::is_available() {
    println!("{} functions indexed", bb_sparse::entry_count());
}
```

---

## Data schema

Each function entry contains:

| Field | Type | Example |
| --- | --- | --- |
| `header` | string | `"fileapi.h"` |
| `dll` | string or array | `"Kernel32.dll"` |
| `lib` | string or array | `"Kernel32.lib"` |
| `min_client_version` | string | `"Windows XP [desktop apps only]"` |
| `min_server_version` | string | `"Windows Server 2003 [desktop apps only]"` |
| `metadata.api_location` | array | `["Kernel32.dll", "KernelBase.dll", ...]` |
| `metadata.api_name` | array | `["CreateFile", "CreateFileA", "CreateFileW"]` |
| `params.<name>.directions` | array | `["in"]`, `["in", "optional"]` |
| `params.<name>.values` | object | `{"FILE_SHARE_READ": 1, ...}` |

All fields are nullable — the sparse JSON schema is inconsistent across entries. The types use `serde_json::Value` internally and expose typed accessor methods.
