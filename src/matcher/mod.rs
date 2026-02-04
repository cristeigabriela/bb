//! Text pattern-matching module.

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
