# bb-cli

> Shared CLI settings for all `bb` command-line applications.

All CLI apps (including TUIs) in `bb` share a set of settings:

- A SDK to be selected;
- Whether the parser should process as if building for **user-mode** or **kernel-mode**;
- What architecture the parser should process as if building for (`amd64` / `x86` / `arm64` / ...);
- Whether to show Clang diagnostics.

`bb-cli` aims to put all of these settings into one place, to make it trivial for all CLI apps to automagically integrate them.

Moreover, `bb-cli` implements a utility to pick the SDK which will be selected that makes the process trivial (and it also makes it behave in a well-defined, shared way).

> **Note** — Subject to change, always make sure to check the latest [in lib.rs](./src/lib.rs).
