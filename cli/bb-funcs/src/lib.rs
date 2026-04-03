//! Shared library for the `bb-funcs` CLI and future TUI.
//!
//! Provides [`FuncFilter`] (pre-parse and post-parse filtering, sorting,
//! SQL `WHERE` evaluation via `bb-sql`), [`collect_funcs_filtered`] (returns
//! `Result` — propagates WHERE parse errors), and the [`enriched`] module
//! for sparse metadata integration.

pub mod enriched;
pub mod where_filter;

use std::str::FromStr;

use bb_clang::Function;
use bb_shared::glob_match;
use clang::{Entity, EntityKind, TranslationUnit};

/* ────────────────────── Parse, iter, collect, filter ────────────────────── */

#[must_use]
pub fn collect_funcs<'a>(tu: &'a TranslationUnit<'a>) -> Vec<Function<'a>> {
    iter_funcs(tu)
        .filter_map(|e| Function::try_from(e).ok())
        .collect()
}

pub fn collect_funcs_filtered<'a>(
    tu: &'a TranslationUnit<'a>,
    filter: &FuncFilter,
) -> Result<Vec<Function<'a>>, String> {
    let funcs: Vec<Function<'a>> = iter_funcs(tu)
        .filter(|e| filter.matches(e))
        .filter_map(|e| Function::try_from(e).ok())
        .collect();

    filter.post_filter(funcs)
}

/// Iterate over function declarations in a [`TranslationUnit`].
pub fn iter_funcs<'a>(tu: &'a TranslationUnit<'a>) -> impl Iterator<Item = Entity<'a>> {
    tu.get_entity()
        .get_children()
        .into_iter()
        .filter(|e| matches!(e.get_kind(), EntityKind::FunctionDecl))
}

/* ──────────────────── Comma-separated glob splitting ───────────────────── */

/// Split a string by commas, respecting `\,` as a literal comma escape.
fn split_escaped_commas(s: &str) -> Vec<String> {
    let mut result = Vec::new();
    let mut current = String::new();
    let mut chars = s.chars().peekable();

    while let Some(c) = chars.next() {
        if c == '\\' && chars.peek() == Some(&',') {
            current.push(chars.next().unwrap());
        } else if c == ',' {
            result.push(current);
            current = String::new();
        } else {
            current.push(c);
        }
    }
    result.push(current);
    result
}

/// A parsed param-type pattern: a sequence of segments separated by `...`.
///
/// - `HANDLE,_,DWORD` → one fixed segment anchored at position 0.
/// - `...,HANDLE,DWORD` → one segment, may start at any position.
/// - `HANDLE,...,DWORD` → two segments with a gap of any size between them.
/// - `...,HANDLE,...,DWORD,...` → two segments, floating start and open tail.
/// - `_` as a standalone slot → matches any single param type.
#[derive(Debug, Clone)]
struct ParamTypePattern {
    /// Whether `...` appears before the first segment.
    anchored_start: bool,
    /// Whether `...` appears after the last segment.
    open_tail: bool,
    /// Contiguous runs of per-position globs, separated by `...` in the input.
    /// Empty string in a slot means "any" (from `_` or empty).
    segments: Vec<Vec<String>>,
}

fn parse_param_type_pattern(raw: &str) -> ParamTypePattern {
    // Split the raw string by the `...` delimiter (which itself is comma-separated).
    let parts: Vec<&str> = raw.split("...").collect();

    let anchored_start = !parts.first().is_some_and(|s| s.is_empty());
    let open_tail = parts.last().is_some_and(|s| s.is_empty());

    let segments: Vec<Vec<String>> = parts
        .iter()
        .map(|part| {
            let trimmed = part.trim_matches(',');
            if trimmed.is_empty() {
                return Vec::new();
            }
            split_escaped_commas(trimmed)
                .into_iter()
                .map(|s| if s == "_" { String::new() } else { s })
                .collect()
        })
        .filter(|seg| !seg.is_empty())
        .collect();

    ParamTypePattern {
        anchored_start,
        open_tail,
        segments,
    }
}

/// Recursively match pattern segments against a parameter list.
///
/// - `seg_idx`: current segment index.
/// - `from`: earliest param position this segment can start at.
/// - `anchored_start`: if true, the first segment must start at position 0.
/// - `open_tail`: if true, unmatched trailing params are allowed.
fn match_segments(
    segments: &[Vec<String>],
    seg_idx: usize,
    from: usize,
    params_len: usize,
    anchored_start: bool,
    open_tail: bool,
    seg_matches: &dyn Fn(&[String], usize) -> bool,
) -> bool {
    // All segments matched — check if trailing params are allowed.
    if seg_idx >= segments.len() {
        return open_tail || from == params_len;
    }

    let seg = &segments[seg_idx];
    if from + seg.len() > params_len {
        return false;
    }

    // First segment respects anchored_start; subsequent segments float.
    let can_float = seg_idx > 0 || !anchored_start;
    let max_start = if can_float {
        params_len - seg.len()
    } else {
        from
    };

    for start in from..=max_start {
        if seg_matches(seg, start)
            && match_segments(
                segments,
                seg_idx + 1,
                start + seg.len(),
                params_len,
                anchored_start,
                open_tail,
                seg_matches,
            )
        {
            return true;
        }
    }
    false
}

