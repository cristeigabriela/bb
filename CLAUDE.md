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
│   ├── bb-clang         # libclang abstractions: Struct, Enum, Constant, Function, Param, TypeInfo
│   ├── bb-cli           # Shared CLI args (SharedArgs), suggestions, terminal_width, helpers
│   ├── bb-sdk           # Windows SDK + PHNT header config, parsing, architecture defines
│   ├── bb-shared         # Tiny utilities: glob_match, levenshtein, suggest_closest
│   ├── bb-sparse        # Embedded MSDN API metadata (compressed JSON from sparse submodule)
│   ├── bb-sql           # Generic SQL WHERE evaluator + SQLite export (rusqlite, sqlparser)
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
            bb-shared  bb-sparse  bb-sql
                   ↓
         cli/{bb-types, bb-consts, bb-funcs}
                   ↓
         tui/{bb-types-tui, bb-consts-tui}
```

- `bb-clang` is the core parsing library. It must NOT depend on `bb-sparse`, `bb-sdk`, `bb-sql`, or any CLI/TUI crate.
- `bb-sparse` is a pure data crate. It must NOT depend on `bb-clang`.
- `bb-sql` is a standalone SQL crate. It must NOT depend on `bb-clang`.
- `bb-funcs` joins `bb-clang` + `bb-sparse` via its `enriched` module.
- All CLIs use `bb-sql` for `--sqlite` export and (bb-funcs) `--where` filtering.

## Building

Requires MSVC build tools, LLVM/Clang (libclang.dll >= 18.1), Rust 2024 edition, and [uv](https://docs.astral.sh/uv/) (preferred) or Python >= 3.10. `uv` is used by `crates/bb-sparse/sparse` to manage its Python deps; bb-sparse's build.rs falls back to plain `python` / `py -3` if `uv` isn't on PATH.

```powershell
# On Windows, MSVC link.exe must be on PATH before Git's /usr/bin/link.exe
# If cargo fails to link, prepend MSVC to PATH or use a Developer Command Prompt

.\update-submodules.ps1   # init phnt + sparse submodules
cargo build --release
```

The `bb-sparse` build.rs auto-generates MSDN metadata from the sparse submodule in **two passes** — one for the SDK dataset (`sdk-api`, user-mode Win32 APIs) and one for the driver dataset (`windows-driver-docs-ddi`, KMDF/UMDF + kernel DDIs). Each pass is cached independently against its own submodule HEAD; only the pass whose submodule moved is rebuilt. The `bb-sdk` build.rs auto-generates phnt.h from the phnt submodule.

### Environment variable overrides

| Variable | Purpose |
|----------|---------|
| `BB_PHNT_HEADER` | Use a custom phnt.h instead of generating from submodule |
| `BB_SPARSE_SDK_JSON` | Use a pre-generated `sdk-api.json` instead of running sparse in SDK mode (alias: `BB_SPARSE_JSON`) |
| `BB_SPARSE_DRIVER_JSON` | Use a pre-generated `driver-docs.json` instead of running sparse in driver mode |

## Running tests

```powershell
# MSVC link.exe must be first on PATH
$env:PATH = "C:\Program Files\Microsoft Visual Studio\2022\Community\VC\Tools\MSVC\14.44.35207\bin\Hostx64\x64;$env:PATH"

cargo test --workspace --verbose
# or just integration tests:
cargo test --package bb-tests -- --test-threads=1
# or just bb-funcs unit tests:
cargo test --package bb-funcs
# or just bb-sql unit tests (evaluator + SQLite export):
cargo test --package bb-sql
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
- **Serialization**: all bb-clang types derive `Serialize`. The `ToJson` trait in `json.rs` provides structured JSON output. `--sqlite` exports mirror `--json` detail via `export_json_to_sqlite`.
- **Filter pattern**: each CLI has a `FuncFilter`/`StructFilter`/`ConstFilter` struct with pre-parse (Entity-level) and post-parse (constructed type-level) filtering.
- **Stack offsets are callee-entry RSP/ESP-relative** (after CALL, before prologue). Not RBP-relative.

## Key architectural decisions

