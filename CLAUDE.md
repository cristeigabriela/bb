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
│   ├── bb-clang         # libclang abstractions: Struct, Union, Enum, Constant, Function, Param, TypeInfo, RecordKind, AnonRef
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
| `BB_NO_CACHE` | Bypass the on-disk AST cache (parse from headers every time) |

### AST cache

First runs of `bb-funcs` / `bb-types` / `bb-consts` save the parsed translation
unit via `clang_saveTranslationUnit` under `%LOCALAPPDATA%\bb\ast\<sha256>.ast`.
The cache key hashes the synthetic header content, every clang arg, and the
bb-sdk crate version — any change to header config, SDK install, target arch,
or a bb-sdk release invalidates automatically. Subsequent runs with the same
SDK / arch / mode load the saved AST and skip libclang's full re-parse.
Saved ASTs are ~80 MB each; clear `bb/ast/` to nuke them, or set
`BB_NO_CACHE=1` per-invocation. The README has timing tables.

### Parse hygiene: zero diagnostics — **MANDATORY**

> **This is non-negotiable.** Any change that touches `crates/bb-sdk/`,
> `crates/bb-clang/`, header inclusion order, preprocessor defines, or
> the PHNT submodule **MUST** be validated against `--diagnostics` and
> **MUST NOT** introduce any new clang `error:` or `warning:` lines.
> Catching these is the single most important step before merging.

`bb` parses the full Windows SDK + WDK + PHNT chain **without libclang
errors or warnings** under every mode combination (`--mode user`,
`--mode kernel`, `--phnt`, `--phnt --mode kernel`). The README publicly
advertises this (search for "zero libclang errors and zero warnings"
in README.md). Breaking the contract — even silently, with a single
new warning — is a regression that must be fixed before the PR lands.

**Required check before every header-touching change:**

```powershell
$env:PATH = "C:\Program Files\Microsoft Visual Studio\2022\Community\VC\Tools\MSVC\14.44.35207\bin\Hostx64\x64;$env:PATH"

# Use a match-something name so bb's own "no constants matching" error
# message doesn't show up in the output and pollute the grep. Any line
# clang emits will follow the `<path>:<line>:<col>: error|warning:`
# format — filter for that and the count MUST be zero.
foreach ($combo in @(
    @('user','--winsdk'), @('kernel','--winsdk'),
    @('user','--phnt'),   @('kernel','--phnt')
)) {
    $mode = $combo[0]; $sdk = $combo[1]
    $log = "$env:TEMP\bb-diag-$mode-$($sdk -replace '--','').txt"
    & .\target\release\bb-consts.exe $sdk --mode $mode --diagnostics --name STATUS_SUCCESS *> $log
    $errs  = (Select-String -Path $log -Pattern ': error:'   | Measure-Object).Count
    $warns = (Select-String -Path $log -Pattern ': warning:' | Measure-Object).Count
    Write-Host ("{0,-7} {1,-8} -> clang errors={2}  warnings={3}" -f $mode, $sdk, $errs, $warns)
}
```

Any non-zero clang `error:` or `warning:` count is a regression.
When one appears:

- Prefer to **fix the root cause**: missing `-D`, wrong inclusion order,
  a guarded define that needs to land before a specific header, etc.
- Only as a last resort, exclude the offending header explicitly in
  `crates/bb-sdk/src/winsdk/{user,kernel}.rs` with an inline rationale
  comment matching the format of the existing exclusions (see the
  "Excluded (with rationale)" section in README.md). Document **why**
  it had to be dropped, not just that it was.

Why this matters operationally: on `error:` clang performs partial
recovery and emits **phantom entities** into the AST. Those flow
straight into `--json` / `--sqlite` exports and into bb-viewer, where
they look real to downstream consumers. Warnings often signal silent
redefinitions that change a struct's layout or a macro's value without
anyone noticing — those bugs are nearly impossible to track down later.