/* ──────────────────────── Parameter count filter ───────────────────────── */

/// Filter by parameter count: exact value or a range.
///
/// Accepted formats: `3` (exact), `3..` (min), `..7` (max), `3..7` (range).
#[derive(Debug, Clone)]
pub enum ParamCountFilter {
    Exact(usize),
    Range { min: usize, max: Option<usize> },
}

impl ParamCountFilter {
    #[must_use]
    pub fn contains(&self, count: usize) -> bool {
        match self {
            Self::Exact(n) => count == *n,
            Self::Range { min, max } => count >= *min && max.is_none_or(|m| count <= m),
        }
    }
}

impl FromStr for ParamCountFilter {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if let Some((left, right)) = s.split_once("..") {
            let min = if left.is_empty() {
                0
            } else {
                left.parse::<usize>()
                    .map_err(|_| format!("invalid min in range: '{left}'"))?
            };
            let max = if right.is_empty() {
                None
            } else {
                Some(
                    right
                        .parse::<usize>()
                        .map_err(|_| format!("invalid max in range: '{right}'"))?,
                )
            };
            Ok(Self::Range { min, max })
        } else {
            let n = s
                .parse::<usize>()
                .map_err(|_| format!("invalid parameter count: '{s}'"))?;
            Ok(Self::Exact(n))
        }
    }
}

/* ─────────────────────────── Sort key ──────────────────────────────────── */

/// Sort key for function results.
#[derive(Debug, Clone, PartialEq, Eq, clap::ValueEnum)]
pub enum FuncSort {
    /// Sort by number of parameters.
    Params,
    /// Sort by function name (alphabetical).
    Name,
    /// Sort by total bytes of stack-passed parameters (not including
    /// local variables or return address — only the caller-pushed args).
    StackSize,
    /// Sort by the size of the largest individual stack parameter.
    MaxStackParam,
}

/// Sort direction.
#[derive(Debug, Clone, Default, PartialEq, Eq, clap::ValueEnum)]
pub enum SortDir {
    /// Ascending (smallest first).
    #[default]
    Asc,
    /// Descending (largest first).
    Desc,
}

/* ────────────────────────────────── Match ───────────────────────────────── */

pub struct FuncFilter {
    // Pre-parse filters (applied on Entity, before Function construction).
    pub name_pattern: Option<String>,
    pub header_filter: Option<String>,
    pub case_sensitive: bool,

    // Post-parse filters (applied on constructed Function).
    pub dllimport_only: bool,
    pub param_count: Option<ParamCountFilter>,
    pub param_type_pattern: Option<String>,
    pub return_type_pattern: Option<String>,
    pub has_body: Option<bool>,

    // Sort (applied after all filters).
    pub sort: Option<FuncSort>,
    pub sort_dir: SortDir,

    // SQL `WHERE` clause (applied after all other filters).
    pub where_clause: Option<String>,

    // Limit (applied last, after sort).
    pub first: Option<usize>,
}

impl FuncFilter {
    /* ──────────────── Pre-parse (Entity-level) matching ─────────────────── */

    #[must_use]
    pub fn matches(&self, entity: &Entity) -> bool {
        self.matches_name(entity) && self.matches_header(entity)
    }

    #[must_use]
    fn matches_name(&self, entity: &Entity) -> bool {
        match (&self.name_pattern, entity.get_name()) {
            (Some(pattern), Some(name)) => glob_match(&name, pattern, self.case_sensitive),
            (Some(_), None) => false,
            (None, _) => true,
        }
    }

    #[must_use]
    fn matches_header(&self, entity: &Entity) -> bool {
        self.header_filter
            .as_ref()
            .is_none_or(|f| bb_clang::entity_in_header(entity, f))
    }

    /* ──────────────── Post-parse (Function-level) filtering ────────────── */

