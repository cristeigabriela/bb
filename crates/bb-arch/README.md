# bb-arch

> Architecture definitions, register sets, and ABI location types.

`bb-arch` provides the shared vocabulary for describing target architectures, hardware registers, and where values live at the ABI level.

This crate is used by both `bb-sdk` (which extends it with SDK-specific preprocessor defines) and `bb-clang` (which uses it for ABI-aware parameter assignment).

---

## What's inside

### `Arch`

The target architecture enum: `X86`, `Amd64`, `Arm`, `Arm64`.

Provides `from_triple()` to derive the architecture from a clang target triple, `target_triple()` for the MSVC triple, and `pointer_size()`.

### Registers

Full GPR enums for each architecture, plus x64 XMM registers:

| Module | Registers |
| --- | --- |
| `reg::x64` | `X64Gpr` (RAX..R15), `X64Xmm` (XMM0..XMM15) |
| `reg::x86` | `X86Gpr` (EAX..EDI) |
| `reg::arm64` | `Arm64Gpr` (X0..X30, SP) |
| `reg::arm32` | `Arm32Gpr` (R0..R12, SP, LR, PC) |

A `Register` sum type wraps all of them.

### ABI locations

- **`MemoryOperand`** — Where a value sits: `Reg(register)` or `RegImm { base, offset }` (e.g., `[RSP+0x28]`).
- **`ParamLocation`** — Where a parameter lives: `Direct` (register or stack slot, possibly multi-location for register pairs) or `Indirect` (caller-allocated, pointer passed).
- **`ReturnLocation`** — Where the return value goes: `Void`, `Register`, or `Indirect` (hidden pointer argument).

Stack offsets are **callee-entry RSP/ESP-relative** -- after CALL pushed the return address, before any prologue instructions execute.

### Display + serialization

The `display` module provides:

- **`register_name`** — canonical display name for any register (`RCX`, `XMM0`, `EAX`, etc.).
- **`operand_to_json`**, **`param_abi_to_json`**, **`return_abi_to_json`** — structured JSON serialization for ABI location types.
