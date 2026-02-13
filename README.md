<div align="center">

# bb

**Benowin Blanc** — Windows through a detective's lens.

A set of command-line tools that parse **Windows SDK** and **PHNT** headers via libclang
and let you inspect what's actually in them: struct layouts, field offsets,
enum values, constants, `#define` macros — the works.

Think of it as `dt` from WinDbg, but you don't need a debugger running,
and it works against any SDK version, architecture, or PHNT release you throw at it.

</div>

---

<br>

<table>
<tr>
<td width="50%">
<h3 align="center">bb-types</h3>
<p align="center"><sub>Struct and class layouts, right in your terminal</sub></p>

<!-- TODO: Add a screenshot of bb-types CLI output (e.g. bb-types --struct PROCESS_BASIC_INFORMATION) -->
<!-- Save to: media/bb-types-output.png -->
<p align="center"><img src="./media/bb-types-output.png" alt="bb-types CLI output showing a struct layout with offsets, sizes, field names, and types" width="95%"></p>

</td>
<td width="50%">
<h3 align="center">bb-consts</h3>
<p align="center"><sub>Constants, enums, and macro definitions</sub></p>

<!-- TODO: Add a screenshot of bb-consts CLI output (e.g. bb-consts --name GENERIC_* or bb-consts --enum FILE_INFORMATION_CLASS) -->
<!-- Save to: media/bb-consts-output.png -->
<p align="center"><img src="./media/bb-consts-output.png" alt="bb-consts CLI output showing enum values and constants with their numeric values" width="95%"></p>

</td>
</tr>
</table>

<table>
<tr>
<td width="50%">
<h3 align="center">bb-types-tui</h3>
<p align="center"><sub>Interactive struct browser</sub></p>

<!-- TODO: Add a screenshot of the bb-types-tui application in action -->
<!-- Save to: media/bb-types-tui.png -->
<p align="center"><img src="./media/bb-types-tui.png" alt="bb-types-tui showing an interactive TUI with file tree, search bar, and struct display" width="95%"></p>

</td>
<td width="50%">
<h3 align="center">bb-consts-tui</h3>
<p align="center"><sub>Interactive constant browser</sub></p>

<!-- TODO: Add a screenshot of the bb-consts-tui application in action -->
<!-- Save to: media/bb-consts-tui.png -->
<p align="center"><img src="./media/bb-consts-tui.png" alt="bb-consts-tui showing an interactive TUI with file tree, search bar, and constant display" width="95%"></p>

</td>
</tr>
</table>

<br>

---

## What is this?

Windows ships with thousands of C/C++ headers (the **Windows SDK**) that define every struct, enum, constant, and macro the OS exposes. Separately, the community-maintained **PHNT** (Process Hacker NT headers) documents internal structures that Microsoft doesn't publish.

`bb` parses these headers with **libclang** and gives you fast, searchable, pretty-printed access to all of it **(hell, even TUIs!)** — no debugger, no IDE, no digging through `.h` files by hand.

<table>
<tr></tr>
<tr>
<td>

**You might want this if you...**

- Reverse-engineer Windows internals;
- Write kernel drivers or need to check struct layouts across architectures;
- Want a quick `dt`-style lookup without spinning up WinDbg;
- Need to export struct/constant definitions as JSON for your own tooling;
- Are just curious about what's inside those headers!

</td>
</tr>
</table>

---

## Quick start

### Building

You need a **Visual Studio Developer Command Prompt** (for SDK include paths), **Rust** (edition 2024), and **LLVM / libclang**.

```
cargo build --release
```

The binaries land in `target/release/`.

### First commands

**Inspect a struct layout:**

```
bb-types --struct PROCESS_BASIC_INFORMATION
```

**Recurse into nested types:**

```
bb-types --phnt --struct PEB --depth 1
```

**Search for constants by wildcard:**

```
bb-consts --name GENERIC_*
```

**Scope to a specific enum:**

```
bb-consts --enum FILE_INFORMATION_CLASS
```

**Use `Enum::Constant` syntax to search within enums:**

```
bb-consts --name "FILE_INFORMATION_CLASS::*Ea*"
```

**Target a different architecture from your host:**

```
bb-types --arch arm64 --struct CONTEXT
```

**Export as JSON for your own tooling:**