    /// Apply all post-parse filters and sorting to collected functions.
    ///
    /// Returns `Err` if the `WHERE` clause is present but fails to parse.
    pub fn post_filter<'a>(&self, funcs: Vec<Function<'a>>) -> Result<Vec<Function<'a>>, String> {
        let mut result: Vec<Function<'a>> = funcs
            .into_iter()
            .filter(|f| !self.dllimport_only || f.is_dllimport())
            .filter(|f| self.matches_param_count(f))
            .filter(|f| self.matches_param_type(f))
            .filter(|f| self.matches_return_type(f))
            .filter(|f| self.matches_has_body(f))
            .collect();

        if let Some(ref sort) = self.sort {
            match sort {
                FuncSort::Params => result.sort_by_key(|f| f.get_params().len()),
                FuncSort::Name => result.sort_by(|a, b| a.get_name().cmp(b.get_name())),
                FuncSort::StackSize => result.sort_by_key(|f| Self::stack_param_bytes(f)),
                FuncSort::MaxStackParam => result.sort_by_key(|f| Self::max_stack_param_size(f)),
            }
            if matches!(self.sort_dir, SortDir::Desc) {
                result.reverse();
            }
        }

        // Apply SQL `WHERE` clause.
        if let Some(ref clause) = self.where_clause {
            let expr = where_filter::parse_where(clause)?;
            result.retain(|f| where_filter::eval_where(&expr, f));
        }

        // Apply `--first` limit.
        if let Some(n) = self.first {
            result.truncate(n);
        }

        Ok(result)
    }

    /// Total bytes of stack-passed parameters.
    fn stack_param_bytes(f: &Function) -> usize {
        f.get_params()
            .iter()
            .filter(|p| p.is_stack())
            .map(bb_clang::Param::size)
            .sum()
    }

    /// Size of the largest individual stack-passed parameter.
    fn max_stack_param_size(f: &Function) -> usize {
        f.get_params()
            .iter()
            .filter(|p| p.is_stack())
            .map(bb_clang::Param::size)
            .max()
            .unwrap_or(0)
    }

    fn matches_param_count(&self, f: &Function) -> bool {
        self.param_count
            .as_ref()
            .is_none_or(|pc| pc.contains(f.get_params().len()))
    }

    /// Parameter type matching with segments separated by `...`.
    ///
    /// - `"HANDLE,_,DWORD"` — fixed: param 0=HANDLE, 1=any, 2=DWORD.
    /// - `"...,HANDLE,DWORD"` — HANDLE,DWORD at any consecutive positions.
    /// - `"HANDLE,...,DWORD"` — HANDLE somewhere, then DWORD at a later position.
    /// - `"...,HANDLE,...,DWORD,..."` — both floating, gap between, open tail.
    /// - `"HANDLE,DWORD,..."` — HANDLE,DWORD at 0-1, any trailing params OK.
    /// - `_` in a slot matches any single type.
    fn matches_param_type(&self, f: &Function) -> bool {
        let Some(ref raw) = self.param_type_pattern else {
            return true;
        };

        let pat = parse_param_type_pattern(raw);
        let params = f.get_params();
        let case = self.case_sensitive;

        if pat.segments.is_empty() {
            return true;
        }

        let min_total: usize = pat.segments.iter().map(Vec::len).sum();
        if min_total > params.len() {
            return false;
        }

        // Match each slot against the parameter's *type name* only
        // (not the parameter name).
        let seg_matches = |seg: &[String], start: usize| -> bool {
            seg.iter().enumerate().all(|(j, slot)| {
                slot.is_empty()
                    || slot == "*"
                    || glob_match(params[start + j].get_type_name(), slot, case)
            })
        };

        match_segments(
            &pat.segments,
            0,
            0,
            params.len(),
            pat.anchored_start,
            pat.open_tail,
            &seg_matches,
        )
    }

    fn matches_return_type(&self, f: &Function) -> bool {
        let Some(ref pattern) = self.return_type_pattern else {
            return true;
        };
        glob_match(f.get_return_type_name(), pattern, self.case_sensitive)
    }

    fn matches_has_body(&self, f: &Function) -> bool {
        self.has_body.is_none_or(|b| f.has_body() == b)
    }
}

/* ─────────────────────────────── Tests ──────────────────────────────────── */

#[cfg(test)]
mod tests {
    use super::*;

    /* ──────────────── split_escaped_commas ──────────────────── */

    #[test]
    fn split_simple() {
        assert_eq!(split_escaped_commas("a,b,c"), vec!["a", "b", "c"]);
    }

    #[test]
    fn split_empty_slots() {
        assert_eq!(
            split_escaped_commas(",,,HANDLE"),
            vec!["", "", "", "HANDLE"]
        );
    }

    #[test]
    fn split_escaped_comma() {
        assert_eq!(split_escaped_commas(r"a\,b,c"), vec!["a,b", "c"]);
    }

    #[test]
    fn split_single() {
        assert_eq!(split_escaped_commas("HANDLE"), vec!["HANDLE"]);
    }

