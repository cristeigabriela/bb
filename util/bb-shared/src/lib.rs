//! Shared utilities for bb.
//!
//! This crate provides common utilities used across the bb workspace.

/* ─────────────────────────────── Glob match ─────────────────────────────── */

/// Match over string using Windows, `PowerShell` `-Like` syntax.
///
/// # Arguments
///
/// * `input`: Input string to match against.
/// * `pattern`: Pattern to match against input string. Only implements `*`.
/// * `case_sensitive`: Whether to ignore case.
///
/// Returns `true` when there is a match.
#[must_use]
pub fn glob_match(input: &str, pattern: &str, case_sensitive: bool) -> bool {
    let (input, pattern) = if case_sensitive {
        (input.to_string(), pattern.to_string())
    } else {
        (input.to_lowercase(), pattern.to_lowercase())
    };

    if !pattern.contains('*') {
        return input == pattern;
    }

    let parts: Vec<&str> = pattern.split('*').collect();
    let mut pos = 0;

    for (i, part) in parts.iter().enumerate() {
        if part.is_empty() {
            continue;
        }
        match input[pos..].find(part) {
            Some(found) if i == 0 && found != 0 => return false,
            Some(found) => pos += found + part.len(),
            None => return false,
        }
    }

    if !pattern.ends_with('*') {
        if let Some(last) = parts.last() {
            if !last.is_empty() && !input.ends_with(last) {
                return false;
            }
        }
    }

    true
}

/* ─────────────────────────────── Suggestions ────────────────────────────── */

/// Levenshtein edit distance between two strings (case-insensitive).
#[must_use]
pub fn levenshtein(a: &str, b: &str) -> usize {
    let a = a.to_lowercase();
    let b = b.to_lowercase();
    let a = a.as_bytes();
    let b = b.as_bytes();

    let mut prev: Vec<usize> = (0..=b.len()).collect();
    let mut curr = vec![0; b.len() + 1];

    for (i, &ca) in a.iter().enumerate() {
        curr[0] = i + 1;
        for (j, &cb) in b.iter().enumerate() {
            let cost = usize::from(ca != cb);
            curr[j + 1] = (prev[j] + cost).min(prev[j + 1] + 1).min(curr[j] + 1);
        }
        std::mem::swap(&mut prev, &mut curr);
    }

    prev[b.len()]
}

/// Find the closest matches for `input` among `candidates`.
///
/// Uses two strategies to catch both typos and incomplete names:
/// 1. **Edit distance** — candidates within `max(2, input.len() / 3)` edits.
/// 2. **Prefix match** — candidates that start with `input` (case-insensitive).
///
/// Returns up to `max_results` candidates, sorted by edit distance.
#[must_use]
pub fn suggest_closest<'a>(
    input: &str,
    candidates: impl Iterator<Item = &'a str>,
    max_results: usize,
) -> Vec<&'a str> {
    let threshold = 2.max(input.len() / 3);
    let input_lower = input.to_lowercase();

    let mut scored: Vec<(&str, usize)> = candidates
        .filter_map(|c| {
            let dist = levenshtein(input, c);

            // Typo match: within edit distance threshold.
            if dist > 0 && dist <= threshold {
                return Some((c, dist));
            }

            // Prefix match: input is a prefix of the candidate.
            if c.to_lowercase().starts_with(&input_lower) && c.len() != input.len() {
                return Some((c, dist));
            }

            None
        })
        .collect();

    scored.sort_by_key(|&(_, dist)| dist);
    scored.dedup_by_key(|&mut (name, _)| name);
    scored.truncate(max_results);
    scored.into_iter().map(|(name, _)| name).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_exact_match() {
        assert!(glob_match(
            "PROCESS_INFORMATION",
            "PROCESS_INFORMATION",
            true
        ));
        assert!(glob_match(
            "process_information",
            "PROCESS_INFORMATION",
            false
        ));
        assert!(!glob_match(
            "process_information",
            "PROCESS_INFORMATION",
            true
        ));
    }

    #[test]
    fn test_wildcard_end() {
        assert!(glob_match("PROCESS_INFORMATION", "PROCESS_*", false));
        assert!(glob_match("PROCESS_BASIC_INFORMATION", "PROCESS_*", false));
        assert!(!glob_match("XPROCESS_INFORMATION", "PROCESS_*", false));
    }

    #[test]
    fn test_wildcard_start() {
        assert!(glob_match("PROCESS_INFORMATION", "*INFORMATION", false));
        assert!(!glob_match("PROCESS_INFORMATION", "*INFO", false));
    }

    #[test]
    fn test_wildcard_middle() {
        assert!(glob_match("PROCESS_BASIC_INFORMATION", "PRO*INFO*", false));
        assert!(!glob_match("XPROCESS_INFORMATION", "PRO*INFO*", false));
    }

    #[test]
    fn test_wildcard_only() {
        assert!(glob_match("ANYTHING", "*", false));
        assert!(glob_match("", "*", false));
    }

    #[test]
    fn test_levenshtein() {
        assert_eq!(levenshtein("", ""), 0);
        assert_eq!(levenshtein("abc", "abc"), 0);
        assert_eq!(levenshtein("_PBE", "_PEB"), 2);
        assert_eq!(levenshtein("kitten", "sitting"), 3);
        // Case-insensitive.
        assert_eq!(levenshtein("_peb", "_PEB"), 0);
    }

    #[test]
    fn test_suggest_closest() {
        let names = ["_PEB", "_PEB32", "_PEB_LDR_DATA", "_TEB", "_CONTEXT"];
        let suggestions = suggest_closest("_PBE", names.iter().copied(), 3);
        assert_eq!(suggestions[0], "_PEB");
    }

    #[test]
    fn test_suggest_closest_prefix() {
        let names = [
            "INVALID_HANDLE_VALUE",
            "INVALID_ATOM",
            "INVALID_SOCKET",
            "UNRELATED",
        ];
        let suggestions = suggest_closest("INVALID_HANDLE", names.iter().copied(), 5);
        assert!(suggestions.contains(&"INVALID_HANDLE_VALUE"));
    }

    #[test]
    fn test_suggest_closest_no_match() {
        let names = ["_PEB", "_TEB", "_CONTEXT"];
        let suggestions = suggest_closest("XYZXYZXYZ", names.iter().copied(), 3);
        assert!(suggestions.is_empty());
    }
}
