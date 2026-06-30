# Notas de integracion de chdman

Consulta hecha el `2026-06-25`.

## Fuentes oficiales

- Documentacion oficial de `chdman`:
  [https://docs.mamedev.org/tools/chdman.html](https://docs.mamedev.org/tools/chdman.html)
- Pagina oficial de descargas de MAME:
  [https://www.mamedev.org/release.html](https://www.mamedev.org/release.html)
- Repositorio oficial de MAME:
  [https://github.com/mamedev/mame](https://github.com/mamedev/mame)

## Observaciones utiles para esta app

- La documentacion oficial indica que `chdman` sirve para crear, convertir, verificar y extraer imagenes CHD.
- Los comandos oficiales relevantes para esta app son `createcd` y `createdvd`.
- La documentacion oficial no enumera por extension todos los formatos de entrada, pero el codigo fuente oficial de `chdman` deja clara la presencia de flujos `CUE/BIN`, `GDI` y `TOC`, ademas del flujo de `ISO` para `createdvd`.
- La pagina oficial de lanzamientos mostraba el `2026-06-25` la version oficial `0.288` en binarios Windows, mientras que la documentacion publicada estaba versionada como `0.289`. La app no debe asumir que ambas numeraciones coinciden siempre.

## Decision de implementacion en esta base

Para no prometer mas de lo que el flujo actual puede resolver bien, la app expone conversion a CHD en estos casos:

- `.cue`
- `.gdi`
- `.toc`
- `.iso`
- `.bin` con `.cue` hermano

Esto cubre los casos de ROMs de disco mas habituales y, en especial, el caso de PlayStation basado en `.cue` + `.bin`.

## Integracion real en este proyecto

- Se ha integrado el binario oficial de Windows `chdman 0.288 (mame0288)` en `third_party/chdman/windows/chdman.exe`.
- Se ha copiado tambien el fichero `COPYING` del paquete oficial para conservar la referencia legal asociada.
- Para Linux, se ha generado `third_party/chdman/linux/chdman` compilando desde la fuente oficial de MAME `0.288` dentro de Ubuntu WSL.
- La pagina oficial de lanzamientos no publica un binario oficial Linux equivalente, asi que en Linux la distribucion del proyecto se basa en compilacion desde fuente oficial.
- La ejecucion real ha quedado verificada tanto en Windows como en Ubuntu WSL.
