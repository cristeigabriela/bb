<div align="center">

# bb

**Benowin Blanc** — Windows through a detective's lens.

**(Name credits go to my beloved spouse. ꨄ︎)**

A set of command-line tools that parse **Windows SDK** and **PHNT** headers via libclang
and let you inspect what's actually in them: struct layouts, field offsets,
enum values, constants, `#define` macros, and **function declarations with full ABI breakdowns** — the works.

Think of it as `dt` + `x` from WinDbg, but you don't need a debugger running,
and it works against any SDK version, architecture, or PHNT release you throw at it.

</div>

---

<br>

<div align="center">

### [**Try bb viewer in your browser**](https://cristeigabriela.github.io/bb-viewer/index.html)

<sub><b><a href="https://github.com/cristeigabriela/bb-viewer">bb-viewer</a></b> — a vanilla TypeScript SPA built with Bun, powered by bb's JSON exports.</sub>

Browse **8,000+ functions**, **5,000+ types**, and **25,000+ constants** from the Windows SDK and PHNT headers across all architectures (amd64, x86, arm64, arm) — with ABI layouts, memory visualizations, C expressions, and an interactive type graph. No install required.

<table>
<tr>
<td width="50%"><a href="https://cristeigabriela.github.io/bb-viewer/index.html"><img src="./media/bb-viewer-home.png" alt="bb viewer dashboard" width="100%"></a></td>
<td width="50%"><a href="https://cristeigabriela.github.io/bb-viewer/index.html#/functions/CreateFileW"><img src="./media/bb-viewer-createfilew.png" alt="CreateFileW function detail" width="100%"></a></td>
</tr>
<tr>
<td align="center"><sub>Dashboard — stats, charts, top types</sub></td>
<td align="center"><sub>CreateFileW — ABI layout, metadata, known values</sub></td>
</tr>
</table>

</div>

<br>

<table>
<tr>
<td width="50%">
<h3 align="center">bb-types</h3>
<p align="center"><sub>Struct and class layouts, right in your terminal</sub></p>

<p align="center"><img src="./media/bb-types-output.png" alt="bb-types CLI output showing a struct layout with offsets, sizes, field names, and types" width="95%"></p>

</td>
<td width="50%">
<h3 align="center">bb-consts</h3>
<p align="center"><sub>Constants, enums, and macro definitions</sub></p>

<p align="center"><img src="./media/bb-consts-output.png" alt="bb-consts CLI output showing enum values and constants with their numeric values" width="95%"></p>

</td>
</tr>
</table>

<table>
<tr>
<td width="50%">
<h3 align="center">bb-types-tui</h3>
<p align="center"><sub>Interactive struct browser</sub></p>

<p align="center"><img src="./media/bb-types-tui.png" alt="bb-types-tui showing an interactive TUI with file tree, search bar, and struct display" width="95%"></p>

</td>
<td width="50%">
<h3 align="center">bb-consts-tui</h3>
<p align="center"><sub>Interactive constant browser</sub></p>

<p align="center"><img src="./media/bb-consts-tui.png" alt="bb-consts-tui showing an interactive TUI with file tree, search bar, and constant display" width="95%"></p>

</td>
</tr>
</table>

<br>

---

## What is this?

Windows ships with thousands of C/C++ headers (the **Windows SDK**) that define every struct, enum, constant, macro, and function the OS exposes. Separately, the community-maintained **PHNT** (Process Hacker NT headers) documents internal structures and syscalls that Microsoft doesn't publish.

`bb` parses these headers with **libclang** and gives you fast, searchable, pretty-printed access to all of it — struct layouts, constant values, **function ABIs with per-parameter register/stack locations**, and more **(hell, even TUIs!)** — no debugger, no IDE, no digging through `.h` files by hand.

<table>
<tr></tr>
<tr>
<td>

**You might want this if you...**

- Reverse-engineer Windows internals;
- Write kernel drivers or need to check struct layouts across architectures;
- Want a quick `dt`-style lookup without spinning up WinDbg;
- Need to see exactly which register or stack slot each function parameter lands in;
- Need to export struct/constant/function definitions as JSON or SQLite for your own tooling;
- Are just curious about what's inside those headers!

