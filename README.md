# Dogu

> **道具** — *herramienta* (japonés)

Aplicacion de escritorio para gestionar colecciones de ROMs. Combina un explorador de archivos avanzado con conexiones remotas persistentes, extraccion de comprimidos, conversion de formatos y terminal integrado. Disponible para Windows y Linux.

## Caracteristicas

### Explorador de archivos
- Vista dividida (split pane) con divisor arrastrable — dos paneles independientes con sus propias pestañas, historial y modo de vista
- Pestañas multiples con drag-and-drop entre paneles, reordenacion y menu contextual
- Arbol lateral con Favoritos, Este equipo (discos con barra de uso) y Conexiones remotas
- Ruta editable, navegacion directa y barra de herramientas responsive (dos filas en vista dividida)
- Busqueda integrada por pane con `Ctrl+F`
- Copiar, cortar, pegar, renombrar inline, eliminar, propiedades
- Vista en lista y rejilla con zoom ajustable

### Conexiones remotas
- Protocolos: **SSH/SFTP**, **SCP**, **FTP**, **FTPS**, **SMB**
- Perfiles persistentes con gestion de credenciales y verificacion de huellas SSH
- Varias sesiones abiertas simultaneamente con cambio rapido desde la barra lateral
- Indicador de espacio libre y total para conexiones SSH (barra visual, igual que discos locales)
- Transferencias local → remoto, remoto → local y remoto → remoto
- Apertura de archivos remotos con copia temporal administrada por la app

### Terminal integrado
- Terminal local con shell nativo (PowerShell/bash/zsh segun plataforma)
- Terminal SSH para conexiones remotas, arrancando en la ruta activa del panel
- Multipestañas, redimensionable con el panel de archivos

### Cola de operaciones
- Modo cola: las operaciones de copia/movimiento se acumulan y se ejecutan juntas
- Vista de progreso con log en tiempo real y posibilidad de pausa/cancelacion

### Extraccion de archivos
- Soporte para `.zip`, `.7z`, `.rar` y formatos compatibles
- **7-Zip integrado** — sin dependencias del sistema
- Extraccion individual o en masa con opciones de destino y previsualizacion

### Conversion CHD
- Conversion a `.chd` desde `.cue`, `.gdi`, `.toc`, `.iso`, `.bin + .cue`
- Restauracion desde `.chd` con `extractcd` y `extractdvd`
- **chdman v0.288 integrado** — sin instalacion adicional
- Seguimiento de progreso con log en tiempo real

### Generador de M3U
- Creacion de listas `.m3u` a partir de carpetas o selecciones de archivos
- Soporte para multidisc y agrupacion por titulo

## Stack tecnico

| Capa | Tecnologia |
|------|-----------|
| Backend | Rust + Tauri v2 |
| Frontend | Svelte 5 (runes) + TypeScript + Tailwind CSS v4 + Vite 8 |
| Remoto | `remotefs` — SMB / SSH / FTP / FTPS |
| Sidecars | chdman v0.288, 7-Zip |
| Empaquetado | NSIS (Windows), AppImage / DEB / Flatpak (Linux) |

## Desarrollo

Requisitos: Node.js 20+, Rust stable

```powershell
npm install
npm run tauri:dev
```

`tauri:dev` arranca el backend Rust y el servidor Vite juntos. La ventana de la app se abre automaticamente con hot-reload en el frontend.

Para verificar tipos sin lanzar la app:

```powershell
npm run check          # svelte-check (TypeScript + Svelte)
cargo check            # Rust (desde src-tauri/)
```

> `npm run dev` arranca **solo el frontend** en el navegador — los `invoke()` de Tauri no funcionan. Util unicamente para iterar en estilos o componentes puros sin compilar Rust.

## Compilacion

Requisitos: Node.js 20+, Rust stable, Python 3.12+

```powershell
# Windows (ejecutar en Windows)
python tools/release.py build-windows
```

```bash
# Linux — AppImage + DEB (ejecutar en Linux)
python3 tools/release.py build-linux-native

# Linux — Flatpak (incluye el build nativo previo)
python3 tools/release.py build-linux-flatpak
```

Los artefactos se generan en `dist/`. Guia completa con dependencias de sistema: [docs/building.md](docs/building.md)

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
