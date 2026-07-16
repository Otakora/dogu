# Bundled 7-Zip

Dogu bundles official 7-Zip binaries so archive extraction can work without
depending on tools installed by the user.

## English

### RAR on Windows

The historical `7za.exe` binary does not support `.rar`. Dogu therefore also
bundles `7z.exe` and `7z.dll`, which can list and extract RAR/RAR5 archives.

Dogu uses that backend for extraction previews, queue ghost files and real
extraction. `7za.exe` is kept for compatibility with existing 7-Zip operations.

Dogu does not bundle a RAR compressor. Creating `.rar` files is enabled only when
the user has `rar` or WinRAR installed on the system.

### Bundled version

- Version: `26.01`
- Official source: [7-zip.org](https://www.7-zip.org/download.html)

### Binaries

- Windows: `third_party/7zip/windows/7z.exe` + `7z.dll` for `.rar`, plus `7za.exe` + `7za.dll`.
- Linux: `third_party/7zip/linux/7zz`.

### Included licenses

- Windows: `third_party/7zip/windows/License.txt`
- Linux: `third_party/7zip/linux/License.txt`

## Español

Dogu incluye binarios oficiales de 7-Zip para extraer archivos sin depender de
herramientas instaladas por el usuario.

`7za.exe` no soporta `.rar`, así que en Windows Dogu incluye también `7z.exe` y
`7z.dll`, capaces de listar y extraer RAR/RAR5. Dogu no incluye compresor RAR; la
creación de `.rar` solo se habilita si el sistema aporta `rar` o WinRAR.

Versión integrada: `26.01`.