</td>
</tr>
</table>

---

## Quick start

### Building

On a Windows host, you will need the following:
- Visual Studio 2019/2022 **Build Tools**
- LLVM + Clang (**libclang.dll**) version **>=18.1**
- Rust **2024 edition**
- Python **>=3.9** (for submodule setup)

Afterwards, you may produce the binaries by invoking the following command:

```powershell
.\update-submodules.ps1   # init + generate submodule data
cargo build --release
```

The project uses two submodules, managed by `update-submodules.ps1`:

| Submodule | Purpose | Required for | Setup |
| --- | --- | --- | --- |
| **phnt** | PHNT NT header generation ([phnt-single-header](https://github.com/mrexodia/phnt-single-header)) | `--phnt` flag | `.\update-submodules.ps1 phnt` |
| **sparse** | MSDN API metadata ([sparse](https://github.com/cristeigabriela/sparse)) | Enriched function views | `.\update-submodules.ps1 sparse` |

You can update them individually or all at once (`.\update-submodules.ps1`). Both support env var overrides for custom data:

| Env var | What it does |
| --- | --- |
| `BB_PHNT_HEADER` | Use a custom `phnt.h` instead of generating from the submodule |
| `BB_SPARSE_JSON` | Use a pre-generated `sparse.json` instead of running the Python tool |

### First commands

**Inspect a struct layout:**

```bash
bb-types --struct _PEB
```

**Recurse into nested types:**

```bash
bb-types --phnt --struct _PEB --depth 2
```

**Search for constants by wildcard:**

```bash
bb-consts --name GENERIC_*
```

**Scope to a specific enum:**

```bash
bb-consts --enum _MINIDUMP_TYPE
```

**Use `Enum::Constant` syntax to search within enums:**

```bash
bb-consts --name "_MINIDUMP_TYPE::*"
```

**Target a different architecture from your host:**

```bash
bb-types --arch arm64 --struct _CONTEXT
```

**Inspect a function's ABI breakdown:**

```bash
bb-funcs --name CreateFileW
```

**List exported functions from a header:**

```bash
bb-funcs --name "Create*" --filter fileapi.h --exported
```

**Filter functions with SQL WHERE clauses:**

```bash
bb-funcs --where "params > 3 AND return_type = 'BOOL'"
bb-funcs --where "name LIKE '%File%' AND is_exported = true"
```

**Export as JSON or SQLite for your own tooling:**

```bash
bb-types --arch arm64 --struct _CONTEXT --json
bb-consts --name "PROCESS_*" --json
bb-funcs --name "Nt*" --phnt --json

# or export to SQLite
bb-funcs --name "Create*" --sqlite funcs.db
bb-types --struct "_*" --sqlite types.db
```

JSON mode in `bb-types` performs full nested type expansion, producing all matched types alongside their deduplicated `referenced_types` — regardless of the `--depth` flag. SQLite exports mirror the same level of detail as JSON.

**Typo? Both CLIs suggest close matches:**

```bash
bb-types --struct _PBE
error: no structs matching '_PBE'

  did you mean?

    _ABC
    _PSP
    _PEB
```

---

## The tools

<table>
<tr></tr>
<tr>
<td width="50%" valign="top">

### CLI applications

| Crate | What it does |
| --- | --- |
| [`bb-types`](cli/bb-types/) | Inspect struct and class layouts |
| [`bb-consts`](cli/bb-consts/) | Inspect constants, enums, and `#define` macros |
| [`bb-funcs`](cli/bb-funcs/) | Inspect function declarations with ABI parameter locations |

</td>
<td width="50%" valign="top">

### TUI applications

| Crate | What it does |
| --- | --- |
| [`bb-types-tui`](tui/bb-types-tui/) | Interactive struct browser |
| [`bb-consts-tui`](tui/bb-consts-tui/) | Interactive constant browser |

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
| [`bb-arch`](crates/bb-arch/) | Architecture definitions, register sets, and ABI location types |
| [`bb-clang`](crates/bb-clang/) | libclang abstractions for types, constants, and functions |
| [`bb-sparse`](crates/bb-sparse/) | Embedded Windows API metadata from MSDN (via [sparse](https://github.com/cristeigabriela/sparse)) |
| [`bb-sdk`](crates/bb-sdk/) | Windows SDK / PHNT header management |
| [`bb-sql`](crates/bb-sql/) | SQL WHERE evaluator + SQLite export |
| [`bb-cli`](crates/bb-cli/) | Shared CLI argument definitions |
| [`bb-tui`](crates/bb-tui/) | Shared TUI framework on [`ratatui`](https://ratatui.rs/) |
| [`bb-shared`](crates/bb-shared/) | Small shared utilities |

</td>
</tr>
</table>


### Web viewer

| | What it does |
| --- | --- |
| [**bb-viewer**](https://github.com/cristeigabriela/bb-viewer) | [Web explorer](https://cristeigabriela.github.io/bb-viewer/index.html) for bb's JSON output — functions, types, constants, type graph |

---

## Supported headers

<table>
<tr>
<td width="50%" valign="top">

### Windows SDK

Uses whatever version is available in your Developer Command Prompt environment.

Covers **user-mode** headers (`windows.h`, `winternl.h`, `dbghelp.h`, crypto, networking, shell, COM, etc.) and **kernel-mode** headers (`ntddk.h`, `wdm.h`, `ntifs.h`, `fltkernel.h`, etc.)

```
bb-types --mode kernel --winsdk --struct *DRIVER_OBJECT*
```

</td>
<td width="50%" valign="top">

### PHNT

The **Process Hacker NT headers**, embedded at compile time. Exposes internal NT structures and constants that the public SDK doesn't ship.

Supports version targeting from **Win2000** through **Win11 22H2**:

```
bb-types --phnt win11 --struct _PEB
bb-consts --phnt --name "STATUS_*"
```

</td>
</tr>
</table>

---

## Architecture support

All tools support cross-compilation via `--arch` — inspect layouts and ABIs for any target from any host:

| Flag | Target | Notes |
| --- | --- | --- |
| `amd64` | `x86_64-pc-windows-msvc` | Default |
| `x86` | `i686-pc-windows-msvc` | |
| `arm64` | `aarch64-pc-windows-msvc` | |
| `arm` | `thumbv7-pc-windows-msvc` | |

```
bb-types --arch arm64 --struct _CONTEXT
```

---

## How it works

The flow is described below:

<p align="center">
  <img src="./media/bb-diagram.png#gh-light-mode-only" alt="Diagram showing the bb crate dependency flow: bb-sdk feeds into bb-clang, which branches into bb-types, bb-funcs bb-consts (CLI frontends), each flowing down to bb-types-tui and bb-consts-tui (TUI frontends)" width="75%">
  <img src="./media/bb-diagram-dark-mode.png#gh-dark-mode-only" alt="Diagram showing the bb crate dependency flow: bb-sdk feeds into bb-clang, which branches into bb-types, bb-funcs bb-consts (CLI frontends), each flowing down to bb-types-tui and bb-consts-tui (TUI frontends)" width="75%">
</p>


We use `bb-sdk` to discover (or gather) the SDK environment, then we generate a SDK-specific "synthetic header" (also known as an `Unsaved`/`CXUnsavedFile` in the Clang-world) which will be passed through partial compilation with `libclang.dll` and in turn give us a `TranslationUnit`.

From the translation unit, we lift the AST entities into `bb-clang` serializable objects, and we use the information that we expose there to develop the tools.

For functions, `bb-clang` computes the full ABI layout: which register or stack slot each parameter occupies, per architecture and calling convention (cdecl, stdcall, fastcall). `bb-funcs` enriches this with MSDN metadata (DLL, lib, min Windows version) from [sparse](https://github.com/cristeigabriela/sparse) and cross-references known constant values for each parameter. SQL `WHERE` clause filtering is supported via `bb-sql`.

For macros specifically, `bb-consts` does a two-pass resolution: first pass evaluates simple literals and variables, second pass substitutes known constant names into unresolved macro token streams before re-evaluating. This handles things like `#define PROCESS_ALL_ACCESS (STANDARD_RIGHTS_REQUIRED | SYNCHRONIZE | 0xFFFF)`.