<div align="center">

# 道具 · Dogu

**A single workbench for the messy work of managing a ROM collection.**

*Dōgu (道具) — Japanese for "tool" or "implement".*

[English](#english) · [Español](#español)

</div>

---

## English

### What is Dogu?

Dogu is a desktop application for **handling the data behind a ROM collection**: moving files
between machines, keeping an eye on disk space, converting formats, unpacking archives and
building playlists — all from one window.

It grew out of a simple frustration: doing this kind of maintenance usually means juggling a
file explorer, an SFTP client, 7-Zip, a terminal and a couple of home-made `chdman` scripts,
all open at once. Dogu folds every one of those steps into a single tool so you stop
alt-tabbing between half a dozen programs to prepare one folder of games.

### What Dogu is *not*

It is **not** a launcher or a frontend. If you want to browse box art and press play — that is
LaunchBox, RetroBat or Batocera's job. Dogu lives one step earlier in the pipeline: it is the
place where you *curate, transfer and convert* the files before they ever reach an emulator.

| | Frontends (LaunchBox, Batocera…) | **Dogu** |
|---|---|---|
| Purpose | Browse and launch games | Move, convert and organize the files |
| You interact with | Cover art, metadata, emulators | Real files, folders, disks and remotes |
| The question it answers | *"What do I want to play?"* | *"How do I get these ROMs where they need to be?"* |

### What it does

**📁 Dual-pane file explorer** — Two independent panes side by side, each with its own tabs,
history and view mode. Drag tabs between panes, edit paths directly, search with `Ctrl+F`, and
switch between list and grid views. The classic copy / cut / paste / rename / delete you'd
expect, plus a properties panel.

**🌐 Persistent remote connections** — Connect over **SSH/SFTP**, **SCP**, **FTP**, **FTPS**
and **SMB**. Save connection profiles, keep several sessions open at once and switch between
them from the sidebar. Transfers work in every direction: local → remote, remote → local and
remote → remote.

**💾 Space awareness, everywhere** — Local disks *and* SSH remotes show a live free/total usage
bar in the sidebar. You know at a glance whether that batch of games actually fits before you
start moving it.

**🖥️ Built-in terminal** — A real local shell (PowerShell / bash / zsh) and SSH terminals for
your remotes, each opening at the pane's current path. Resizable, multi-tab, no context switch.

**⏳ Operation queue** — Flip on queue mode and copy/move operations stack up so you can review
and run them together, with a live progress log and pause/cancel.

**📦 Archive extraction** — Unpack `.zip`, `.7z`, `.rar` and more, one at a time or in bulk,
with destination options and a preview. **7-Zip is bundled** — nothing to install.

**💿 CHD conversion** — Convert `.cue`, `.gdi`, `.toc`, `.iso` and `.bin+.cue` to `.chd`, and
restore back out again. **chdman is bundled** too, with a real-time progress log.

**🎵 M3U playlist generator** — Build `.m3u` playlists from folders or selections, with
multi-disc detection and grouping by title.

### Tech stack

| Layer | Technology |
|-------|-----------|
| Backend | Rust + Tauri v2 |
| Frontend | Svelte 5 (runes) + TypeScript + Tailwind CSS v4 + Vite 8 |
| Remote | `remotefs` — SMB / SSH / FTP / FTPS |
| Bundled tools | chdman v0.288, 7-Zip |
| Packaging | NSIS (Windows), AppImage / DEB / Flatpak (Linux) |

### Development

Requirements: Node.js 20+, Rust stable.

```powershell
npm install
npm run tauri:dev
```

`tauri:dev` starts the Rust backend and the Vite server together; the app window opens
automatically with frontend hot-reload.

To type-check without launching the app:

```powershell
npm run check          # svelte-check (TypeScript + Svelte)
cargo check            # Rust (from src-tauri/)
```

> `npm run dev` starts **only the frontend** in the browser — Tauri's `invoke()` calls won't
> work there. It's useful just for iterating on pure styling or components without compiling Rust.

### Building

Requirements: Node.js 20+, Rust stable, Python 3.12+.

```powershell
# Windows (run on Windows)
python tools/release.py build-windows
```

```bash
# Linux — AppImage + DEB (run on Linux)
python3 tools/release.py build-linux-native

# Linux — Flatpak (includes the native build first)
python3 tools/release.py build-linux-flatpak
```

Artifacts land in `dist/`. Full guide with system dependencies: [docs/building.md](docs/building.md)

### Bundled third-party tools

These binaries ship inside `third_party/` — users don't install anything separately. Their
license texts travel with the app and are credited in **Settings → Credits & Licenses**.

| Tool | Source | License | Platforms |
|------|--------|---------|-----------|
| chdman v0.288 | [MAME](https://mamedev.org/) | GPL-2.0 | Windows, Linux |
| 7-Zip | [7-zip.org](https://7-zip.org/) | LGPL-2.1 | Windows (`7za`), Linux (`7zz`) |

See [THIRD_PARTY_NOTICES](THIRD_PARTY_NOTICES) for the full attribution.

### Target platforms

| Platform | Distribution formats |
|----------|----------------------|
| Windows | NSIS installer |
| Linux | AppImage, DEB, Flatpak |

---

## Español

### ¿Qué es Dogu?

Dogu es una aplicación de escritorio para **tratar los datos que hay detrás de una colección de
ROMs**: mover archivos entre máquinas, vigilar el espacio en disco, convertir formatos,
descomprimir y generar listas de reproducción — todo desde una sola ventana.

Nació de una frustración sencilla: este tipo de mantenimiento suele obligarte a hacer malabares
con un explorador de archivos, un cliente SFTP, 7-Zip, una terminal y un par de scripts caseros
de `chdman`, todos abiertos a la vez. Dogu reúne cada uno de esos pasos en una única herramienta
para que dejes de saltar entre media docena de programas solo para preparar una carpeta de juegos.

### Lo que Dogu *no* es

**No** es un launcher ni un frontend. Si lo que quieres es ver carátulas y darle a jugar, de eso
se encargan LaunchBox, RetroBat o Batocera. Dogu vive un paso antes en el proceso: es el lugar
donde *organizas, transfieres y conviertes* los archivos antes de que lleguen a un emulador.

| | Frontends (LaunchBox, Batocera…) | **Dogu** |
|---|---|---|
| Objetivo | Explorar y lanzar juegos | Mover, convertir y organizar los archivos |
| Interactúas con | Carátulas, metadatos, emuladores | Archivos, carpetas, discos y remotos reales |
| Pregunta que responde | *«¿A qué quiero jugar?»* | *«¿Cómo llevo estas ROMs a donde deben estar?»* |

### Qué hace

**📁 Explorador de doble panel** — Dos paneles independientes lado a lado, cada uno con sus
propias pestañas, historial y modo de vista. Arrastra pestañas entre paneles, edita rutas
directamente, busca con `Ctrl+F` y alterna entre vista de lista y cuadrícula. El clásico
copiar / cortar / pegar / renombrar / eliminar de siempre, más un panel de propiedades.

**🌐 Conexiones remotas persistentes** — Conecta por **SSH/SFTP**, **SCP**, **FTP**, **FTPS** y
**SMB**. Guarda perfiles de conexión, mantén varias sesiones abiertas a la vez y cambia entre
ellas desde la barra lateral. Las transferencias funcionan en todas las direcciones:
local → remoto, remoto → local y remoto → remoto.

**💾 Espacio a la vista, en todas partes** — Los discos locales *y* los remotos SSH muestran una
barra de uso libre/total en tiempo real en la barra lateral. Sabes de un vistazo si ese lote de
juegos cabe de verdad antes de empezar a moverlo.

**🖥️ Terminal integrado** — Un shell local real (PowerShell / bash / zsh) y terminales SSH para
tus remotos, cada uno abriéndose en la ruta actual del panel. Redimensionable, con varias
pestañas y sin cambiar de aplicación.

**⏳ Cola de operaciones** — Activa el modo cola y las operaciones de copia/movimiento se
acumulan para que las revises y ejecutes juntas, con un log de progreso en vivo y pausa/cancelación.

**📦 Extracción de comprimidos** — Descomprime `.zip`, `.7z`, `.rar` y más, de uno en uno o en
masa, con opciones de destino y previsualización. **7-Zip viene integrado** — nada que instalar.

**💿 Conversión CHD** — Convierte `.cue`, `.gdi`, `.toc`, `.iso` y `.bin+.cue` a `.chd`, y
restaura de vuelta. **chdman también viene integrado**, con un log de progreso en tiempo real.

**🎵 Generador de listas M3U** — Crea listas `.m3u` a partir de carpetas o selecciones, con
detección de multidisco y agrupación por título.

### Stack técnico

| Capa | Tecnología |
|------|-----------|
| Backend | Rust + Tauri v2 |
| Frontend | Svelte 5 (runes) + TypeScript + Tailwind CSS v4 + Vite 8 |
| Remoto | `remotefs` — SMB / SSH / FTP / FTPS |
| Herramientas integradas | chdman v0.288, 7-Zip |
| Empaquetado | NSIS (Windows), AppImage / DEB / Flatpak (Linux) |

### Desarrollo

Requisitos: Node.js 20+, Rust stable.

```powershell
npm install
npm run tauri:dev
```

`tauri:dev` arranca el backend Rust y el servidor Vite juntos; la ventana de la app se abre
automáticamente con hot-reload en el frontend.

Para verificar tipos sin lanzar la app:

```powershell
npm run check          # svelte-check (TypeScript + Svelte)
cargo check            # Rust (desde src-tauri/)
```

> `npm run dev` arranca **solo el frontend** en el navegador — los `invoke()` de Tauri no
> funcionan ahí. Sirve únicamente para iterar en estilos o componentes puros sin compilar Rust.

### Compilación

Requisitos: Node.js 20+, Rust stable, Python 3.12+.

```powershell
# Windows (ejecutar en Windows)
python tools/release.py build-windows
```

```bash
# Linux — AppImage + DEB (ejecutar en Linux)
python3 tools/release.py build-linux-native

# Linux — Flatpak (incluye antes el build nativo)
python3 tools/release.py build-linux-flatpak
```

Los artefactos se generan en `dist/`. Guía completa con dependencias de sistema:
[docs/building.md](docs/building.md)

### Herramientas de terceros integradas

Estos binarios se empaquetan dentro de `third_party/` — el usuario no instala nada por separado.
Sus textos de licencia viajan con la app y se acreditan en **Configuración → Créditos y licencias**.

| Herramienta | Fuente | Licencia | Plataformas |
|------------|--------|----------|-------------|
| chdman v0.288 | [MAME](https://mamedev.org/) | GPL-2.0 | Windows, Linux |
| 7-Zip | [7-zip.org](https://7-zip.org/) | LGPL-2.1 | Windows (`7za`), Linux (`7zz`) |

Consulta [THIRD_PARTY_NOTICES](THIRD_PARTY_NOTICES) para la atribución completa.

### Plataformas objetivo

| Plataforma | Formatos de distribución |
|-----------|--------------------------|
| Windows | Instalador NSIS |
| Linux | AppImage, DEB, Flatpak |
