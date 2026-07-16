# chdman integration notes

English is the primary language for this technical note. Spanish follows below.

## English

Research date: `2026-06-25`.

### Official sources

- Official `chdman` documentation:
  [https://docs.mamedev.org/tools/chdman.html](https://docs.mamedev.org/tools/chdman.html)
- Official MAME downloads:
  [https://www.mamedev.org/release.html](https://www.mamedev.org/release.html)
- Official MAME repository:
  [https://github.com/mamedev/mame](https://github.com/mamedev/mame)

### Useful observations for Dogu

- Official documentation describes `chdman` as a tool for creating, converting,
  verifying and extracting CHD images.
- The relevant commands for Dogu are `createcd`, `createdvd` and extraction/info
  commands used by restore workflows.
- The official docs do not list every supported input extension, but the MAME
  source clearly includes CUE/BIN, GDI, TOC and ISO-oriented flows.
- On `2026-06-25`, the official downloads page exposed Windows binaries for
  MAME `0.288`, while the published documentation was versioned as `0.289`.
  Dogu should not assume those numbers always match.

### Implementation decision

Dogu exposes CHD conversion for:

- `.cue`
- `.gdi`
- `.toc`
- `.iso`
- `.bin` when a sibling `.cue` exists

This covers the most common disc-image ROM workflows, especially PlayStation
sets based on `.cue` + `.bin`.

Dogu does not expose CDI as a reliable CHD source because CDI can be lossy and is
not a safe preservation-oriented input for these workflows.

### Current project integration

- Windows bundles official `chdman 0.288 (mame0288)` at
  `third_party/chdman/windows/chdman.exe`.
- The official package `COPYING` file is kept with the bundled binary.
- Linux bundles `third_party/chdman/linux/chdman`, compiled from official MAME
  `0.288` source in Ubuntu/WSL.
- The official MAME release page does not publish an equivalent Linux binary, so
  Dogu's Linux distribution relies on a source-built binary.
- Runtime execution has been verified on Windows and Ubuntu/WSL.

### Windows path limitation

`chdman` has been verified to be unsafe with non-ASCII paths on Windows. Dogu
works around that by running `chdman` from an ASCII-safe working directory with
relative file names, then moving the result with Rust filesystem APIs when
needed. File names that are unsafe for `chdman` are blocked or offered for
de-accented rename before queue execution.

## Español

Fecha de revisión: `2026-06-25`.

### Fuentes oficiales

- Documentación oficial de `chdman`:
  [https://docs.mamedev.org/tools/chdman.html](https://docs.mamedev.org/tools/chdman.html)
- Descargas oficiales de MAME:
  [https://www.mamedev.org/release.html](https://www.mamedev.org/release.html)
- Repositorio oficial de MAME:
  [https://github.com/mamedev/mame](https://github.com/mamedev/mame)

### Observaciones útiles para Dogu

- La documentación oficial describe `chdman` como herramienta para crear,
  convertir, verificar y extraer imágenes CHD.
- Los comandos relevantes para Dogu son `createcd`, `createdvd` y los comandos
  de info/extracción usados en restauración.
- La documentación oficial no enumera por extensión todos los formatos de
  entrada, pero el código fuente de MAME deja claros los flujos CUE/BIN, GDI,
  TOC e ISO.
- El `2026-06-25`, la página oficial ofrecía binarios Windows de MAME `0.288`,
  mientras que la documentación publicada aparecía como `0.289`. Dogu no debe
  asumir que ambas numeraciones coinciden siempre.

### Decisión de implementación

Dogu expone conversión a CHD para:

- `.cue`
- `.gdi`
- `.toc`
- `.iso`
- `.bin` cuando existe un `.cue` hermano

Esto cubre los flujos de ROMs de disco más habituales, especialmente PlayStation
basado en `.cue` + `.bin`.

Dogu no expone CDI como fuente fiable para CHD porque CDI puede ser un formato
con pérdida y no es una entrada segura para preservación.

### Integración actual

- Windows incluye `chdman 0.288 (mame0288)` en
  `third_party/chdman/windows/chdman.exe`.
- Se conserva el fichero `COPYING` del paquete oficial.
- Linux incluye `third_party/chdman/linux/chdman`, compilado desde la fuente
  oficial de MAME `0.288` en Ubuntu/WSL.
- MAME no publica un binario Linux equivalente en su página oficial, así que la
  distribución Linux de Dogu se basa en compilación desde fuente.
- La ejecución real está verificada en Windows y Ubuntu/WSL.

### Limitación con rutas en Windows

Se ha verificado que `chdman` no es seguro con rutas no ASCII en Windows. Dogu lo
rodea ejecutando `chdman` desde una carpeta de trabajo ASCII-safe con nombres
relativos, y moviendo después el resultado con APIs de Rust cuando hace falta.
Los nombres inseguros para `chdman` se bloquean o se ofrecen para renombrado sin
acentos antes de ejecutar la cola.
