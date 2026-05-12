//! Embedded Windows API metadata from [sparse](https://github.com/cristeigabriela/sparse).
//!
//! Provides offline lookup of function-level documentation metadata extracted
//! from Microsoft's `sdk-api` repository (user-mode Win32 APIs) **and**
//! `windows-driver-docs-ddi` (KMDF/UMDF and kernel DDIs): library/DLL info,
//! version requirements, parameter directions, known constant values, plus
//! driver-specific fields like IRQL constraints and KMDF/UMDF version tags.
//!
//! Two JSON blobs are gzip-compressed at build time and decompressed lazily
//! on first access. SDK and driver entries live in the same lookup space —
//! [`lookup`] checks SDK first then driver (their function-name spaces are
//! largely disjoint). Use [`lookup_sdk`] / [`lookup_driver`] for an explicit
//! per-source query.
//!
//! Driver-only metadata (IRQL, KMDF/UMDF version, `target-type`, etc.) is
//! exposed via [`FuncMetadata::driver`], which returns `Some(&DriverMetadata)`
//! only for entries that came from the driver dataset. SDK entries always
//! return `None` here, even if the source JSON were to contain stray fields.
//!
//! See `bb-sparse/sparse` for the upstream parser and JSON schema.

use std::collections::HashMap;
use std::io::Read;
use std::sync::OnceLock;

use flate2::read::GzDecoder;
use serde::Deserialize;

/* ────────────────────────────────── Types ───────────────────────────────── */

/// Which sparse dataset a [`FuncMetadata`] entry came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Source {
    /// MicrosoftDocs/sdk-api — user-mode Win32 APIs.
    Sdk,
    /// MicrosoftDocs/windows-driver-docs-ddi — kernel / WDF DDIs.
    Driver,
}

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

    /// Internal slot for driver-only fields. Always `None` for SDK entries.
    /// Public access goes through [`FuncMetadata::driver`], which enforces
    /// the source check.
    #[serde(skip)]
    driver: Option<DriverMetadata>,

    /// Which dataset this entry came from. Populated at load time; never
    /// present in the source JSON.
    #[serde(skip)]
    pub source: Option<Source>,
}

