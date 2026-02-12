# bb

**Benowin Blanc** -- Windows through a detective's lens.

A set of command-line tools that parse Windows SDK and PHNT headers via libclang
and let you inspect what's actually in them: struct layouts, field offsets,
enum values, constants, `#define` macros, the works.

Think of it as `dt` from WinDbg but you don't need a debugger running, and it
works against any SDK version, architecture, or PHNT release you throw at it.

---

## What's in the box

| Crate | What it does |
|---|---|
| [`bb-types`](bb-types/) | CLI -- inspect struct and class layouts |
| [`bb-consts`](bb-consts/) | CLI -- inspect constants, enums, and `#define` macros |
| [`bb-clang`](util/bb-clang/) | Library -- libclang abstractions for types and constants |
| [`bb-sdk`](util/bb-sdk/) | Library -- Windows SDK / PHNT header management |
| [`bb-shared`](util/bb-shared/) | Library -- small shared utilities |

---

## bb-types

Parses Windows headers and prints struct layouts in a WinDbg `dt`-style tree.
Fields show their offset, size, name, and type -- colored and aligned.

```
bb-types --struct PROCESS_BASIC_INFORMATION
```

<!-- TODO: paste real output here -->

Recurse into nested types with `--depth`:

```
bb-types --phnt --struct PEB --depth 1
```

Filter by field name:

```
bb-types --struct PROCESS_BASIC_INFORMATION --field *Process*
```

JSON output for tooling:

```
bb-types --struct PROCESS_BASIC_INFORMATION --json
```

Use PHNT (Process Hacker NT headers) instead of the stock SDK -- the entire
header is embedded in the binary, no extra files needed:

```
bb-types --phnt --struct *OBJECT* --depth 1
```

Target a different architecture from your host:

```
bb-types --arch arm64 --struct CONTEXT
```

Kernel mode:

```
bb-types --mode kernel --struct DRIVER_OBJECT
```

---

## bb-consts

Same idea, but for constants. Parses enums, `const`/`constexpr` variables, and
`#define` macros. Composite macros (ones built from other named constants) get
their components resolved and displayed inline.

```
bb-consts --name GENERIC_*
```

<!-- TODO: paste real output here -->

Scope to an enum with `::` syntax:

```
bb-consts --name "FILE_INFORMATION_CLASS::*Ea*"
```

Or use `--enum` directly:

```
bb-consts --enum FILE_INFORMATION_CLASS
```

PHNT constants:

```
bb-consts --phnt --name "STATUS_*"
```

JSON:

```
bb-consts --enum FILE_INFORMATION_CLASS --json
```

---

## Supported headers

Both tools accept `--winsdk` (default) or `--phnt` as the header source.

**Windows SDK** -- uses whatever version is available in your Developer Command
Prompt environment. Covers user-mode (`windows.h`, `winternl.h`, `dbghelp.h`,
crypto, networking, shell, COM, etc.) and kernel-mode (`ntddk.h`, `wdm.h`,
`ntifs.h`, `fltkernel.h`, etc.) headers.

**PHNT** -- the Process Hacker NT headers, embedded at compile time. Exposes
internal NT structures and constants that the public SDK doesn't ship. Supports
version targeting from Win2000 through Win11 22H2:

```
bb-types --phnt win11 --struct *PROCESS*
bb-types --phnt vista --struct PEB
```

---

## Building

You need:

- Rust (edition 2024)
- LLVM / libclang (the `clang` crate needs it)
- A Visual Studio Developer Command Prompt (for SDK include paths)

```
cargo build --release
```

The binaries land in `target/release/bb-types.exe` and `target/release/bb-consts.exe`.

---

## Architecture support

Both tools support cross-compilation via `--arch`:

| Flag | Target triple | Notes |
|---|---|---|
| `amd64` (default) | `x86_64-pc-windows-msvc` | |
| `x86` | `i686-pc-windows-msvc` | |
| `arm64` | `aarch64-pc-windows-msvc` | |
| `arm` | `thumbv7-pc-windows-msvc` | |

This means you can inspect ARM64 struct layouts from an x64 machine -- clang
handles the cross-compilation defines and target triple internally.

---

## How it works

Neither tool reads header files off disk directly. Instead, `bb-sdk` builds a
synthetic `#include` cascade at runtime (covering the relevant subset of SDK or
PHNT headers) and hands it to libclang as an in-memory buffer. The parsed AST
is then walked by `bb-clang` to extract typed representations of structs,
fields, enums, and constants.

For macros specifically, `bb-consts` does a two-pass resolution: first pass
evaluates simple literals and variables, second pass substitutes known constant
names into unresolved macro token streams before re-evaluating. This handles
things like `#define GENERIC_ALL (GENERIC_READ | GENERIC_WRITE | GENERIC_EXECUTE)`.

---

## Project structure

```
bb/
  bb-types/          cli: struct inspection
  bb-consts/         cli: constant inspection
  util/
    bb-clang/        libclang type abstractions + display rendering
    bb-sdk/          SDK/PHNT header management + clang argument generation
    bb-shared/       glob matching, small utilities
```

## License

<!-- TODO -->
