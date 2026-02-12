# bb-types

Struct and class layout inspector for Windows headers.

Parses Windows SDK or PHNT headers through libclang and prints field-level
layout information: byte offsets, sizes, type names, and source locations.
Output is either a colorized WinDbg `dt`-style tree or JSON.

## Usage

Run from a Visual Studio Developer Command Prompt (needs SDK include paths).

```
bb-types [OPTIONS]
```

### Header source

By default, uses the Windows SDK version detected from your environment.

```
bb-types --struct PEB                          # Windows SDK (default)
bb-types --winsdk 10.0.22621.0 --struct PEB    # specific SDK version
bb-types --phnt --struct PEB                   # PHNT headers (Win11 default)
bb-types --phnt win7 --struct PEB              # PHNT targeting Win7
```

`--winsdk` and `--phnt` are mutually exclusive.

### Filtering

```
bb-types --struct PROCESS_*              # wildcard on struct name
bb-types --struct PEB --field *Ldr*      # filter fields within a struct
bb-types -H winternl.h --struct *        # only structs from a specific header
bb-types --struct PEB -c                 # case-sensitive matching
```

### Nested types

By default, only top-level fields are shown. Use `--depth` to expand:

```
bb-types --phnt --struct PEB --depth 2
```

This recursively expands fields that are structs/classes up to the given depth.
Cycle detection prevents infinite recursion on self-referential types.

### Cross-architecture

```
bb-types --arch x86 --struct CONTEXT
bb-types --arch arm64 --struct CONTEXT
```

### Kernel mode

```
bb-types --mode kernel --struct DRIVER_OBJECT
```

Requires WDK headers to be available.

### JSON output

```
bb-types --struct PEB --depth 1 --json
```

Outputs a JSON array of structs and their fields. Nested types (from `--depth`)
are flattened into the array and deduplicated.

## All options

| Flag | Short | Description |
|---|---|---|
| `--winsdk [VERSION]` | | Use Windows SDK headers |
| `--phnt [VERSION]` | | Use PHNT headers |
| `--mode <user\|kernel>` | `-m` | Target mode (default: user) |
| `--arch <x86\|amd64\|arm\|arm64>` | `-a` | Architecture (default: amd64) |
| `--struct <PATTERN>` | `-s` | Struct name pattern (`*` wildcard) |
| `--field <PATTERN>` | `-f` | Field name pattern (`*` wildcard) |
| `--filter <HEADER>` | `-H` | Filter by header filename |
| `--case-sensitive` | `-c` | Case-sensitive matching |
| `--depth <N>` | `-d` | Nested type expansion depth (default: 0) |
| `--json` | | Output as JSON |
| `--diagnostics` | | Show clang diagnostics |
