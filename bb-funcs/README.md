# bb-funcs

> CLI application for querying and exporting `Function` entities from **Windows SDK** / **PHNT** headers.

`bb-funcs` is a CLI application dedicated to querying, and exporting, information extracted from `Function` entities with `bb-clang`, from the respective SDK (**Windows SDK**/**PHNT**) of your choice.

Each function is parsed with full ABI awareness: the target architecture is detected from the translation unit, and every parameter is assigned its calling-convention location (register, stack offset, or indirect pointer).

---

## Arguments

### Specific to `bb-funcs`

| Flag | Description |
| --- | --- |
| `--name` / `-n` | Function name pattern (supports `*` wildcard) |
| `--filter` / `-H` | Filter your searches to a specific header |
| `--case-sensitive` / `-c` | Case-sensitive matching |
| `--exported` | Show only exported (dllimport) functions |
| `--params` / `-p` | Filter by parameter count (`3`, `0`, `3..7`, `3..`, `..5`) |
| `--param-type` | Parameter type pattern with `...` for gaps and `_` for any single type (see below) |
| `--return` / `-r` | Filter by return type (supports `*` wildcard, e.g., `BOOL`, `void`, `*STATUS*`) |
| `--has-body` | Show only functions with a body (definition, not just declaration) |
| `--sort` | Sort results (`params`, `name`) |
| `--detail` / `-d` | Force detailed ABI breakdown for all results (auto when single result) |
| `--json` | Output as JSON |

When a query matches exactly one function, the detailed ABI breakdown is shown automatically. Use `-d` to force detail mode for multiple results.

Filters are combined with AND logic — `--params 1 --return BOOL` shows functions with exactly 1 parameter that return BOOL.

#### `--param-type` syntax

Comma-separated positional slots. Use `...` for "any number of params" and `_` for "any single type":

| Pattern | Meaning |
| --- | --- |
| `HANDLE,_,DWORD` | Param 0 = HANDLE, 1 = any, 2 = DWORD (exactly 3 params) |
| `HANDLE,...` | HANDLE at position 0, any number of params after |
| `...,HANDLE,...` | HANDLE at any position |
| `HANDLE,...,DWORD` | HANDLE at 0, then DWORD at some later position (exactly) |
| `...,HANDLE,...,DWORD,...` | HANDLE then DWORD somewhere, any surrounding params |
| `HANDLE,DWORD,...,BOOL` | HANDLE,DWORD at 0-1, then BOOL as last param |

Type slots also support `*` glob wildcards (e.g., `*HANDLE*` matches `LPHANDLE`).

---

### Fuzzy suggestions

When an exact (non-wildcard) name doesn't match anything, `bb-funcs` suggests close matches:

```bash
bb-funcs --name CloseHandl
error: no functions matching 'CloseHandl'

  did you mean?

    CloseHandle
```

---

### Examples

```bash
# Functions with exactly 7 parameters in fileapi.h
bb-funcs --filter fileapi.h --params 7

# Functions that take a HANDLE parameter, sorted by param count
bb-funcs --param-type HANDLE --filter handleapi.h --sort params

# Functions returning BOOL with 5+ parameters
bb-funcs --return BOOL --params 5..

# Functions with 0 parameters (no-arg functions)
bb-funcs --filter errhandlingapi.h --params 0

# All exported functions in a header, sorted by name
bb-funcs --filter processthreadsapi.h --exported --sort name
```

---

### Shared with `bb-types` and `bb-consts`

<details>
<summary>Expand shared arguments</summary>

<br>

These arguments are managed by [`bb-cli`](./util/bb-cli/) and are shared across all CLI apps.

| Flag | Default | Description |
| --- | --- | --- |
| `--winsdk [VERSION]` | *(default SDK)* | Use Windows SDK headers. Optionally specify a version present in your environment |
| `--phnt [VERSION]` | -- | Use PHNT headers instead. Optionally specify a Windows version target |
| `--mode` / `-m` | `user` | `user` or `kernel` (defines `_KERNEL_MODE` for kernel) |
| `--arch` / `-a` | host | `x86` / `amd64` / `arm` / `arm64` -- supports cross-compilation |
| `--diagnostics` | off | Show Clang diagnostics. Useful for troubleshooting |

**PHNT version targets:** `win2k` `win-xp` `ws03` `vista` `win7` `win8` `win-blue` `threshold` `threshold2` `redstone` `redstone2` `redstone3` `redstone4` `redstone5` `19H1` `19H2` `20H1` `20H2` `21H1` `Win10-21H2` `Win10-22H2` `win11` `Win11-22H2`

</details>
