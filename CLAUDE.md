# CLAUDE.md

Instructions for AI assistants working on this project.

## What is bb?

**Benowin Blanc** (bb) is a set of Windows SDK/PHNT header analysis tools. It parses C/C++ headers via libclang and provides struct layouts, constant values, and function declarations with full ABI awareness (register/stack parameter locations per architecture and calling convention).

The project runs on Windows only (requires MSVC build tools + libclang.dll).

## Workspace structure

```
bb/
├── crates/              # Libraries (never produce binaries)
│   ├── bb-arch          # Architecture enums, registers, ABI location types, JSON serialization
│   ├── bb-clang         # libclang abstractions: Struct, Enum, Constant, Function, Param
│   ├── bb-cli           # Shared CLI args (SharedArgs), suggestions, helpers
│   ├── bb-sdk           # Windows SDK + PHNT header config, parsing, architecture defines
│   ├── bb-shared         # Tiny utilities: glob_match, levenshtein, suggest_closest
│   ├── bb-sparse        # Embedded MSDN API metadata (compressed JSON from sparse submodule)
│   └── bb-tui           # Shared TUI framework (ratatui app loop, keybinds, layout)
├── cli/                 # CLI binaries (each has a lib + bin)
│   ├── bb-types         # Struct/class layout inspector
│   ├── bb-consts        # Constant/enum/macro inspector
│   └── bb-funcs         # Function inspector with ABI, sparse metadata, SQL filtering
├── tui/                 # TUI binaries
│   ├── bb-types-tui     # Interactive struct browser
│   └── bb-consts-tui    # Interactive constant browser
├── tests/               # Integration tests (bb-tests crate)
├── update-submodules.ps1
└── Cargo.toml           # Workspace root
```

## Dependency flow

```
bb-arch ← bb-clang ← bb-sdk ← bb-cli
                   ↑
            bb-shared  bb-sparse
                   ↓
         cli/{bb-types, bb-consts, bb-funcs}
                   ↓
         tui/{bb-types-tui, bb-consts-tui}
```

- `bb-clang` is the core parsing library. It must NOT depend on `bb-sparse`, `bb-sdk`, or any CLI/TUI crate.
- `bb-sparse` is a pure data crate. It must NOT depend on `bb-clang`.
- `bb-funcs` joins `bb-clang` + `bb-sparse` via its `enriched` module.

## Building

Requires MSVC build tools, LLVM/Clang (libclang.dll >= 18.1), Python >= 3.9, Rust 2024 edition.

```powershell
# On Windows, MSVC link.exe must be on PATH before Git's /usr/bin/link.exe
# If cargo fails to link, prepend MSVC to PATH or use a Developer Command Prompt

.\update-submodules.ps1   # init phnt + sparse submodules
cargo build --release
```

The `bb-sparse` build.rs auto-generates MSDN metadata from the sparse submodule (Python required). The `bb-sdk` build.rs auto-generates phnt.h from the phnt submodule. Both cache results and skip regeneration when the submodule hasn't changed.

### Environment variable overrides

| Variable | Purpose |
|----------|---------|
| `BB_PHNT_HEADER` | Use a custom phnt.h instead of generating from submodule |
| `BB_SPARSE_JSON` | Use a pre-generated sparse.json instead of running Python |

## Running tests

```powershell
# MSVC link.exe must be first on PATH
$env:PATH = "C:\Program Files\Microsoft Visual Studio\2022\Community\VC\Tools\MSVC\14.44.35207\bin\Hostx64\x64;$env:PATH"

cargo test --workspace --verbose
# or just integration tests:
cargo test --package bb-tests -- --test-threads=1
# or just bb-funcs unit tests:
cargo test --package bb-funcs
```

Tests use `serial_test` because libclang is not fully thread-safe. Integration tests parse real Windows SDK headers and assert on well-known types/functions (e.g., `_GUID`, `CreateFileW`, `CloseHandle`).

## Code style and conventions

- **Module doc comments** (`//!`) at the top of every file describing what the module does.
- **Section separators** using `/* ──── Section Name ──── */` Unicode box-drawing comments.
- **`#[must_use]`** on all public functions that return values.
- **`colored` crate** for CLI ANSI colors. `ratatui` for TUI rendering.
- **Semantic color roles**: cyan = type names, green = return types/sizes, yellow = ABI locations, white+bold = identifiers, dimmed = metadata/connectors.
- **Tree connectors**: `├─`, `╰─`, `│` (dimmed) for tree-style output.
- **Error types**: per-entity error enums in `crates/bb-clang/src/error.rs`. Use `thiserror` derive.
- **Serialization**: all bb-clang types derive `Serialize`. The `ToJson` trait in `json.rs` provides structured JSON output.
- **Filter pattern**: each CLI has a `FuncFilter`/`StructFilter`/`ConstFilter` struct with pre-parse (Entity-level) and post-parse (constructed type-level) filtering.
- **Stack offsets are callee-entry RSP/ESP-relative** (after CALL, before prologue). Not RBP-relative.

## Key architectural decisions

- **`Param::is_stack()`** and **`Param::size()`** are methods on the Param type for ABI queries.
- **`entity_in_header()`** in `bb-clang/location.rs` is the shared header-matching helper used by all filter structs.
- **`bb_cli::current_command_string()`** is used by all CLIs for JSON `"command"` fields.
- **`format_abi_param()`** in `bb-clang/display/function.rs` is the shared ABI row formatter.
- **`format_tags()`** returns `Vec<String>` so callers can extend before joining.
- **bb-funcs `enriched` module** owns the sparse metadata rendering. bb-clang stays generic.
- **bb-funcs `where_filter` module** evaluates SQL WHERE clauses via `sqlparser`.

## File naming in bb-clang

| File | Contents |
|------|----------|
| `function/abi.rs` | Calling conventions + ABI parameter assignment engine |
| `function/param.rs` | Param type with `is_stack()`, `size()` |
| `constant/tokens.rs` | Clang ↔ cexpr token conversion |
| `constant/macro_.rs` | Macro resolution with identifier substitution |
| `ext.rs` | Extension traits for clang types (`UnderlyingType`, `AnonymousType`, etc.) |
| `display/constant.rs` | Constant rendering |
| `display/function.rs` | Function rendering (list, detail, shared formatters) |

Files using trailing underscores (`struct_/`, `enum_/`, `macro_.rs`) follow the Rust convention for avoiding keyword conflicts.

## Submodules

| Path | Repo | Purpose |
|------|------|---------|
| `crates/bb-sparse/sparse` | cristeigabriela/sparse | MSDN API metadata generator |
| `crates/bb-sdk/phnt` | mrexodia/phnt-single-header | PHNT NT header generator |

Both have nested submodules (sdk-api, systeminformer). Use `.\update-submodules.ps1` to manage them.

## Self-maintenance

When making changes to this project, keep this file up to date:

- If you add, rename, or remove a crate, update the workspace structure diagram.
- If you change the dependency flow between crates, update the dependency diagram.
- If you add new conventions or architectural patterns, document them.
- If you rename files in bb-clang, update the file naming section.
- If you add new environment variables, update the overrides table.
- If you change the submodule setup, update the submodules table.

This file should always reflect the current state of the project, not its history.
