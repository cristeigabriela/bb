# bb-shared

Tiny shared utilities for the bb workspace. Zero external dependencies.

## `glob_match`

PowerShell `-Like` style glob matching with `*` wildcard. Used throughout the
workspace for filtering struct names, field names, enum names, and constants.

```rust
glob_match("PROCESS_BASIC_INFORMATION", "PROCESS_*", false)  // true
glob_match("PROCESS_BASIC_INFORMATION", "*INFO*", false)      // true
glob_match("PROCESS_BASIC_INFORMATION", "*BASIC*INFO*", false) // true
glob_match("anything", "*", false)                             // true
```

Third argument is `case_sensitive`. When `false`, both input and pattern are
lowercased before matching.
