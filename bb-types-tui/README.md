# bb-types-tui

> TUI browser for **Windows SDK** / **PHNT** struct types.

`bb-types-tui` is a TUI version of the `bb-types` CLI application crate, using its library code, and exposing the data that is gathered with it to a [`bb-tui`](./util/bb-tui/) data model.

---

## Help

```
Benowin Blanc (bb): Windows through a detective's lens...

TUI browser for Windows SDK / PHNT struct types.

Usage: bb-types-tui.exe [OPTIONS]

Options:
      --winsdk [<WINSDK>]     Use Windows SDK headers (optionally specify version)
      --phnt [<PHNT>]         Use PHNT headers with specified version [possible values: win2k, win-xp, ws03, vista, win7, win8, win-blue, threshold, threshold2, redstone, redstone2, redstone3, redstone4, redstone5, 19H1, 19H2, 20H1, 20H2, 21H1, Win10-21H2, Win10-22H2, win11, Win11-22H2]
  -m, --mode <MODE>           Mode: user or kernel (defines _KERNEL_MODE for kernel) [default: user] [possible values: user, kernel]
  -a, --arch <ARCH>           Architecture to target (supports cross-compilation) [default: amd64] [possible values: x86, amd64, arm, arm64]
      --diagnostics           Show clang diagnostics
  -H, --filter <FILTER>       Filter by header file (e.g., winternl.h)
  -s, --struct <STRUCT_NAME>  Struct name pattern (supports * wildcard)
  -c, --case-sensitive        Case-sensitive matching
  -h, --help                  Print help
```

> **Note** — This may be outdated, please always check the latest output on your own machine.

---

## What am I seeing?

Invoke the command ever-so-simply, and it will start in **Windows SDK** mode. You may change this by starting it with one of the SDK flags.

**On the top** — you will see a search bar. Interact with it just as you would with `bb-types`. It uses `*` as its match character, just like the aforementioned.

**On the left** — you will see a file tree, as a way to separate the type entities on the basis of their source location.

**On the right** — you will see the content area, displaying the types.

**At the bottom** — you will see a status bar that tells you all the keybinds you must know to operate this damn thing.

---

## Stuck?

You may always exit the application by pressing `q`.