```
bb-types --struct PROCESS_BASIC_INFORMATION --json
```

---

## The tools

<table>
<tr></tr>
<tr>
<td width="50%" valign="top">

### CLI applications

|   | Crate | What it does |
| --- | --- | --- |
| | [`bb-types`](bb-types/) | Inspect struct and class layouts |
| | [`bb-consts`](bb-consts/) | Inspect constants, enums, and `#define` macros |

</td>
<td width="50%" valign="top">

### TUI applications

|   | Crate | What it does |
| --- | --- | --- |
| | [`bb-types-tui`](bb-types-tui/) | Interactive struct browser |
| | [`bb-consts-tui`](bb-consts-tui/) | Interactive constant browser |

</td>
</tr>
</table>

<table>
<tr></tr>
<tr>
<td width="33%" valign="top">

### Libraries

| Crate | What it does |
| --- | --- |
| [`bb-clang`](util/bb-clang/) | libclang abstractions for types and constants |
| [`bb-sdk`](util/bb-sdk/) | Windows SDK / PHNT header management |
| [`bb-cli`](util/bb-cli/) | Shared CLI argument definitions |
| [`bb-tui`](util/bb-tui/) | Shared TUI framework on [`ratatui`](https://ratatui.rs/) |
| [`bb-shared`](util/bb-shared/) | Small shared utilities |

</td>
</tr>
</table>

---

## Supported headers

<table>
<tr>
<td width="50%" valign="top">

### Windows SDK

Uses whatever version is available in your Developer Command Prompt environment.

Covers **user-mode** headers (`windows.h`, `winternl.h`, `dbghelp.h`, crypto, networking, shell, COM, etc.) and **kernel-mode** headers (`ntddk.h`, `wdm.h`, `ntifs.h`, `fltkernel.h`, etc.)

```
bb-types --winsdk --struct DRIVER_OBJECT
bb-types --mode kernel --struct EPROCESS
```

</td>
<td width="50%" valign="top">

### PHNT

The **Process Hacker NT headers**, embedded at compile time. Exposes internal NT structures and constants that the public SDK doesn't ship.

Supports version targeting from **Win2000** through **Win11 22H2**:

```
bb-types --phnt win11 --struct PEB
bb-consts --phnt --name "STATUS_*"
```

</td>
</tr>
</table>

---

## Architecture support

Both tools support cross-compilation via `--arch` -- inspect struct layouts for any target from any host:

| Flag | Target | Notes |
| --- | --- | --- |
| `amd64` | `x86_64-pc-windows-msvc` | Default |
| `x86` | `i686-pc-windows-msvc` | |
| `arm64` | `aarch64-pc-windows-msvc` | |
| `arm` | `thumbv7-pc-windows-msvc` | |

```
bb-types --arch arm64 --struct CONTEXT
```

---

## How it works

Neither tool reads header files off disk directly. Instead, `bb-sdk` builds a synthetic `#include` cascade at runtime (covering the relevant subset of SDK or PHNT headers) and hands it to **libclang** as an in-memory buffer. The parsed AST is then walked by `bb-clang` to extract typed representations of structs, fields, enums, and constants.

For macros specifically, `bb-consts` does a two-pass resolution: first pass evaluates simple literals and variables, second pass substitutes known constant names into unresolved macro token streams before re-evaluating. This handles things like `#define GENERIC_ALL (GENERIC_READ | GENERIC_WRITE | GENERIC_EXECUTE)`.

<table>
<tr></tr>
<tr>
<td>

```
                  ┌──────────┐
                  │  bb-sdk  │  Discovers SDK, builds synthetic headers
                  └────┬─────┘
                       │
                       ▼
                  ┌──────────┐
                  │ bb-clang │  Parses AST, extracts structured entities
                  └────┬─────┘
                       │
              ┌────────┴────────┐
             ▼                 ▼
        ┌──────────┐     ┌───────────┐
        │ bb-types │     │ bb-consts │      CLI frontends
        └──────────┘     └───────────┘
              │                 │
             ▼                 ▼
       ┌──────────────┐  ┌───────────────┐
       │ bb-types-tui │  │ bb-consts-tui │  TUI frontends
       └──────────────┘  └───────────────┘
```

</td>
</tr>
</table>