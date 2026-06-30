# Dogu

> **道具** — *herramienta* (japonés)

Aplicacion de escritorio para gestionar colecciones de ROMs. Combina un explorador de archivos avanzado con conexiones remotas persistentes, extraccion de comprimidos y conversion de formatos. Disponible para Windows y Linux.

## Caracteristicas

### Explorador de archivos
- Arbol lateral + vista de contenido en lista o rejilla
- Ruta editable, boton subir y navegacion directa
- Busqueda global en todo el arbol y busqueda local con `Ctrl+F`
- Copiar, cortar, pegar, renombrar inline, eliminar, propiedades
- Vista de contenido con zoom ajustable y separador arrastrable
- Seleccion simple y multiple con resumen en barra inferior

### Conexiones remotas
- Protocolos: **SMB**, **SSH** (SFTP/SCP), **FTP**, **FTPS**
- Perfiles persistentes con gestion de credenciales y huellas SSH
- Varias sesiones abiertas al mismo tiempo con cambio rapido desde la barra lateral
- Transferencias local → remoto, remoto → local y remoto → remoto
- Apertura de archivos remotos con copia temporal administrada por la app

### Extraccion de archivos
- Soporte para `.zip`, `.7z`, `.rar` y formatos compatibles
- **7-Zip integrado** — sin dependencias del sistema
- Extraccion individual o en masa, con opciones de destino y previsualizacion

### Conversion CHD
- Conversion a `.chd` desde `.cue`, `.gdi`, `.toc`, `.iso`, `.bin + .cue`
- Restauracion desde `.chd` con `extractcd` y `extractdvd`
- **chdman v0.288 integrado** — sin instalacion adicional
- Seguimiento de progreso con log en tiempo real

## Stack tecnico

| Capa | Tecnologia |
|------|-----------|
| Backend | Rust + Tauri v2 |
| Frontend | TypeScript + Vite (vanilla, sin framework UI) |
| Remoto | `remotefs` — SMB / SSH / FTP / FTPS |
| Sidecars | chdman v0.288, 7-Zip |
| Empaquetado | NSIS (Windows), AppImage / DEB / Flatpak (Linux) |

## Compilacion

Requisitos: Node.js 20+, Rust stable, Python 3.12+

```powershell
# Windows
python .\tools\release.py build-windows
```

```bash
# Linux
python3 tools/release.py build-linux-native
python3 tools/release.py build-linux-flatpak
```

Guia completa con dependencias de sistema: [docs/building.md](docs/building.md)

## Herramientas de terceros integradas

Los binarios se empaquetan en `third_party/` y no requieren instalacion separada por parte del usuario:

| Herramienta | Fuente | Plataformas |
|------------|--------|-------------|
| chdman v0.288 | [MAME](https://mamedev.org/) | Windows, Linux |
| 7-Zip | [7-zip.org](https://7-zip.org/) | Windows (`7za`), Linux (`7zz`) |

## Plataformas objetivo

| Plataforma | Formatos de distribucion |
|-----------|--------------------------|
| Windows | Instalador NSIS |
| Linux | AppImage, DEB, Flatpak |
