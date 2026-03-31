# bb-sdk

> Synthetic header generation for **Windows SDK** and **PHNT**.

`bb-sdk` is responsible for generating synthetic headers that allow `bb-clang` to later index and parse them.

To get there however, the crate also takes care of the following things:

- Checking that your environment is set up with **Windows SDK**;
- Parsing your environment's latest **Windows SDK** version;
    - Checking if you have all the pre-requisites necessary for generating a building kernel-mode SDK, if applicable.

This crate also takes on the responsibility to handle versions for the provided SDKs.

---

## Architectures

Target architectures are defined in [`bb-arch`](../bb-arch/) and re-exported here. `bb-sdk` extends them with SDK-specific preprocessor defines via the `ArchDefines` trait.

`x86` | `amd64` | `arm` | `arm64`

### Header configuration

These are later relevant when you're defining a header configuration.

From a header configuration, you can obtain a translation unit.

In preparing this, the header configuration's information will be used to provide stuff like command-line arguments (such as the target architecture), and more.

The result will be a translation unit that is created from an in-memory file.

---

## PHNT headers

The PHNT header (`phnt.h`) provides internal NT structure definitions not available in the public Windows SDK. It is generated at build time from the [phnt-single-header](https://github.com/mrexodia/phnt-single-header) submodule.

### How it works

The `build.rs` resolves `phnt.h` in this order:

1. **`BB_PHNT_HEADER`** env var — use a custom `phnt.h` directly.
2. **`phnt.h`** next to this crate — local override file.
3. **Submodule generation** — runs `amalgamate.py` from the `phnt/` submodule:
   - Initializes the `phnt` submodule if not present.
   - Initializes the nested `systeminformer` submodule (the PHNT source).
   - Downloads `cpp-amalgamate.exe` (if missing) to combine headers.
   - Runs `amalgamate.py` to generate `phnt/out/phnt.h`.
   - Caches the result — subsequent builds are instant until the submodule changes.

### Setup

The recommended way:

```powershell
.\update-submodules.ps1 phnt
```

This initializes the submodule, downloads dependencies, and generates `phnt.h` automatically.

### Manual setup

```bash
cd crates/bb-sdk/phnt
git submodule update --init       # init systeminformer source
python amalgamate.py              # generate out/phnt.h
```

### Custom header

To use your own `phnt.h` without the submodule:

```powershell
$env:BB_PHNT_HEADER = "C:\path\to\my\phnt.h"
cargo build
```

### Updating

To update to the latest PHNT definitions:

```bash
cd crates/bb-sdk/phnt
git pull                          # update the generator
git submodule update --remote     # update systeminformer source
python amalgamate.py              # regenerate
```

Or delete the cached stamp and rebuild:

```powershell
.\update-submodules.ps1 phnt
cargo build
```
