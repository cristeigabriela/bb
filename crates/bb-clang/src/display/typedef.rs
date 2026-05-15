//! Display rendering for [`Typedef`](crate::Typedef) entries.

use colored::Colorize;

use crate::typedef::Typedef;

/// One-line summary for a typedef, used by `bb-types` to surface
/// pointer/primitive typedefs that don't resolve to a record (so they
/// have no `[aka …]` chip to ride on top of).
///
/// Example: `HANDLE  →  PVOID → void *   (pointer)  winnt.h:1234:5`.
#[must_use]
pub fn format_typedef_summary(t: &Typedef) -> String {
    let name = t.name.cyan().bold();
    let arrow_chain: String = t
        .chain
        .iter()
        .map(String::as_str)
        .collect::<Vec<_>>()
        .join(" → ");
    let kind = format!("({})", t.kind.label()).dimmed();
    let loc = t
        .location
        .as_ref()
        .map(|l| format!("  {}", l.to_string().dimmed()))
        .unwrap_or_default();
    format!("  {name}  →  {arrow_chain}   {kind}{loc}")
}