/// Driver-mode-only metadata from `windows-driver-docs-ddi` frontmatter.
///
/// Only present on entries returned by [`lookup_driver`] (or by [`lookup`]
/// when the SDK dataset doesn't have a matching name).
#[derive(Debug, Clone, Default, Deserialize)]
pub struct DriverMetadata {
    /// `req.include-header` from the page frontmatter.
    #[serde(default)]
    pub include_header: serde_json::Value,
    /// `req.target-type` (e.g., `"Universal"`, `"Desktop"`).
    #[serde(default)]
    pub target_type: serde_json::Value,
    /// `req.construct-type` (e.g., `"function"`, `"macro"`, `"method"`).
    #[serde(default)]
    pub construct_type: serde_json::Value,
    /// `req.kmdf-ver` — minimum KMDF version string.
    #[serde(default)]
    pub kmdf_ver: serde_json::Value,
    /// `req.umdf-ver` — minimum UMDF version string.
    #[serde(default)]
    pub umdf_ver: serde_json::Value,
    /// `tech.root` — top-level driver tech category (e.g., `"storage"`).
    #[serde(default)]
    pub tech_root: serde_json::Value,
    /// Normalized IRQL constraint: `{ "level": "...", "op": "<=" }` or `null`.
    #[serde(default)]
    pub irql: Option<IrqlConstraint>,
    /// Raw IRQL string before normalization (for entries the grammar
    /// couldn't parse).
    #[serde(default)]
    pub irql_raw: serde_json::Value,
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
    /// Short description from the page intro.
    #[serde(default)]
    pub description: serde_json::Value,
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

/// Normalized IRQL constraint.
///
/// `level` is one of: `PASSIVE_LEVEL`, `APC_LEVEL`, `DISPATCH_LEVEL`,
/// `DPC_LEVEL`, `DEVICE_LEVEL`, `DIRQL`, `HIGH_LEVEL`, `IPI_LEVEL`,
/// or `ANY`. `op` is one of `<`, `<=`, `=`, `==`, `>=`, `>` or `None`
/// for an exact-or-implicit match.
#[derive(Debug, Clone, Deserialize)]
pub struct IrqlConstraint {
    pub level: String,
    #[serde(default)]
    pub op: Option<String>,
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
    /// Driver-only metadata, only present for entries from the driver dataset.
    /// Returns `None` for SDK entries.
    #[must_use]
    pub fn driver(&self) -> Option<&DriverMetadata> {
        self.driver.as_ref()
    }
}

impl DriverMetadata {
    /// `req.include-header`, if available.
    #[must_use]
    pub fn include_header_str(&self) -> Option<&str> {
        self.include_header.as_str()
    }
    /// `req.target-type`, if available.
    #[must_use]
    pub fn target_type_str(&self) -> Option<&str> {
        self.target_type.as_str()
    }
    /// `req.construct-type`, if available.
    #[must_use]
    pub fn construct_type_str(&self) -> Option<&str> {
        self.construct_type.as_str()
    }
    /// Minimum KMDF version, if available.
    #[must_use]
    pub fn kmdf_ver_str(&self) -> Option<&str> {
        self.kmdf_ver.as_str()
    }
    /// Minimum UMDF version, if available.
    #[must_use]
    pub fn umdf_ver_str(&self) -> Option<&str> {
        self.umdf_ver.as_str()
    }
    /// Tech root (e.g., `"storage"`), if available.
    #[must_use]
    pub fn tech_root_str(&self) -> Option<&str> {
        self.tech_root.as_str()
    }
    /// Raw IRQL string before normalization, if any.
    #[must_use]
    pub fn irql_raw_str(&self) -> Option<&str> {
        self.irql_raw.as_str()
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
    /// Get the short description, if present.
    #[must_use]
    pub fn description_str(&self) -> Option<&str> {
        self.description.as_str()
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

/// Gzip-compressed sparse JSON for the SDK dataset, embedded at compile time.
/// Empty when no data was available at build time.
static COMPRESSED_SDK: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/sparse_sdk.json.gz"));

/// Gzip-compressed sparse JSON for the driver dataset, embedded at compile time.
static COMPRESSED_DRIVER: &[u8] =
    include_bytes!(concat!(env!("OUT_DIR"), "/sparse_driver.json.gz"));

/// SDK-only lookup table.
static LOOKUP_SDK: OnceLock<HashMap<String, FuncMetadata>> = OnceLock::new();
/// Driver-only lookup table.
static LOOKUP_DRIVER: OnceLock<HashMap<String, FuncMetadata>> = OnceLock::new();

fn decompress(data: &[u8]) -> Option<String> {
    if data.is_empty() {
        return None;
    }
    let mut decoder = GzDecoder::new(data);
    let mut out = String::new();
    decoder
        .read_to_string(&mut out)
        .expect("failed to decompress sparse data");
    Some(out)
}

fn load(data: &[u8], source: Source) -> HashMap<String, FuncMetadata> {
    let Some(json) = decompress(data) else {
        return HashMap::new();
    };

    let mut map: HashMap<String, FuncMetadata> =
        serde_json::from_str(&json).expect("failed to parse sparse JSON");

    // Driver-only metadata is gated behind the source: deserialize the same
    // JSON a second time into DriverMetadata and attach to each entry. This
    // keeps the FuncMetadata struct uniform while ensuring driver fields are
    // *only* exposed for driver-source entries.
    if matches!(source, Source::Driver) {
        let raw: HashMap<String, DriverMetadata> =
            serde_json::from_str(&json).expect("failed to parse driver metadata");
        for (name, dm) in raw {
            if let Some(entry) = map.get_mut(&name) {
                entry.driver = Some(dm);
            }
        }
    }

    for v in map.values_mut() {
        v.source = Some(source);
    }
    map
}

fn sdk_lookup() -> &'static HashMap<String, FuncMetadata> {
    LOOKUP_SDK.get_or_init(|| load(COMPRESSED_SDK, Source::Sdk))
}

fn driver_lookup() -> &'static HashMap<String, FuncMetadata> {
    LOOKUP_DRIVER.get_or_init(|| load(COMPRESSED_DRIVER, Source::Driver))
}

/* ─────────────────────────── Public API ─────────────────────────────────── */

/// Look up metadata for a function by name.
///
/// Checks the SDK dataset first, then the driver dataset. The two
/// function-name spaces are largely disjoint (user-mode Win32 vs.
/// kernel/WDF DDIs) so collisions are rare; when one happens, SDK wins.
///
/// Returns `None` if the function isn't in either dataset, or if no
/// sparse data was embedded at build time.
#[must_use]
pub fn lookup(name: &str) -> Option<&'static FuncMetadata> {
    sdk_lookup()
        .get(name)
        .or_else(|| driver_lookup().get(name))
}

/// Look up metadata for a function by name in the SDK dataset only.
#[must_use]
pub fn lookup_sdk(name: &str) -> Option<&'static FuncMetadata> {
    sdk_lookup().get(name)
}

/// Look up metadata for a function by name in the driver dataset only.
#[must_use]
pub fn lookup_driver(name: &str) -> Option<&'static FuncMetadata> {
    driver_lookup().get(name)
}

/// Returns `true` if **any** sparse data was embedded at build time.
#[must_use]
pub fn is_available() -> bool {
    is_available_sdk() || is_available_driver()
}

/// Returns `true` if the SDK dataset was embedded at build time.
#[must_use]
pub fn is_available_sdk() -> bool {
    !COMPRESSED_SDK.is_empty()
}

/// Returns `true` if the driver dataset was embedded at build time.
#[must_use]
pub fn is_available_driver() -> bool {
    !COMPRESSED_DRIVER.is_empty()
}

/// Total number of functions across both datasets.
#[must_use]
pub fn entry_count() -> usize {
    entry_count_sdk() + entry_count_driver()
}

/// Number of functions in the SDK dataset.
#[must_use]
pub fn entry_count_sdk() -> usize {
    sdk_lookup().len()
}

/// Number of functions in the driver dataset.
#[must_use]
pub fn entry_count_driver() -> usize {
    driver_lookup().len()
}
