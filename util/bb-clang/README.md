# bb-clang

Libclang abstraction layer for the bb workspace.

Takes raw `clang::Entity` values from a parsed translation unit and turns them
into typed Rust objects you can query, filter, serialize, and display.

## Exports

| Export | What |
|---|---|
| `Struct` | Struct/class declaration with fields, size, source location |
| `Field` | A field within a struct -- offset, size, type, nesting support |
| `Enum` | Enum declaration with its constants |
| `Constant` | A constant from an enum, `const` variable, or `#define` macro |
| `ConstValue` | The value itself: `I64`, `U64`, or `F64` |
| `ConstLookup` | `HashMap<String, ConstValue>` for macro name resolution |
| `render_constants` | Render a slice of constants as a colored aligned tree |
| `SourceLocation` | File, line, column from the original header |
| `StructError`, `FieldError`, `EnumError`, `ConstantError` | Error types |

Also re-exports `Entity`, `EntityKind`, `Index`, `TranslationUnit`, `Unsaved`
from the `clang` crate.

## Display

Structs render in WinDbg `dt` style with Unicode box-drawing, colored columns
(yellow offsets, green sizes, cyan types), and recursive expansion with cycle
detection.

Constants render as aligned tables with inline macro composition breakdown
(showing which named constants contribute to a composite `#define`).

## Traits

Extends `clang::Type` with: `AnonymousType`, `DeclarationKind`,
`UnderlyingType`, `HasChildrenType` -- see `bb_clang::traits`.
