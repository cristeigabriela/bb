//! Embedded Windows API metadata from [sparse](https://github.com/cristeigabriela/sparse).
//!
//! Provides offline lookup of function-level documentation metadata extracted
//! from Microsoft's sdk-api repository: library/DLL info, version requirements,
//! parameter directions, and known constant values.
//!
//! The JSON data is gzip-compressed at build time and decompressed lazily on
//! first access.

use std::collections::HashMap;
use std::io::Read;
use std::sync::OnceLock;

use flate2::read::GzDecoder;
use serde::Deserialize;

/* ────────────────────────────────── Types ───────────────────────────────── */

/// Metadata for a single Windows API function, sourced from MSDN documentation.
///
/// Fields use `serde_json::Value` where the sparse JSON schema is inconsistent
/// (some fields are strings in one entry, arrays or null in another).
#[derive(Debug, Clone, Deserialize)]
pub struct FuncMetadata {
    /// Header file (e.g., `"fileapi.h"`).
    #[serde(default)]
    pub header: serde_json::Value,
    /// Link library (e.g., `"Kernel32.lib"` or `["lib1", "lib2"]`).
    #[serde(default)]
    pub lib: serde_json::Value,
    /// DLL name (e.g., `"Kernel32.dll"` or `["dll1", "dll2"]`).
    #[serde(default)]
    pub dll: serde_json::Value,
    /// Minimum Windows client version.
    #[serde(default)]
    pub min_client_version: serde_json::Value,
    /// Minimum Windows server version.
    #[serde(default)]
    pub min_server_version: serde_json::Value,
    /// Extended API metadata.
    #[serde(default)]
    pub metadata: Option<ApiMetadata>,
    /// Per-parameter metadata, keyed by parameter name.
    #[serde(default)]
    pub params: HashMap<String, ParamMetadata>,
}

/// API-level metadata from the MSDN documentation frontmatter.
#[derive(Debug, Clone, Deserialize)]
pub struct ApiMetadata {
    /// Unique identifier (e.g., `"NF:fileapi.CreateFileW"`).
    #[serde(default, rename = "UID")]
    pub uid: serde_json::Value,
    /// Documentation title.
    #[serde(default)]
    pub title: serde_json::Value,
    /// API classification (e.g., `["DllExport"]`). May contain nulls.
    #[serde(default)]
    pub api_type: Vec<serde_json::Value>,
    /// All DLL locations where the function exists. May contain nulls.
    #[serde(default)]
    pub api_location: Vec<serde_json::Value>,
    /// Function name variants (e.g., `["CreateFile", "CreateFileA", "CreateFileW"]`).
    #[serde(default)]
    pub api_name: Vec<serde_json::Value>,
}

/// Per-parameter metadata from MSDN documentation.
#[derive(Debug, Clone, Deserialize)]
pub struct ParamMetadata {
    /// SAL-style directions (e.g., `["in"]`, `["in", "optional"]`, `["out"]`).
    #[serde(default)]
    pub directions: Vec<serde_json::Value>,
    /// Known constant values for this parameter (e.g., `{"FILE_SHARE_READ": 1}`).
    #[serde(default)]
    pub values: HashMap<String, serde_json::Value>,
}

/* ─────────────────────── Value extraction helpers ──────────────────────── */

/// Extract a display string from a `Value` that might be a string or an array of strings.
fn value_as_display(v: &serde_json::Value) -> Option<String> {
    match v {
        serde_json::Value::String(s) => Some(s.clone()),
        serde_json::Value::Array(arr) => {
            let strs: Vec<&str> = arr.iter().filter_map(|v| v.as_str()).collect();
            if strs.is_empty() {
                None
            } else {
                Some(strs.join(", "))
            }
        }
        _ => None,
    }
}

/// Extract a `Vec<String>` from a `Vec<Value>`, skipping nulls.
fn values_as_strings(vals: &[serde_json::Value]) -> Vec<String> {
    vals.iter()
        .filter_map(|v| v.as_str().map(String::from))
        .collect()
}

impl FuncMetadata {
    /// Get the header as a string, if available.
    #[must_use]
    pub fn header_str(&self) -> Option<&str> {
        self.header.as_str()
    }
    /// Get the lib as a display string (may be a single value or comma-joined array).
    #[must_use]
    pub fn lib_display(&self) -> Option<String> {
        value_as_display(&self.lib)
    }
    /// Get the DLL as a display string (may be a single value or comma-joined array).
    #[must_use]
    pub fn dll_display(&self) -> Option<String> {
        value_as_display(&self.dll)
    }
    /// Get the minimum client version, if available.
    #[must_use]
    pub fn min_client_str(&self) -> Option<&str> {
        self.min_client_version.as_str()
    }
    /// Get the minimum server version, if available.
    #[must_use]
    pub fn min_server_str(&self) -> Option<&str> {
        self.min_server_version.as_str()
    }
}

impl ApiMetadata {
    /// Get the API location DLLs as strings.
    #[must_use]
    pub fn locations(&self) -> Vec<String> {
        values_as_strings(&self.api_location)
    }
    /// Get the function name variants as strings.
    #[must_use]
    pub fn names(&self) -> Vec<String> {
        values_as_strings(&self.api_name)
    }
}

impl ParamMetadata {
    /// Get the directions as strings.
    #[must_use]
    pub fn direction_strings(&self) -> Vec<String> {
        values_as_strings(&self.directions)
    }
}

/* ──────────────────────── Compressed data embedding ────────────────────── */

/// The gzip-compressed sparse JSON, embedded at compile time.
/// If no data file was present at build time, this is an empty slice.
static COMPRESSED_DATA: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/sparse.json.gz"));

/// Lazily decompressed and deserialized lookup table.
static LOOKUP: OnceLock<HashMap<String, FuncMetadata>> = OnceLock::new();

fn load_lookup() -> HashMap<String, FuncMetadata> {
    if COMPRESSED_DATA.is_empty() {
        return HashMap::new();
    }

    let mut decoder = GzDecoder::new(COMPRESSED_DATA);
    let mut json_str = String::new();
    decoder
        .read_to_string(&mut json_str)
        .expect("failed to decompress sparse data");

    serde_json::from_str(&json_str).expect("failed to parse sparse JSON")
}

/* ─────────────────────────── Public API ─────────────────────────────────── */

/// Look up metadata for a function by name.
///
/// Returns `None` if the function is not in the sparse database, or if
/// no sparse data was embedded at build time.
#[must_use]
pub fn lookup(name: &str) -> Option<&'static FuncMetadata> {
    LOOKUP.get_or_init(load_lookup).get(name)
}

/// Returns `true` if sparse data was embedded at build time.
#[must_use]
pub fn is_available() -> bool {
    !COMPRESSED_DATA.is_empty()
}

/// Returns the number of functions in the sparse database.
#[must_use]
pub fn entry_count() -> usize {
    LOOKUP.get_or_init(load_lookup).len()
}
