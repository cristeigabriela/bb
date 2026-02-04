use bb::matcher::glob_match;

#[test]
fn test_exact_match() {
    assert!(glob_match(
        "PROCESS_INFORMATION",
        "PROCESS_INFORMATION",
        true
    ));
    assert!(!glob_match(
        "PROCESS_INFORMATION",
        "process_information",
        true
    ));
    assert!(glob_match(
        "PROCESS_INFORMATION",
        "process_information",
        false
    ));
}

#[test]
fn test_wildcard_end() {
    assert!(glob_match("PROCESS_INFORMATION", "PROCESS_*", true));
    assert!(!glob_match("XPROCESS_INFORMATION", "PROCESS_*", true));
}

#[test]
fn test_wildcard_start() {
    assert!(glob_match("PROCESS_INFORMATION", "*INFORMATION", true));
    assert!(glob_match("THREAD_INFORMATION", "*INFORMATION", true));
}

#[test]
fn test_wildcard_middle() {
    assert!(glob_match("PROCESS_INFORMATION", "PRO*INFO*", true));
}

#[test]
fn test_wildcard_only() {
    assert!(glob_match("ANYTHING", "*", true));
}
