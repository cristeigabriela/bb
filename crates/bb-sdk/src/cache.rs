//! On-disk cache of parsed translation units.
//!
//! Persists `TranslationUnit`s to `<user-cache-dir>/bb/<sha256>.ast` via
//! libclang's `clang_saveTranslationUnit`. Subsequent invocations with the
//! same synthetic header + clang args reload via `TranslationUnit::from_ast`,
//! skipping the multi-second parse.
//!
//! The cache key hashes:
//! - the synthetic header text (so any header-set change invalidates),
//! - every clang command-line argument (target triple, -I paths, defines —
//!   so SDK install path, WDF/NetCx version, arch changes invalidate),
//! - the bb-sdk crate version (so bb releases invalidate),
//! - whether detailed preprocessing was requested (different ASTs).
//!
//! Set `BB_NO_CACHE=1` in the environment to bypass and always parse fresh.

use std::path::PathBuf;

use sha2::{Digest, Sha256};

/// Crate version baked in at compile time. Bumping bb-sdk invalidates
/// every existing cached AST automatically.
const BB_SDK_VERSION: &str = env!("CARGO_PKG_VERSION");

/// `true` if the user disabled the AST cache via `BB_NO_CACHE`.
#[must_use]
pub fn is_disabled() -> bool {
    std::env::var_os("BB_NO_CACHE").is_some()
}

/// Compute the on-disk cache path for a (synthetic header, clang args,
/// `detailed_preprocessing`) triple. Returns `None` if the cache directory
/// can't be resolved on this platform.
#[must_use]
pub fn cache_path(
    synthetic: &str,
    args: &[String],
    detailed_preprocessing: bool,
) -> Option<PathBuf> {
    let mut hasher = Sha256::new();
    hasher.update(BB_SDK_VERSION.as_bytes());
    hasher.update(b"\0");
    hasher.update([u8::from(detailed_preprocessing)]);
    for arg in args {
        hasher.update(arg.as_bytes());
        hasher.update(b"\0");
    }
    hasher.update(b"\0");
    hasher.update(synthetic.as_bytes());
    let hex = format!("{:x}", hasher.finalize());

    let dir = dirs::cache_dir()?.join("bb").join("ast");
    if let Err(e) = std::fs::create_dir_all(&dir) {
        // Cache dir uncreatable — fall back to no cache.
        eprintln!("bb-sdk: AST cache dir unavailable ({e}); not caching");
        return None;
    }
    Some(dir.join(format!("{hex}.ast")))
}
