# bb-consts

Constant, enum, and `#define` macro inspector for Windows headers.

Parses Windows SDK or PHNT headers through libclang and extracts numeric
constants from three sources: enum declarations, `const`/`constexpr` variables,
and preprocessor `#define` macros. Output is either a colorized aligned tree
or JSON.

Composite macros -- ones built from other named constants -- get their
components resolved and shown inline so you can see what went into the final
value.

## Usage

Run from a Visual Studio Developer Command Prompt.

```
bb-consts [OPTIONS]
```

### Searching constants

```
bb-consts --name GENERIC_*                 # wildcard on constant name
bb-consts --name STATUS_ACCESS_DENIED      # exact match
bb-consts --name "FILE_*" -H fileapi.h     # scoped to a header
```

### Searching enums

```
bb-consts --enum FILE_INFORMATION_CLASS    # all values in an enum
bb-consts --enum *PROCESS*                 # wildcard on enum name
```

### Scoped enum::constant syntax

Use `::` to search for constants within a specific enum:

```
bb-consts --name "FILE_INFORMATION_CLASS::*Ea*"
```

This filters to the `FILE_INFORMATION_CLASS` enum and shows only constants
matching `*Ea*`.

### PHNT

```
bb-consts --phnt --name "STATUS_*"
bb-consts --phnt --enum "PS_*"
```

### Cross-architecture and kernel mode

```
bb-consts --arch x86 --name PAGE_SIZE
bb-consts --mode kernel --enum *POOL*
```

### JSON output

```
bb-consts --enum FILE_INFORMATION_CLASS --json
```

Outputs `{ "enums": [...], "constants": [...] }`, omitting empty arrays.

## How macro resolution works

Many Windows `#define` macros reference other named constants:

```c
#define GENERIC_ALL (GENERIC_READ | GENERIC_WRITE | GENERIC_EXECUTE)
```

`bb-consts` does a two-pass evaluation:

1. First pass: evaluate simple numeric literals, enum values, and `const` variables
2. Second pass: for macros that failed the first pass, substitute known constant
   names with their resolved values and re-evaluate

This means composite macros display both their final value and a breakdown of
their components.

## All options

| Flag | Short | Description |
|---|---|---|
| `--winsdk [VERSION]` | | Use Windows SDK headers |
| `--phnt [VERSION]` | | Use PHNT headers |
| `--mode <user\|kernel>` | `-m` | Target mode (default: user) |
| `--arch <x86\|amd64\|arm\|arm64>` | `-a` | Architecture (default: amd64) |
| `--name <PATTERN>` | `-n` | Constant name pattern (`*` wildcard, `Enum::Const` syntax) |
| `--enum <PATTERN>` | `-e` | Enum name pattern (`*` wildcard) |
| `--filter <HEADER>` | `-H` | Filter by header filename |
| `--case-sensitive` | `-c` | Case-sensitive matching |
| `--json` | | Output as JSON |
| `--diagnostics` | | Show clang diagnostics |
