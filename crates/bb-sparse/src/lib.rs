//! Embedded Windows API metadata from [sparse](https://github.com/cristeigabriela/sparse).
//!
//! Provides offline lookup of function-level documentation metadata extracted
//! from Microsoft's `sdk-api` repository (user-mode Win32 APIs) **and**
//! `windows-driver-docs-ddi` (KMDF/UMDF and kernel DDIs): library/DLL info,
//! version requirements, parameter directions, known constant values, plus
//! driver-specific fields like IRQL constraints and KMDF/UMDF version tags.
//!
//! Two JSON blobs are gzip-compressed at build time and decompressed lazily
//! on first access. The two datasets are exposed as two distinct types —
//! [`SdkMetadata`] and [`DriverMetadata`] — that share a common surface
//! through the [`Metadata`] trait. [`lookup`] returns an [`Entry`] enum that
//! tags which source the result came from; [`lookup_sdk`] and
//! [`lookup_driver`] hand back concrete types when you already know the
//! source.

use std::collections::HashMap;
use std::io::Read;
use std::sync::OnceLock;

use flate2::read::GzDecoder;
use serde::Deserialize;

pub mod irql;

pub use irql::IrqlConstraint;

/* ────────────────────────────────── Source ──────────────────────────────── */

/// Which sparse dataset an entry came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Source {
    /// MicrosoftDocs/sdk-api — user-mode Win32 APIs.
    Sdk,
    /// MicrosoftDocs/windows-driver-docs-ddi — kernel / WDF DDIs.
    Driver,
}

/* ─────────────────────────── Shared metadata ────────────────────────────── */

/// Fields shared by both sdk-api and windows-driver-docs-ddi frontmatter.
///
/// Kept private — public callers reach these fields through the [`Metadata`]
/// trait (accessor methods) or through the typed wrappers.
#[derive(Debug, Clone, Deserialize)]
struct Shared {
    /// Header file (e.g., `"fileapi.h"`).
    #[serde(default)]
    header: serde_json::Value,
    /// Link library (string or array of strings).
    #[serde(default)]
    lib: serde_json::Value,
    /// DLL name (string or array of strings).
    #[serde(default)]
    dll: serde_json::Value,
    /// Minimum Windows client version.
    #[serde(default)]
    min_client_version: serde_json::Value,
    /// Minimum Windows server version.
    #[serde(default)]
    min_server_version: serde_json::Value,
    /// Extended API metadata.
    #[serde(default)]
    metadata: Option<ApiMetadata>,
    /// Per-parameter metadata, keyed by parameter name.
    #[serde(default)]
    params: HashMap<String, ParamMetadata>,
}

/* ─────────────────────────── SdkMetadata ────────────────────────────────── */

/// Metadata for a single user-mode Win32 API function, from sdk-api.
#[derive(Debug, Clone, Deserialize)]
pub struct SdkMetadata {
    #[serde(flatten)]
    shared: Shared,
}

/* ─────────────────────────── DriverMetadata ─────────────────────────────── */

/// Metadata for a single kernel / WDF DDI, from windows-driver-docs-ddi.
///
/// Extends the shared MSDN frontmatter with driver-only fields: IRQL
/// constraints, KMDF/UMDF version requirements, target-type, tech.root,
/// and the include-header used by driver code.
#[derive(Debug, Clone, Deserialize)]
pub struct DriverMetadata {
    #[serde(flatten)]
    shared: Shared,

    /// `req.include-header` from the page frontmatter (e.g., `"wdf.h"`).
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

/* ────────────────────────── Shared accessor trait ───────────────────────── */

/// Accessors shared by every metadata flavor.
///
/// Implemented by both [`SdkMetadata`] and [`DriverMetadata`] so callers that
/// only care about fields common to both datasets (header, DLL, lib, min
/// versions, params) can write `&dyn Metadata` once.
pub trait Metadata {
    fn header_str(&self) -> Option<&str>;
    fn lib_display(&self) -> Option<String>;
    fn dll_display(&self) -> Option<String>;
    fn min_client_str(&self) -> Option<&str>;
    fn min_server_str(&self) -> Option<&str>;
    fn api_metadata(&self) -> Option<&ApiMetadata>;
    fn params(&self) -> &HashMap<String, ParamMetadata>;
}

macro_rules! impl_metadata_via_shared {
    ($t:ty) => {
        impl Metadata for $t {
            fn header_str(&self) -> Option<&str> {
                self.shared.header.as_str()
            }
            fn lib_display(&self) -> Option<String> {
                value_as_display(&self.shared.lib)
            }
            fn dll_display(&self) -> Option<String> {
                value_as_display(&self.shared.dll)
            }
            fn min_client_str(&self) -> Option<&str> {
                self.shared.min_client_version.as_str()
            }
            fn min_server_str(&self) -> Option<&str> {
                self.shared.min_server_version.as_str()
            }
            fn api_metadata(&self) -> Option<&ApiMetadata> {
                self.shared.metadata.as_ref()
            }
            fn params(&self) -> &HashMap<String, ParamMetadata> {
                &self.shared.params
            }
        }
    };
}

impl_metadata_via_shared!(SdkMetadata);
impl_metadata_via_shared!(DriverMetadata);

/* ─────────────────────────────── Entry ──────────────────────────────────── */

/// A metadata entry returned by [`lookup`], tagged with its source.
///
/// Use [`Entry::as_metadata`] to access the shared fields without caring
/// which source they came from. Use [`Entry::driver`] to reach the
/// driver-only fields when you specifically need them.
#[derive(Debug, Clone, Copy)]
pub enum Entry<'a> {
    Sdk(&'a SdkMetadata),
    Driver(&'a DriverMetadata),
}

impl<'a> Entry<'a> {
    /// Borrow this entry as a shared-trait reference.
    #[must_use]
    pub fn as_metadata(&self) -> &'a dyn Metadata {
        match *self {
            Self::Sdk(m) => m,
            Self::Driver(m) => m,
        }
    }

    /// Which dataset this entry came from.
    #[must_use]
    pub fn source(&self) -> Source {
        match self {
            Self::Sdk(_) => Source::Sdk,
            Self::Driver(_) => Source::Driver,
        }
    }

    /// Driver-only metadata, if and only if this is a driver entry.
    #[must_use]
    pub fn driver(&self) -> Option<&'a DriverMetadata> {
        match *self {
            Self::Driver(m) => Some(m),
            Self::Sdk(_) => None,
        }
    }