    #[test]
    fn split_all_empty() {
        assert_eq!(split_escaped_commas(",,"), vec!["", "", ""]);
    }

    #[test]
    fn split_trailing_escape() {
        // backslash not followed by comma is kept as-is
        assert_eq!(split_escaped_commas(r"a\b,c"), vec![r"a\b", "c"]);
    }

    /* ──────────────── ParamCountFilter parsing ─────────────── */

    #[test]
    fn param_count_exact() {
        let f = ParamCountFilter::from_str("3").unwrap();
        assert!(f.contains(3));
        assert!(!f.contains(2));
        assert!(!f.contains(4));
    }

    #[test]
    fn param_count_zero() {
        let f = ParamCountFilter::from_str("0").unwrap();
        assert!(f.contains(0));
        assert!(!f.contains(1));
    }

    #[test]
    fn param_count_open_range() {
        let f = ParamCountFilter::from_str("3..").unwrap();
        assert!(!f.contains(2));
        assert!(f.contains(3));
        assert!(f.contains(100));
    }

    #[test]
    fn param_count_bounded_range() {
        let f = ParamCountFilter::from_str("2..5").unwrap();
        assert!(!f.contains(1));
        assert!(f.contains(2));
        assert!(f.contains(5));
        assert!(!f.contains(6));
    }

    #[test]
    fn param_count_max_only() {
        let f = ParamCountFilter::from_str("..3").unwrap();
        assert!(f.contains(0));
        assert!(f.contains(3));
        assert!(!f.contains(4));
    }

    #[test]
    fn param_count_invalid() {
        assert!(ParamCountFilter::from_str("abc").is_err());
        assert!(ParamCountFilter::from_str("3..abc").is_err());
        assert!(ParamCountFilter::from_str("abc..3").is_err());
    }

    /* ──────────── parse_param_type_pattern ──────────────────── */

    #[test]
    fn pattern_fixed() {
        let p = parse_param_type_pattern("HANDLE,_,DWORD");
        assert!(p.anchored_start);
        assert!(!p.open_tail);
        assert_eq!(p.segments, vec![vec!["HANDLE", "", "DWORD"]]);
    }

    #[test]
    fn pattern_floating_start() {
        let p = parse_param_type_pattern("...,HANDLE,DWORD");
        assert!(!p.anchored_start);
        assert!(!p.open_tail);
        assert_eq!(p.segments, vec![vec!["HANDLE", "DWORD"]]);
    }

    #[test]
    fn pattern_open_tail() {
        let p = parse_param_type_pattern("HANDLE,DWORD,...");
        assert!(p.anchored_start);
        assert!(p.open_tail);
        assert_eq!(p.segments, vec![vec!["HANDLE", "DWORD"]]);
    }

    #[test]
    fn pattern_floating_and_open_tail() {
        let p = parse_param_type_pattern("...,HANDLE,...");
        assert!(!p.anchored_start);
        assert!(p.open_tail);
        assert_eq!(p.segments, vec![vec!["HANDLE"]]);
    }

    #[test]
    fn pattern_middle_gap() {
        let p = parse_param_type_pattern("HANDLE,...,DWORD");
        assert!(p.anchored_start);
        assert!(!p.open_tail);
        assert_eq!(p.segments, vec![vec!["HANDLE"], vec!["DWORD"]]);
    }

    #[test]
    fn pattern_all_three_ellipses() {
        let p = parse_param_type_pattern("...,HANDLE,...,DWORD,...");
        assert!(!p.anchored_start);
        assert!(p.open_tail);
        assert_eq!(p.segments, vec![vec!["HANDLE"], vec!["DWORD"]]);
    }

    #[test]
    fn pattern_single_no_ellipsis() {
        let p = parse_param_type_pattern("HANDLE");
        assert!(p.anchored_start);
        assert!(!p.open_tail);
        assert_eq!(p.segments, vec![vec!["HANDLE"]]);
    }

    #[test]
    fn pattern_underscore_wildcard() {
        let p = parse_param_type_pattern("HANDLE,_,DWORD");
        assert_eq!(p.segments, vec![vec!["HANDLE", "", "DWORD"]]);
    }

    #[test]
    fn pattern_just_ellipsis() {
        let p = parse_param_type_pattern("...");
        assert!(!p.anchored_start);
        assert!(p.open_tail);
        assert!(p.segments.is_empty());
    }

    #[test]
    fn pattern_multi_slot_segments() {
        let p = parse_param_type_pattern("A,B,...,C,D");
        assert!(p.anchored_start);
        assert!(!p.open_tail);
        assert_eq!(p.segments, vec![vec!["A", "B"], vec!["C", "D"]]);
    }
}
