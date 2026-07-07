# DolphinTool (RVZ engine)

Dogu converts GameCube/Wii ISO ⇄ RVZ with two engines:

- **nod** — the permissively-licensed ([MIT/Apache](https://crates.io/crates/nod))
  Rust library compiled into Dogu. This is the **default** engine and needs no
  external binary.
- **DolphinTool** — the official CLI tool from the Dolphin emulator, used as a
  **fallback** (and selectable as primary in Settings → RVZ).

Dogu **bundles** a self-built `DolphinTool`, the same way it bundles a self-built
`chdman`. It is GPL-2.0+; building and redistributing it is fine as long as the
corresponding source is offered (see
[`../THIRD-PARTY-LICENSES.md`](../THIRD-PARTY-LICENSES.md)).

## Expected paths

Dogu looks for the executable here (first match wins), then on `PATH`:

```
third_party/dolphin-tool/windows/DolphinTool.exe
third_party/dolphin-tool/linux/DolphinTool
```

Everything under `third_party/**` is bundled into release builds automatically
(see `src-tauri/tauri.conf.json`).

## Building the binaries

The binaries are **not committed** (build them once per platform and commit the
result, exactly like `chdman/linux/chdman`):

- **Linux:** `sudo bash linux/build_dolphin_tool.sh`
- **Windows:** `powershell -ExecutionPolicy Bypass -File windows/build_dolphin_tool.ps1`
  (needs Visual Studio 2022 with the C++ workload)

Both clone the pinned Dolphin release (`metadata.json` → `release_tag`), build
only the `dolphin-tool` CMake target (no Qt/GUI), and drop the binary in place.

Note: unlike chdman, DolphinTool links part of Dolphin's core, so a fully-static
Linux binary is not practical — build on an older-glibc distro for broad
compatibility and check `ldd` output.

## Command used by Dogu

```
DolphinTool convert -i <in> -o <out> -f rvz -c zstd -l <level> -b 131072   # ISO -> RVZ
DolphinTool convert -i <in> -o <out> -f iso                                 # RVZ -> ISO
```

## Licensing

GPL-2.0+. Invoked as a separate process (mere aggregation), like `chdman`. The
corresponding-source offer lives in `../THIRD-PARTY-LICENSES.md`. Keep the
Dolphin license/copyright with the binary when redistributing.
