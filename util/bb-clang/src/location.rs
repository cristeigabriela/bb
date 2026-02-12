//! Source location information shared across all bb-clang types.

use std::fmt;

use clang::Entity;
use serde::Serialize;

/* ────────────────────────────────── Types ───────────────────────────────── */

/// Source location information (file, line, column).
#[derive(Debug, Clone, Serialize)]
pub struct SourceLocation {
    pub file: Option<String>,
    pub line: u32,
    pub column: u32,
}

impl SourceLocation {
    /// Extract source location from a Clang [`Entity`].
    #[must_use]
    pub fn from_entity(entity: &Entity) -> Option<Self> {
        entity.get_location().map(|loc| {
            let file_loc = loc.get_file_location();
            Self {
                file: file_loc
                    .file
                    .map(|f| f.get_path())
                    .and_then(|p| p.file_name().map(|n| n.to_string_lossy().into_owned())),
                line: file_loc.line,
                column: file_loc.column,
            }
        })
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