Silent regressions (a header chain that started warning after an
unrelated refactor) are the worst case. Treat any new `warning:` line
with the same urgency as a build error.

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
- **`Struct` vs `Union` are strictly separate types** since the Item-5 refactor. `Struct::try_from` accepts only `StructDecl` / `ClassDecl`; `Union::try_from` accepts only `UnionDecl`. Both carry `kind: RecordKind` (`Struct` / `Union`) for symmetric JSON dispatch. Top-level union typedefs like `LARGE_INTEGER` are findable via `collect_unions` and the `find_union_by_name` lib helper (parallels `collect_structs` / `find_struct_by_name`). Anonymous nested unions never appear at the TU root — they surface only inside their parent record's member list.
- **`record.rs`** holds the shared `RecordKind` enum and the `AnonRef { kind, enclosing_record, field_path }` cross-reference used by anonymous nested records to point into the parent record's `referenced_types` slot.
- **Anonymous nested records are synthesized as `Field` entries**. Under default MSVC parsing the `DUMMYUNIONNAME`/`DUMMYSTRUCTNAME` macros expand to empty, so clang represents the inner union as a `UnionDecl` *sibling* of the parent struct's FieldDecls — no `FieldDecl` wrapping. `build_anon_record_field` in `struct_/field.rs` synthesizes a `Field` for that decl with synthetic name `<anonymous_N>` (per-parent counter, separate counters for nameless FieldDecls vs sibling record decls), `is_anonymous: true`, an `anon_ref`, and an offset computed by `anon_record_offset_in_parent` (asks the parent's type for the offset of any reachable named member). The synthetic Field's `entity` is the record decl itself; `Field::get_field_decl()` returns `None` for these (real fields return `Some`).
- **JSON shape: single `referenced_types` slot**. `Struct` / `Union` JSON outputs collapse into a uniform shape: `to_json` adds `referenced_types` as a name-string list of named referenced records; `to_json_full` adds it as full objects (named + anonymous, distinguished by per-entry `"kind"`). The mixed-record helper `bb_clang::records_to_json_full(structs, unions)` produces the top-level `{ types, referenced_types }` envelope shared by struct + union queries. The `bb-types` CLI wraps that with `command` and `typedefs`.
- **`Struct::display(depth, field_filter, typedef_index)`** takes an optional `&TypedefIndex` to drive header `[aka …]` aliases and inline field-type annotations. `Union::display(...)` mirrors it. `Function::display_detail(typedef_index)` is analogous for ABI rows + return type.
- **`TypedefKind::label()`** returns a human-readable string (`"struct"`, `"union"`, `"pointer"`, `"function pointer"`, etc.). Used by `display::format_typedef_summary` to render typedef-only hits like `HANDLE → PVOID → void *   (pointer)`.
- **bb-funcs `enriched` module** owns the sparse metadata rendering. Enriched JSON is composed by starting from `p.to_json()` / `f.to_json()` and extending with sparse metadata. bb-clang stays generic.
- **bb-funcs `where_filter` module** evaluates SQL WHERE clauses via `bb-sql::Evaluator`.
- **`bb_cli::terminal_width()`** is the shared terminal width helper used by all CLIs.
- **`bb-sql`** provides a generic `Evaluator<T>` with a column resolver closure, plus `export_json_to_sqlite` for serde-based SQLite export. All CLIs support `--sqlite`.
- **`HeaderGroup.pre_lines`**: every entry in `bb-sdk/src/winsdk/{user,kernel}.rs::GROUPS` can carry a slice of raw preprocessor lines emitted before its `#include`s. Used for the `WIN32_NO_STATUS` dance — `user::RAW_DEFINES` sets `WIN32_NO_STATUS` so `winnt.h` skips its small inline `STATUS_*` subset, and the final user-mode HeaderGroup undoes it with `pre_lines: &["#undef WIN32_NO_STATUS"]` right before `#include <ntstatus.h>` so the full set emits. Kernel mode gets a similar terminal `ntstatus.h` group (with empty `pre_lines`) as a safety net for environments without the WDK installed — `ntstatus.h` lives in `shared/` which the plain SDK always ships. Keep new preprocessor scaffolding as a HeaderGroup rather than special-casing the build path.

## File naming in bb-clang

| File | Contents |
|------|----------|
| `function/abi.rs` | Calling conventions + ABI parameter assignment engine |
| `function/param.rs` | Param type with `is_stack()`, `size()`, embeds `TypeInfo` |
| `type_info.rs` | Shared `TypeInfo` struct: type classification (pointer, array, const, volatile, function pointer, underlying type) |
| `typedef.rs` | `TypedefIndex` / `Typedef` / `TypedefKind` (+ `TypedefKind::label()` for display): translation-unit-scoped typedef resolution (alias name → canonical form + chain + kind) |
| `record.rs` | `RecordKind` (Struct/Union) discriminator + `AnonRef { kind, enclosing_record, field_path }` cross-reference for anonymous nested records |
| `struct_/mod.rs` | `Struct` type (rejects `UnionDecl`), `extract_nested_records` walker, `collect_nested_from_fields` |
| `struct_/field.rs` | `Field` type. `build_anon_record_field` + `anon_record_offset_in_parent` synthesize Field entries for sibling anon-record decls. `Field::record_kind()` dispatches on the underlying type's kind. |
| `union_/mod.rs` | `Union` type, parallel to `Struct` |
| `constant/tokens.rs` | Clang ↔ cexpr token conversion |
| `constant/macro_.rs` | Macro resolution with identifier substitution |
| `ext.rs` | Extension traits for clang types (`UnderlyingType`, `AnonymousType`, etc.) |
| `json.rs` | `ToJson` trait + impls for all entity types (Struct, Union, Field, Enum, Constant, Function, Param). `records_to_json_full(structs, unions)` produces the mixed-kind `{ types, referenced_types }` envelope. |
| `display/constant.rs` | Constant rendering |
| `display/function.rs` | Function rendering (list, detail, shared formatters) |
| `display/struct_.rs` | `render_struct` / `render_union` (mirrored), `typedef_annotation` for the dim `(canonical)` chip |
| `display/typedef.rs` | `format_typedef_summary` — one-line typedef-only hit row |

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
