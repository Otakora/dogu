# 7-Zip integrado

Este proyecto integra binarios oficiales de 7-Zip para poder extraer `.7z` y `.rar` sin depender de herramientas instaladas por el usuario.

## RAR en Windows

El binario historico `7za.exe` no soporta archivos `.rar`. Por eso Dogu incluye
tambien `7z.exe` y `7z.dll`, que si soportan listar y extraer RAR/RAR5.

Dogu usa ese backend para la vista previa de extraccion, los fantasmas de cola y
la extraccion real. `7za.exe` se conserva como compatibilidad para operaciones
7-Zip existentes.

Dogu no incluye ningun compresor RAR. La creacion de `.rar` solo se habilita si
el usuario tiene `rar`/WinRAR instalado en su sistema.

## Version integrada

- Version: `26.01`
- Source oficial: [7-zip.org](https://www.7-zip.org/download.html)

## Binarios

- Windows: `third_party/7zip/windows/7z.exe` + `7z.dll` para `.rar`, y `7za.exe` + `7za.dll`
- Linux: `third_party/7zip/linux/7zz`

## Licencias incluidas

- Windows: `third_party/7zip/windows/License.txt`
- Linux: `third_party/7zip/linux/License.txt`
