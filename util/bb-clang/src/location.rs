//! Source location information shared across all bb-clang types.

use std::fmt;
use std::path::{Path, PathBuf};

use clang::Entity;
use serde::Serialize;

/* ────────────────────────────────── Types ───────────────────────────────── */

/// Source location information (file, line, column).
#[derive(Debug, Clone, Serialize)]
pub struct SourceLocation {
    /// Filename only (e.g. `"winnt.h"`).
    pub file: Option<String>,
    /// Full filesystem path to the source file.
    #[serde(skip)]
    pub full_path: Option<PathBuf>,
    pub line: u32,
    pub column: u32,
}

impl SourceLocation {
    /// Extract source location from a Clang [`Entity`].
    #[must_use]
    pub fn from_entity(entity: &Entity) -> Option<Self> {
        entity.get_location().map(|loc| {
            let file_loc = loc.get_file_location();
            let full_path = file_loc.file.map(|f| f.get_path());
            let file = full_path
                .as_ref()
                .and_then(|p| p.file_name().map(|n| n.to_string_lossy().into_owned()));
            Self {
                file,
                full_path,
                line: file_loc.line,
                column: file_loc.column,
            }
        })
    }

    /// Full filesystem path, if available.
    #[must_use]
    pub fn path(&self) -> Option<&Path> {
        self.full_path.as_deref()
    }
}

/* ──────────────────────────────── Displays ──────────────────────────────── */

/// Format as `"<file>:<line>:<col>"` (or `"<line>:<col>"` if no file).
impl fmt::Display for SourceLocation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.file {
            Some(file) => write!(f, "{}:{}:{}", file, self.line, self.column),
            None => write!(f, "{}:{}", self.line, self.column),
        }
    }
}
