<div align="center">
  <img src="assets/Dogu-logo.png" alt="Dogu" width="280" />

  <h3>Dogu</h3>
  <p><strong>A practical file workbench for local files, remotes and queued operations.</strong></p>
  <p><em>Dōgu / 道具 means "tool" in Japanese.</em></p>

  <p>
    <a href="#english">English</a> ·
    <a href="#espanol">Español</a>
  </p>
</div>

---

> Official source / Fuente oficial: [github.com/Otakora/dogu](https://github.com/Otakora/dogu)
>
> Official releases / Releases oficiales: [GitHub Releases](https://github.com/Otakora/dogu/releases)
>
> Release verification / Verificación de releases: official releases publish `SHA256SUMS.txt`
>
> Questions and support / Preguntas y soporte: GitHub Discussions
>
> Bugs and feature requests / Bugs y propuestas: GitHub Issues
>
> License / Licencia: `GPL-2.0-or-later`

<a id="english"></a>

## English

### What is Dogu?

Dogu is a desktop file manager with two panes, remote connections and a queue that understands more than "do this later".

It is built for the kind of file work that usually gets messy: extracting archives, moving things between machines, converting disc images, preparing multi-disc playlists, cleaning old files and chaining several steps without babysitting every folder by hand.

It started with ROM collection maintenance in mind, but most of the app behaves like a general-purpose file explorer with extra tools for archive and disc-image workflows.

### A quick safety note ⚠️

Dogu is in active development and it performs real operations on real files. Some options can delete originals, overwrite files, move folders or clean up sources after a conversion.

Please use it like a power tool:

- Try new workflows on test folders first.
- Keep backups of anything important.
- Review the queue before running destructive batches.
- Treat ghost files as a preview of planned work, not as a backup or guarantee.

### What Dogu is not

Dogu is not a launcher or an emulator frontend. It will not replace LaunchBox, RetroBat, Batocera or similar tools.

The goal is earlier in the pipeline: Dogu helps you prepare, move, convert and organize the files before they reach whatever frontend or emulator you actually use.

### The everyday explorer bits 🧭

Dogu gives you a familiar two-pane file workflow:

- Browse two locations side by side.
- Use local folders or connected remotes in either pane.
- Copy, move, rename, delete, create, refresh and inspect files.
- Open files or open them with another app.
- Pin favorite locations in the sidebar.
- Browse known folders and local volumes.
- Use breadcrumbs to jump to parent folders quickly.
- Drag files and folders between panes, into folders or onto breadcrumb segments.

Drag and drop supports local and remote combinations where the selected backend and operation allow it. If queue mode is active, planned ghost folders can also be used as part of queued workflows.

### Remote locations 🌐

Dogu can connect to remote storage and use it from the same two-pane interface.

Currently supported connection types include:

- SSH/SFTP.
- SSH with SCP-style transfer mode.
- FTP.
- FTPS.
- SMB, where the platform backend supports it.

Remote support includes browsing, basic file operations, transfers and saved connection profiles. SSH/SFTP profiles can also open a remote terminal when the server allows an interactive shell.

Disk usage is shown where Dogu can query it reliably. Some backends, such as FTP/FTPS and some SMB setups, may not expose portable free-space information.

Transfers that pass through Dogu report byte-level progress. Remote-to-remote operations that can stay inside the remote backend prioritize speed and avoid staging large data locally.

### The queue is the heart of Dogu ⏳

Queue mode lets you prepare work before executing it. That is useful when one task depends on another, such as:

1. Extract an archive.
2. Convert the files that will appear after extraction.
3. Move the converted files somewhere else.
4. Generate an M3U that points to the final paths.
5. Delete temporary folders when everything else is done.

Dogu tries to understand those relationships. It builds a plan, detects dependencies, warns about conflicts and runs compatible jobs in parallel when it is safe to do so.

The queue currently supports:

- Copy and move.
- Delete.
- Extract archives.
- Compress files or folders.
- Convert to CHD and restore from CHD.
- Convert or restore CSO.
- Pack or unpack XISO.
- Convert or restore RVZ.
- Generate M3U playlists.

Queue tools include retry for one failed job, retry for all failed jobs, configurable concurrency, dependency-aware ordering and recoverable pauses for some errors.

The queue panel also includes a relation map for dependencies, safe ordering and conflicts. Items that will disappear because of a queued move, cleanup or deletion are shown with a strikethrough preview.

### Ghost files, without the mystery 👻

Ghost files and folders appear only when queue mode is active. They are Dogu's way of saying: "this does not exist yet, but a queued operation is expected to create it".

That makes it possible to prepare later steps before earlier ones have finished. For example, you can queue an extraction, enter the ghost folder it will create, then queue a conversion from files that will exist after extraction.

Ghost predictions currently cover common queued outputs such as:

- Copies and moves.
- Local archive extraction previews.
- Compression outputs.
- CHD conversion and restoration outputs.
- CSO, XISO and RVZ outputs.
- M3U playlist files.
- Planned cleanup or deletion marks.

Remote archives may not always expose their full future contents before execution, because Dogu cannot inspect every remote archive without downloading or staging it first.

### Archives and compression 📦

Dogu can extract archives using bundled 7-Zip support. It can also preview many local archives before extraction and queue the result as ghost content.

Archive workflows include:

- Extract here, extract to a named folder or choose a custom destination.
- Extract several archives into separate folders.
- Extract to local or remote destinations.
- Optionally delete source archives after a successful extraction.
- Compress to ZIP or 7Z.
- Choose compression level and conflict behavior.
- Optionally delete originals after successful compression.

RAR extraction is handled through 7-Zip where supported. Dogu does not bundle a RAR compressor. Creating `.rar` files is only available if a compatible external RAR or WinRAR installation is detected.

### Disc-image tools 💿

Dogu includes a few helpers for disc-image and ROM maintenance.

CHD workflows:

- Convert supported disc images to `.chd`.
- Restore `.chd` files back to disc images.
- Scan folders for CUE/BIN and other supported sets.
- Handle CUE files and their linked BIN files together.
- Choose output names and destinations.
- Optionally delete originals after success.
- Optionally remove source subfolders after success.

Other local disc-image treatments:

- ISO to CSO, and CSO back to ISO.
- Xbox ISO/folder to XISO, and XISO back to a folder.
- GameCube/Wii ISO to RVZ, and RVZ back to ISO.

CSO, XISO and RVZ operations are local-only right now. RVZ can use Dogu's native `nod` engine, DolphinTool, or automatic fallback depending on settings and available tools.

### M3U playlists 🎵

Dogu can scan folders for multi-disc sets and generate `.m3u` playlists with relative paths.

This also works with queued workflows: if previous jobs are moving or creating the disc files, Dogu tries to generate the playlist using the paths those files should have when the M3U job finally runs.

### Settings and interface

Dogu includes:

- Light and dark themes.
- English and Spanish UI text.
- Font scale and compact mode.
- Default view preferences.
- Conflict behavior options.
- Queue mode and maximum concurrency settings.
- CHD scan depth settings.
- RVZ engine and fallback settings.
- Tool-status checks for bundled or optional helpers.
- Stable/Beta update channels with automatic checks enabled by default.
- A built-in terminal panel for local shells and supported SSH sessions.

### Updates and release channels

Dogu has two update channels:

- Stable: recommended official releases.
- Beta: builds prepared from `dev` for users who want to try recent work earlier.

If a user moves from Beta back to Stable, Dogu can offer the latest Stable release even when that means downgrading from a newer beta build.

Automatic installation is supported for Windows NSIS installs and Linux AppImage builds. DEB and Flatpak builds can notify the user and open the release page, but they do not self-install until Dogu has a proper package repository path for those formats.

### Bundled and integrated tools

| Tool | Used for | Notes |
| --- | --- | --- |
| `chdman` 0.288 | CHD conversion and restoration | Bundled helper from MAME. |
| 7-Zip 26.01 | Archive extraction and ZIP/7Z compression | Bundled helper. |
| `nod` 2.0.0-alpha.9 | RVZ conversion/restoration | Compiled into Dogu. |
| DolphinTool 2606 | Optional RVZ engine/fallback | Bundled where available and also searched on `PATH`. |

Legal and redistribution details are documented in [THIRD_PARTY_NOTICES](THIRD_PARTY_NOTICES) and [third_party/THIRD-PARTY-LICENSES.md](third_party/THIRD-PARTY-LICENSES.md).

### Development

Requirements:

- Node.js 20.19 or newer, or 22.12 or newer.
- Rust stable toolchain.
- Platform requirements for Tauri.

Install dependencies:

```bash
npm install
```

Run the full Tauri app in development mode:

```bash
npm run tauri:dev
```

Useful checks:

```bash
npm run check
npm run build
cargo test --manifest-path src-tauri/Cargo.toml
```

Build release packages:

```bash
python tools/release.py build-windows
```

Release builds need the updater signing environment described in [docs/building.md](docs/building.md).

Packaging notes:

| Platform | Current status |
| --- | --- |
| Windows | NSIS installer builds are supported. Stable releases also prepare `Otakora.Dogu` winget metadata. |
| Linux AppImage | Official automated artifact and the Linux self-update path. |
| Linux | DEB and Flatpak are supported in the automated release flow. |
| macOS | Not currently a maintained release target. |

More build details are available in [docs/building.md](docs/building.md) and [docs/release-channels.md](docs/release-channels.md).

---

<a id="espanol"></a>

## Español

### Qué es Dogu

Dogu es un gestor de archivos de escritorio con dos paneles, conexiones remotas y una cola que entiende algo más que "haz esto luego".

Está pensado para ese trabajo con archivos que suele volverse lioso: extraer comprimidos, mover cosas entre máquinas, convertir imágenes de disco, preparar playlists multidisco, limpiar archivos antiguos y encadenar varios pasos sin tener que vigilar cada carpeta a mano.

Nació pensando en el mantenimiento de colecciones de ROMs, pero gran parte de la app funciona como un explorador de archivos general con herramientas extra para comprimidos e imágenes de disco.

### Nota rápida de seguridad ⚠️

Dogu está en desarrollo activo y ejecuta operaciones reales sobre archivos reales. Algunas opciones pueden eliminar originales, sobrescribir archivos, mover carpetas o limpiar orígenes después de una conversión.

Úsalo como una herramienta potente:

- Prueba flujos nuevos sobre carpetas de prueba.
- Mantén copias de seguridad de cualquier cosa importante.
- Revisa la cola antes de ejecutar lotes destructivos.
- Trata los archivos fantasma como una vista previa del trabajo planificado, no como una copia de seguridad ni una garantía.

### Lo que Dogu no es

Dogu no es un launcher ni un frontend de emulación. No pretende sustituir a LaunchBox, RetroBat, Batocera o herramientas parecidas.

Su sitio está un paso antes: Dogu ayuda a preparar, mover, convertir y ordenar los archivos antes de que lleguen al frontend o emulador que uses.

### El explorador del día a día 🧭

Dogu ofrece un flujo familiar de explorador en dos paneles:

- Navega por dos ubicaciones en paralelo.
- Usa carpetas locales o remotas conectadas en cualquiera de los paneles.
- Copia, mueve, renombra, borra, crea, actualiza e inspecciona archivos.
- Abre archivos o ábrelos con otra aplicación.
- Fija ubicaciones favoritas en la barra lateral.
- Navega por carpetas conocidas y volúmenes locales.
- Usa la barra de ruta para saltar rápido a carpetas superiores.
- Arrastra archivos y carpetas entre paneles, dentro de carpetas o sobre segmentos de la ruta.

El arrastre soporta combinaciones locales y remotas cuando el backend y la operación lo permiten. Si el modo cola está activo, los destinos fantasma también pueden formar parte de flujos encolados.

### Ubicaciones remotas 🌐

Dogu puede conectarse a almacenamiento remoto y usarlo desde la misma interfaz de dos paneles.

Tipos de conexión soportados actualmente:

- SSH/SFTP.
- SSH con modo de transferencia tipo SCP.
- FTP.
- FTPS.
- SMB, cuando el backend de la plataforma lo soporta.

El soporte remoto incluye navegación, operaciones básicas, transferencias y perfiles de conexión guardados. Los perfiles SSH/SFTP también pueden abrir un terminal remoto cuando el servidor permite una shell interactiva.

El uso de disco se muestra cuando Dogu puede consultarlo de forma fiable. Algunos backends, como FTP/FTPS y ciertas configuraciones SMB, pueden no ofrecer información portable de espacio libre.

Las transferencias que pasan por Dogu muestran progreso por bytes. Las operaciones remoto-remoto que pueden permanecer dentro del backend remoto priorizan velocidad y evitan preparar grandes datos en local.

### La cola es el corazón de Dogu ⏳

El modo cola permite preparar trabajo antes de ejecutarlo. Eso viene muy bien cuando una tarea depende de otra, por ejemplo:

1. Extraer un comprimido.
2. Convertir los archivos que aparecerán tras la extracción.
3. Mover los archivos convertidos a otra ubicación.
4. Generar un M3U que apunte a las rutas finales.
5. Borrar carpetas temporales cuando todo lo anterior haya terminado.

Dogu intenta entender esas relaciones. Construye un plan, detecta dependencias, avisa de conflictos y ejecuta trabajos compatibles en paralelo cuando es seguro hacerlo.

La cola soporta actualmente:

- Copiar y mover.
- Borrar.
- Extraer comprimidos.
- Comprimir archivos o carpetas.
- Convertir a CHD y restaurar desde CHD.
- Convertir o restaurar CSO.
- Empaquetar o desempaquetar XISO.
- Convertir o restaurar RVZ.
- Generar playlists M3U.

La cola incluye reintento de una tarea fallida, reintento de todas las fallidas, concurrencia configurable, ordenación por dependencias y pausas recuperables para algunos errores.

El panel de cola también incluye un mapa de relaciones para dependencias, orden seguro y conflictos. Los elementos que desaparecerán por un movimiento, limpieza o borrado en cola se muestran tachados como vista previa.

### Archivos fantasma, sin misterio 👻

Los archivos y carpetas fantasma solo aparecen cuando el modo cola está activo. Son la forma que tiene Dogu de decir: "esto todavía no existe, pero una operación en cola debería crearlo".

Gracias a eso puedes preparar pasos posteriores antes de que terminen los anteriores. Por ejemplo, puedes encolar una extracción, entrar en la carpeta fantasma que va a crear y preparar una conversión sobre archivos que existirán tras extraer.

Las predicciones fantasma cubren actualmente salidas habituales como:

- Copias y movimientos.
- Previsualizaciones de extracción de comprimidos locales.
- Salidas de compresión.
- Conversión y restauración CHD.
- Salidas CSO, XISO y RVZ.
- Archivos M3U.
- Marcas de limpieza o borrado planificado.

Los comprimidos remotos no siempre pueden mostrar todo su contenido futuro antes de ejecutarse, porque Dogu no puede inspeccionar cualquier archivo remoto sin descargarlo o prepararlo antes.

### Comprimidos y compresión 📦

Dogu puede extraer comprimidos usando soporte integrado de 7-Zip. También puede previsualizar muchos comprimidos locales antes de extraerlos y encolar el resultado como contenido fantasma.

Flujos con comprimidos:

- Extraer aquí, extraer a una carpeta con nombre o elegir un destino personalizado.
- Extraer varios comprimidos en carpetas separadas.
- Extraer a destinos locales o remotos.
- Eliminar los comprimidos de origen tras una extracción correcta.
- Comprimir a ZIP o 7Z.
- Elegir nivel de compresión y comportamiento ante conflictos.
- Eliminar originales después de una compresión correcta.

La extracción RAR se gestiona mediante 7-Zip cuando está soportada. Dogu no incluye un compresor RAR. Crear archivos `.rar` solo está disponible si se detecta una instalación externa compatible de RAR o WinRAR.

### Herramientas de imágenes de disco 💿

Dogu incluye algunas utilidades para mantenimiento de ROMs e imágenes de disco.

Flujos CHD:

- Convertir imágenes de disco compatibles a `.chd`.
- Restaurar archivos `.chd` a imágenes de disco.
- Escanear carpetas buscando conjuntos CUE/BIN y otros formatos soportados.
- Tratar archivos CUE y sus BIN enlazados como un conjunto.
- Elegir nombres y destinos de salida.
- Eliminar originales opcionalmente tras completar con éxito.
- Eliminar subcarpetas de origen opcionalmente tras completar con éxito.

Otros tratamientos locales:

- ISO a CSO, y CSO de vuelta a ISO.
- ISO/carpeta Xbox a XISO, y XISO de vuelta a carpeta.
- ISO de GameCube/Wii a RVZ, y RVZ de vuelta a ISO.

Las operaciones CSO, XISO y RVZ son solo locales por ahora. RVZ puede usar el motor nativo `nod`, DolphinTool o fallback automático según la configuración y las herramientas disponibles.

### Playlists M3U 🎵

Dogu puede escanear carpetas buscando conjuntos multidisco y generar playlists `.m3u` con rutas relativas.

También encaja con flujos en cola: si tareas anteriores van a mover o crear los archivos de disco, Dogu intenta generar la playlist usando las rutas que deberían tener cuando por fin se ejecute la tarea M3U.

### Ajustes e interfaz

Dogu incluye:

- Temas claro y oscuro.
- Textos de interfaz en inglés y español.
- Escala de fuente y modo compacto.
- Preferencias de vista por defecto.
- Opciones de comportamiento ante conflictos.
- Modo cola y concurrencia máxima configurables.
- Profundidad de escaneo CHD configurable.
- Ajustes de motor y fallback para RVZ.
- Comprobación de estado de herramientas integradas u opcionales.
- Canales de actualización Stable/Beta con comprobación automática activada por defecto.
- Panel de terminal integrado para shells locales y sesiones SSH soportadas.

### Actualizaciones y canales de release

Dogu tiene dos canales de actualización:

- Stable: releases oficiales recomendadas.
- Beta: builds preparadas desde `dev` para probar antes cambios recientes.

Si un usuario vuelve de Beta a Stable, Dogu puede ofrecer la última Stable aunque eso implique bajar desde una beta más nueva.

La instalación automática está soportada en Windows NSIS y Linux AppImage. Los paquetes DEB y Flatpak pueden avisar al usuario y abrir la página de la release, pero no se autoinstalan hasta que Dogu tenga una ruta adecuada de repositorio para esos formatos.

### Herramientas integradas

| Herramienta | Uso | Notas |
| --- | --- | --- |
| `chdman` 0.288 | Conversión y restauración CHD | Herramienta integrada procedente de MAME. |
| 7-Zip 26.01 | Extracción de comprimidos y compresión ZIP/7Z | Herramienta integrada. |
| `nod` 2.0.0-alpha.9 | Conversión/restauración RVZ | Compilado dentro de Dogu. |
| DolphinTool 2606 | Motor opcional/fallback para RVZ | Integrado cuando está disponible y también buscado en `PATH`. |

Los detalles legales y de redistribución están documentados en [THIRD_PARTY_NOTICES](THIRD_PARTY_NOTICES) y [third_party/THIRD-PARTY-LICENSES.md](third_party/THIRD-PARTY-LICENSES.md).

### Desarrollo

Requisitos:

- Node.js 20.19 o superior, o 22.12 o superior.
- Toolchain estable de Rust.
- Requisitos de plataforma necesarios para Tauri.

Instalar dependencias:

```bash
npm install
```

Ejecutar la app completa de Tauri en modo desarrollo:

```bash
npm run tauri:dev
```

Comprobaciones útiles:

```bash
npm run check
npm run build
cargo test --manifest-path src-tauri/Cargo.toml
```

Construir paquetes de release:

```bash
python tools/release.py build-windows
```

Los builds de release necesitan el entorno de firma updater descrito en [docs/building.md](docs/building.md).

Notas de empaquetado:

| Plataforma | Estado actual |
| --- | --- |
| Windows | Los instaladores NSIS están soportados. Las releases stable también preparan metadatos winget `Otakora.Dogu`. |
| Linux AppImage | Artefacto oficial automatizado y ruta de autoactualización en Linux. |
| Linux | DEB y Flatpak están soportados en el flujo automatizado de release. |
| macOS | No es actualmente un objetivo de release mantenido. |

Hay más detalles de compilación en [docs/building.md](docs/building.md) y [docs/release-channels.md](docs/release-channels.md).
