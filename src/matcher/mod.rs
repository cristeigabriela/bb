#[must_use] pub fn glob_match(name: &str, pattern: &str, case_sensitive: bool) -> bool {
    let (name, pattern) = if case_sensitive {
        (name.to_string(), pattern.to_string())
    } else {
        (name.to_lowercase(), pattern.to_lowercase())
    };

    if !pattern.contains('*') {
        return name == pattern;
    }

    let parts: Vec<&str> = pattern.split('*').collect();
    let mut pos = 0;

    for (i, part) in parts.iter().enumerate() {
        if part.is_empty() {
            continue;
        }
        match name[pos..].find(part) {
            Some(found) if i == 0 && found != 0 => return false,
            Some(found) => pos += found + part.len(),
            None => return false,
        }
    }

    if !pattern.ends_with('*') {
        if let Some(last) = parts.last() {
            if !last.is_empty() && !name.ends_with(last) {
                return false;
            }
        }
    }

    true
}
