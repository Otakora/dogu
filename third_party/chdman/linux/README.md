# Linux chdman binary

## English

Expected application path:

- `third_party/chdman/linux/chdman`

Current status:

- Dogu already includes a functional Linux binary at
  `third_party/chdman/linux/chdman`.
- The official MAME `0.288` release page publishes Windows binaries and source
  code, but not an equivalent official Linux binary.
- MAME's documented Linux path is to compile from source with `TOOLS=1`; the
  script in this directory automates that route.
- The binary produced in Ubuntu/WSL links dynamically against `SDL2` and common
  Linux graphics/audio runtime libraries, so another distro must provide those
  runtime libraries.

To populate this path on a Linux machine:

```bash
bash third_party/chdman/linux/install_official_chdman.sh
```

## Español

Ruta esperada por la aplicación:

- `third_party/chdman/linux/chdman`

Estado actual:

- Dogu ya incluye un binario Linux funcional en esa ruta.
- La página oficial de MAME `0.288` publica binarios Windows y código fuente,
  pero no un binario Linux equivalente.
- La vía documentada por MAME en Linux es compilar desde fuente con `TOOLS=1`; el
  script de esta carpeta automatiza ese camino.
- El binario generado en Ubuntu/WSL enlaza dinámicamente con `SDL2` y librerías
  gráficas/audio habituales de Linux, por lo que otra distro debe aportar esas
  runtime libraries.

Para poblar la ruta en Linux:

```bash
bash third_party/chdman/linux/install_official_chdman.sh
```
