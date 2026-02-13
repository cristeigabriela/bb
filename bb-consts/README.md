# bb-consts

> CLI application for querying and exporting `Enum`/`Constant` entities from **Windows SDK** / **PHNT** headers.

`bb-consts` is a CLI application dedicated to querying, and exporting, information extracted from `Enum`/`Constant` entities with `bb-clang`, from the respective SDK (**Windows SDK**/**PHNT**) of your choice.

---

## Arguments

### Specific to `bb-consts`

| Flag | Description |
| --- | --- |
| `--name` / `-n` | Filter for constants. Use `::` in the query to scope your search to enums |
| `--enum` / `-e` | Filter for enums being searched for |
| `--filter` / `-H` | Filter your searches to a specific header |
| `--case-sensitive` / `-c` | Case-sensitive matching |
| `--json` | Output as a JSON array of constants (plus extra information) |

---

### Fuzzy suggestions

When an exact (non-wildcard) name doesn't match anything, `bb-consts` suggests close matches — catching both typos and incomplete names:

```bash
bb-consts --name INVALID_
error: no constants matching 'INVALID_'

  did you mean?

    INVALID_ATOM
    INVALID_SOCKET
    INVALID_FILE_SIZE
    INVALID_LINK_INDEX
    INVALID_HANDLE_VALUE
```

This also works for `--enum` patterns.

---

### Shared with `bb-types`

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
