# Linux

Ruta esperada por la aplicacion:

- `third_party/chdman/linux/chdman`

Estado actual:

- El proyecto ya incluye un binario Linux funcional en `third_party/chdman/linux/chdman`.
- La pagina oficial de lanzamiento de MAME `0.288` publica binarios oficiales de Windows y codigo fuente, pero no un binario oficial equivalente para Linux.
- La via oficial documentada por MAME para Linux es compilar desde el codigo fuente con `TOOLS=1`, y el script de este directorio automatiza ese camino.
- El binario generado en Ubuntu WSL enlaza dinamicamente con `SDL2` y librerias graficas/audio habituales de Linux, por lo que en otra distro necesitara esas runtime libraries presentes.

Si quieres poblar esta ruta en una maquina Linux, usa:

```bash
bash third_party/chdman/linux/install_official_chdman.sh
```
