# bb-sdk

Windows SDK and PHNT header management for the bb workspace.

Locates SDK installations, generates synthetic in-memory headers, and drives
libclang parsing. Neither `bb-types` nor `bb-consts` touch header files
directly -- this crate handles all of that.

## Exports

| Export | What |
|---|---|
| `HeaderConfig` | Top-level config: WinSDK or PHNT, with arch/mode. Call `.parse()` to get a `TranslationUnit` |
| `Arch` | `X86`, `Amd64`, `Arm`, `Arm64` -- target triples and preprocessor defines |
| `SdkMode` | `User` or `Kernel` |
| `SdkInfo` | Detected SDK include directory and version |
| `PhntVersion` | Win2000 through Win11-22H2 (23 variants) |
| `PHNT_HEADER` | The entire PHNT header embedded at compile time via `include_str!` |
| `get_sdk_info` | Read SDK paths from environment variables |
| `sdk_header` | Generate the synthetic `#include` cascade for Windows SDK |
| `phnt_synthetic_header` | Generate the synthetic header for PHNT |
| `parse_winsdk`, `parse_phnt` | Convenience parsing functions |

Also re-exports `Struct`, `Field`, `StructError`, `FieldError` from `bb-clang`.

## How synthetic headers work

Instead of pointing clang at a real `.h` file on disk, this crate builds a
string of `#include` directives covering the relevant subset of headers
(user-mode: `windows.h`, `winternl.h`, `dbghelp.h`, crypto, networking, etc.;
kernel-mode: `ntddk.h`, `wdm.h`, `ntifs.h`, `fltkernel.h`, etc.) and passes
it to libclang as an `Unsaved` buffer. No temp files.

For PHNT, the header content itself is compiled into the binary, prepended with
the appropriate base includes and `PHNT_VERSION` define, and fed in the same way.

## Environment

Expects a Visual Studio Developer Command Prompt (or equivalent) that sets
`WindowsSdkDir` and `WindowsSDKLibVersion`. Kernel mode additionally requires
WDK to be installed.