    /// SDK-only metadata, if and only if this is an SDK entry.
    #[must_use]
    pub fn sdk(&self) -> Option<&'a SdkMetadata> {
        match *self {
            Self::Sdk(m) => Some(m),
            Self::Driver(_) => None,
        }
    }
}

/* ─────────────────────────── ApiMetadata / params ───────────────────────── */

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

impl ApiMetadata {
    /// API location DLLs as strings.
    #[must_use]
    pub fn locations(&self) -> Vec<String> {
        values_as_strings(&self.api_location)
    }
    /// Function name variants as strings.
    #[must_use]
    pub fn names(&self) -> Vec<String> {
        values_as_strings(&self.api_name)
    }
    /// Short description, if present.
    #[must_use]
    pub fn description_str(&self) -> Option<&str> {
        self.description.as_str()
    }
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

impl ParamMetadata {
    /// Directions as strings.
    #[must_use]
    pub fn direction_strings(&self) -> Vec<String> {
        values_as_strings(&self.directions)
    }
}

/* ─────────────────────────── Value helpers ──────────────────────────────── */

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

fn values_as_strings(vals: &[serde_json::Value]) -> Vec<String> {
    vals.iter()
        .filter_map(|v| v.as_str().map(String::from))
        .collect()
}

/* ──────────────────────── Compressed data embedding ────────────────────── */

static COMPRESSED_SDK: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/sparse_sdk.json.gz"));
static COMPRESSED_DRIVER: &[u8] =
    include_bytes!(concat!(env!("OUT_DIR"), "/sparse_driver.json.gz"));

static LOOKUP_SDK: OnceLock<HashMap<String, SdkMetadata>> = OnceLock::new();
static LOOKUP_DRIVER: OnceLock<HashMap<String, DriverMetadata>> = OnceLock::new();

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

fn load<T: serde::de::DeserializeOwned>(data: &[u8]) -> HashMap<String, T> {
    decompress(data)
        .map(|json| serde_json::from_str(&json).expect("failed to parse sparse JSON"))
        .unwrap_or_default()
}

fn sdk_lookup() -> &'static HashMap<String, SdkMetadata> {
    LOOKUP_SDK.get_or_init(|| load(COMPRESSED_SDK))
}

fn driver_lookup() -> &'static HashMap<String, DriverMetadata> {
    LOOKUP_DRIVER.get_or_init(|| load(COMPRESSED_DRIVER))
}

/* ─────────────────────────── Public API ─────────────────────────────────── */

/// Look up metadata for a function by name.
///
/// Checks the SDK dataset first, then the driver dataset. The two
/// function-name spaces are largely disjoint (user-mode Win32 vs.
/// kernel/WDF DDIs) so collisions are rare; when one happens, SDK wins.
///
/// For mode-aware callers, prefer [`lookup_sdk`] / [`lookup_driver`] and
/// pick the right one based on context.
#[must_use]
pub fn lookup(name: &str) -> Option<Entry<'static>> {
    if let Some(m) = sdk_lookup().get(name) {
        return Some(Entry::Sdk(m));
    }
    driver_lookup().get(name).map(Entry::Driver)
}

/// Look up metadata for a function by name in the SDK dataset only.
#[must_use]
pub fn lookup_sdk(name: &str) -> Option<&'static SdkMetadata> {
    sdk_lookup().get(name)
}

/// Look up metadata for a function by name in the driver dataset only.
#[must_use]
pub fn lookup_driver(name: &str) -> Option<&'static DriverMetadata> {
    driver_lookup().get(name)
}

/// `true` if **any** sparse data was embedded at build time.
#[must_use]
pub fn is_available() -> bool {
    is_available_sdk() || is_available_driver()
}

/// `true` if the SDK dataset was embedded at build time.
#[must_use]
pub fn is_available_sdk() -> bool {
    !COMPRESSED_SDK.is_empty()
}

/// `true` if the driver dataset was embedded at build time.
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
