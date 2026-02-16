# bb-types

> CLI application for querying and exporting `Type` entities from **Windows SDK** / **PHNT** headers.

`bb-types` is a CLI application dedicated to querying, and exporting, information extracted from `Type` entities with `bb-clang`, from the respective SDK (**Windows SDK**/**PHNT**) of your choice.

---

## Arguments

### Specific to `bb-types`

| Flag | Description |
| --- | --- |
| `--struct` / `-s` | Filter for the structs being searched for |
| `--field` / `-f` | Filter for a field within a struct. Does not nest. Compatible with depth |
| `--depth` / `-d` | Depth of inline field type expansion |
| `--filter` / `-H` | Filter your searches to a specific header |
| `--case-sensitive` / `-c` | Case-sensitive matching |
| `--json` | Output as JSON with full nested type expansion (ignores `--depth`) |

---

### Fuzzy suggestions

When an exact (non-wildcard) name doesn't match anything, `bb-types` suggests close matches — catching both typos and incomplete names:

```bash
bb-types --struct _PBE
error: no structs matching '_PBE'

  did you mean?

    _ABC
    _PSP
    _PEB
```

---

### Shared with `bb-consts`

<details>
<summary>Expand shared arguments</summary>

<br>

These arguments are managed by [`bb-cli`](./util/bb-cli/) and are shared across all CLI apps.

| Flag | Default | Description |
| --- | --- | --- |
| `--winsdk [VERSION]` | *(default SDK)* | Use Windows SDK headers. Optionally specify a version present in your environment |
| `--phnt [VERSION]` | -- | Use PHNT headers instead. Optionally specify a Windows version target |
| `--mode` / `-m` | `user` | `user` or `kernel` (defines `_KERNEL_MODE` for kernel) |
| `--arch` / `-a` | host | `x86` / `amd64` / `arm` / `arm64` -- supports cross-compilation |
| `--diagnostics` | off | Show Clang diagnostics. Useful for troubleshooting |

**PHNT version targets:** `win2k` `win-xp` `ws03` `vista` `win7` `win8` `win-blue` `threshold` `threshold2` `redstone` `redstone2` `redstone3` `redstone4` `redstone5` `19H1` `19H2` `20H1` `20H2` `21H1` `Win10-21H2` `Win10-22H2` `win11` `Win11-22H2`

</details>
