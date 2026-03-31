# bb-funcs

> CLI application for querying and exporting `Function` entities from **Windows SDK** / **PHNT** headers.

`bb-funcs` is a CLI application dedicated to querying, and exporting, information extracted from `Function` entities with `bb-clang`, from the respective SDK (**Windows SDK**/**PHNT**) of your choice.

Each function is parsed with full ABI awareness: the target architecture is detected from the translation unit, and every parameter is assigned its calling-convention location (register, stack offset, or indirect pointer).

When [`bb-sparse`](../../crates/bb-sparse/) data is embedded, the detail view is enriched with MSDN metadata: DLL/lib linkage, Windows version requirements, function variants, SAL parameter annotations, and known constant values with source locations cross-referenced from `bb-consts`.

---

## Arguments

### Filtering

| Flag | Description |
| --- | --- |
| `--name` / `-n` | Function name pattern (supports `*` wildcard) |
| `--filter` / `-H` | Filter by header file (e.g., `fileapi.h`) |
| `--case-sensitive` / `-c` | Case-sensitive matching |
| `--exported` | Show only exported (dllimport) functions |
| `--params` / `-p` | Filter by parameter count (`3`, `0`, `3..7`, `3..`, `..5`) |
| `--signature` | Parameter type signature pattern (see [syntax](#--signature-syntax)) |
| `--return` / `-r` | Filter by return type (supports `*` wildcard) |
| `--has-body` | Show only functions with a body |
| `--where` / `-w` | SQL WHERE clause for advanced filtering (see [syntax](#--where-syntax)) |

Filters are combined with AND logic. Simple flags filter first, then `--where` filters the remaining results.

### Sorting and limiting

| Flag | Description |
| --- | --- |
| `--sort` | Sort key: `params`, `name`, `stack-size`, `max-stack-param` |
| `--sort-dir` | Sort direction: `asc` (default), `desc` |
| `--first` / `-f` | Limit to first N results. `-f` alone = 1, `-f 5` = first 5 |

### Output

| Flag | Description |
| --- | --- |
| `--detail` / `-d` | Force detailed ABI breakdown for all results (auto for single result) |
| `--json` | Output as JSON with structured ABI, metadata, and constant values |

---

## Detail view

When a query matches exactly one function (or `-d` is used), `bb-funcs` shows a detailed breakdown:

- **C prototype** with SAL annotations (`/* in */`, `/* out, optional */`)
- **ABI table** — register/stack location for each parameter with index, kind, offset, size
- **Arguments** — per-parameter constant value tables with names, hex values, and source locations (cross-referenced from the SDK headers via `bb-consts`)
- **Info** — minimum Windows version, DLL/lib linkage, alternative DLL locations

The enriched view requires [`bb-sparse`](../../crates/bb-sparse/) data to be embedded at build time (run `.\update-submodules.ps1 sparse`). Without it, the plain ABI-only view is shown.

---

## `--signature` syntax

Comma-separated positional type slots. Matches parameter *types only* (not names). Use `...` for "any number of params" and `_` for "any single type":

| Pattern | Meaning |
| --- | --- |
| `HANDLE,_,DWORD` | Param 1 = HANDLE, 2 = any, 3 = DWORD (exactly 3 params) |
| `HANDLE,...` | HANDLE at position 1, any number of params after |
| `...,HANDLE,...` | HANDLE at any position |
| `HANDLE,...,DWORD` | HANDLE at 1, then DWORD at some later position (exactly) |
| `...,HANDLE,...,DWORD,...` | HANDLE then DWORD somewhere, any surrounding params |

Type slots also support `*` glob wildcards (e.g., `*HANDLE*` matches `LPHANDLE`).

---

## `--where` syntax

SQL WHERE clause for advanced filtering.

**Available columns:**

| Column | Type | Description |
| --- | --- | --- |
| `name` | string | Function name |
| `return_type` | string | Return type name |
| `params` | int | Number of parameters |
| `stack_size` | int | Total bytes of stack-passed params |
| `arch` | string | Architecture (`x64`, `x86`, `ARM64`, `ARM32`) |
| `calling_convention` | string | `cdecl`, `stdcall`, `fastcall` |
| `is_exported` | bool | Whether the function is exported (dllimport) |
| `has_body` | bool | Whether the function has a body |
| `header` | string | Source header file name |

**Supported operators:** `=`, `!=`, `<`, `>`, `<=`, `>=`, `AND`, `OR`, `NOT`, `LIKE`, `IN`, `BETWEEN`, `IS NULL`.

String comparisons are case-insensitive. `LIKE` uses SQL wildcards (`%` = any, `_` = single char).

---

## Fuzzy suggestions

When an exact (non-wildcard) name doesn't match anything, `bb-funcs` suggests close matches:

```bash
bb-funcs --name CloseHandl
error: no functions matching 'CloseHandl'

  did you mean?

    CloseHandle
```

---

## Examples

```bash
# Inspect a single function (auto-detail)
bb-funcs --name CreateFileW

# List functions in a header
bb-funcs --filter handleapi.h

# Functions with HANDLE as first param, sorted by param count
bb-funcs --signature HANDLE,... --filter handleapi.h --sort params

# Functions returning BOOL with 5+ parameters
bb-funcs --return BOOL --params 5..

# Exported functions sorted by name
bb-funcs --filter processthreadsapi.h --exported --sort name

# SQL-style filtering
bb-funcs --where "params > 3 AND return_type = 'BOOL'" --first 5

# Find functions with the largest stack parameters on x86
bb-funcs -a x86 --sort max-stack-param --sort-dir desc --first 3

# Combine flags + SQL + signature
bb-funcs --exported --signature "HANDLE,..." --where "params > 2" --first 10 --sort params

# Export as JSON
bb-funcs --name CreateFileW --json

# PHNT internal functions
bb-funcs --phnt --name "Nt*" --first 10
```

---

### Shared with `bb-types` and `bb-consts`

<details>
<summary>Expand shared arguments</summary>

<br>

These arguments are managed by [`bb-cli`](../../crates/bb-cli/) and are shared across all CLI apps.

| Flag | Default | Description |
| --- | --- | --- |
| `--winsdk [VERSION]` | *(default SDK)* | Use Windows SDK headers. Optionally specify a version present in your environment |
| `--phnt [VERSION]` | -- | Use PHNT headers instead. Optionally specify a Windows version target |
| `--mode` / `-m` | `user` | `user` or `kernel` (defines `_KERNEL_MODE` for kernel) |
| `--arch` / `-a` | host | `x86` / `amd64` / `arm` / `arm64` -- supports cross-compilation |
| `--diagnostics` | off | Show Clang diagnostics. Useful for troubleshooting |

**PHNT version targets:** `win2k` `win-xp` `ws03` `vista` `win7` `win8` `win-blue` `threshold` `threshold2` `redstone` `redstone2` `redstone3` `redstone4` `redstone5` `19H1` `19H2` `20H1` `20H2` `21H1` `Win10-21H2` `Win10-22H2` `win11` `Win11-22H2`

</details>