- **`Param::is_stack()`** and **`Param::size()`** are methods on the Param type for ABI queries.
- **`entity_in_header()`** in `bb-clang/location.rs` is the shared header-matching helper used by all filter structs.
- **`bb_cli::current_command_string()`** is used by all CLIs for JSON `"command"` fields.
- **`format_abi_param()`** in `bb-clang/display/function.rs` is the shared ABI row formatter. Takes an optional `&TypedefIndex` so typedef'd param types render with a dim `(canonical)` annotation (e.g. `HANDLE (void *)`).
- **`format_tags()`** returns `Vec<String>` so callers can extend before joining.
- **`TypeInfo`** in `bb-clang/type_info.rs` is the shared type metadata struct embedded (via `#[serde(flatten)]`) in both `Field` and `Param`. Constructed via `From<clang::Type>`. Exposes `underlying_type`, `is_const`, `is_volatile`, `is_restrict`, `is_pointer`, `pointer_depth`, `is_function_pointer`, `is_array`, `array_size`.
- **`TypedefIndex`** in `bb-clang/typedef.rs` is a translation-unit-scoped map of every `EntityKind::TypedefDecl`. Each `Typedef` entry carries the alias name, the immediate target (`typedef_of`), the resolved canonical form (`canonical`), the optional `canonical_decl_name` (when the chain ends at a named record/enum), the full `chain` of intermediate steps, and a `TypedefKind` classification (`struct`, `union`, `enum`, `pointer`, `function_pointer`, `array`, `primitive`, `other`). Drives (1) alias-aware struct lookup in `bb-types`, (2) the `aliases: [...]` field on `Struct` JSON, (3) typedef-only hit reporting (`HANDLE`, `PVOID`), (4) the dim `(canonical)` annotation in CLI/TUI field & param renderers via `display::typedef_annotation`.
- **`Struct`** accepts `StructDecl`, `ClassDecl`, **and `UnionDecl`** (unions are modeled with overlapping field offsets — needed for `LARGE_INTEGER` and friends).
- **`Struct::display(depth, field_filter, typedef_index)`** takes an optional `&TypedefIndex` to drive header `[aka …]` aliases and inline field-type annotations. `Function::display_detail(typedef_index)` mirrors this for ABI rows + return type.
- **bb-funcs `enriched` module** owns the sparse metadata rendering. Enriched JSON is composed by starting from `p.to_json()` / `f.to_json()` and extending with sparse metadata. bb-clang stays generic.
- **bb-funcs `where_filter` module** evaluates SQL WHERE clauses via `bb-sql::Evaluator`.
- **`bb_cli::terminal_width()`** is the shared terminal width helper used by all CLIs.
- **`bb-sql`** provides a generic `Evaluator<T>` with a column resolver closure, plus `export_json_to_sqlite` for serde-based SQLite export. All CLIs support `--sqlite`.

## File naming in bb-clang

| File | Contents |
|------|----------|
| `function/abi.rs` | Calling conventions + ABI parameter assignment engine |
| `function/param.rs` | Param type with `is_stack()`, `size()`, embeds `TypeInfo` |
| `type_info.rs` | Shared `TypeInfo` struct: type classification (pointer, array, const, volatile, function pointer, underlying type) |
| `typedef.rs` | `TypedefIndex` / `Typedef` / `TypedefKind`: translation-unit-scoped typedef resolution (alias name → canonical form + chain + kind) |
| `constant/tokens.rs` | Clang ↔ cexpr token conversion |
| `constant/macro_.rs` | Macro resolution with identifier substitution |
| `ext.rs` | Extension traits for clang types (`UnderlyingType`, `AnonymousType`, etc.) |
| `json.rs` | `ToJson` trait + impls for all entity types (Struct, Field, Enum, Constant, Function, Param) |
| `display/constant.rs` | Constant rendering |
| `display/function.rs` | Function rendering (list, detail, shared formatters) |

Files using trailing underscores (`struct_/`, `enum_/`, `macro_.rs`) follow the Rust convention for avoiding keyword conflicts.

## Submodules

| Path | Repo | Purpose |
|------|------|---------|
| `crates/bb-sparse/sparse` | cristeigabriela/sparse (tracks `main`) | MSDN API metadata generator — embeds both sdk-api and windows-driver-docs-ddi |
| `crates/bb-sdk/phnt` | mrexodia/phnt-single-header | PHNT NT header generator |

`sparse` has two nested submodules (sdk-api, windows-driver-docs-ddi); `phnt` has one (systeminformer). Use `.\update-submodules.ps1` to manage them.

## Self-maintenance

When making changes to this project, keep this file up to date:

- If you add, rename, or remove a crate, update the workspace structure diagram.
- If you change the dependency flow between crates, update the dependency diagram.
- If you add new conventions or architectural patterns, document them.
- If you rename files in bb-clang, update the file naming section.
- If you add new environment variables, update the overrides table.
- If you change the submodule setup, update the submodules table.
- After every implementation session, review this file and update any sections that have drifted from the current state.

This file should always reflect the current state of the project, not its history.
