//! Shared utilities for bb.
//!
//! This crate provides common utilities used across the bb workspace.

/* ──────────────────────────────── Utilities ─────────────────────────────── */

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
}
