# Build y distribucion

## Enfoque

La base actual del proyecto es `Tauri 2`.

La estrategia canonica de compilacion es:

1. Windows se compila en Windows.
2. Linux se compila en Linux.
3. CI genera ambos artefactos de manera reproducible.

No forzamos cross-compilacion entre Windows y Linux como ruta principal.

## Requisitos generales

- Node.js 20+
- Rust toolchain estable
- `npm`

## Build Windows

Ejecutar en Windows:

```powershell
python .\tools\release.py build-windows
```

Salida esperada:

- `dist/dogu-windows-x64-setup.exe`

El script lanza build Tauri con bundle `NSIS`.

## Build Linux nativo

Ejecutar en Linux:

```bash
python3 tools/release.py build-linux-native
```

Salida esperada:

- `dist/dogu-linux-x86_64.AppImage`
- `dist/dogu-linux-x86_64.deb`

Dependencias de sistema habituales para Tauri en Debian/Ubuntu:

```bash
sudo apt update
sudo apt install -y \
  build-essential \
  curl \
  wget \
  file \
  pkg-config \
  libgtk-3-dev \
  libwebkit2gtk-4.1-dev \
  libsoup-3.0-dev \
  libayatana-appindicator3-dev \
  librsvg2-dev \
  patchelf
```

## Build Linux Flatpak

Ejecutar en Linux:

```bash
python3 tools/release.py build-linux-flatpak
```

Salida esperada:

- `dist/dogu-linux-x86_64.flatpak`

Notas importantes:

- El script genera primero la build Linux nativa y despues prepara el bundle Flatpak.
- Flatpak se apoya en el runtime `org.freedesktop.Platform//24.08`.
- El wrapper Flatpak fija `DOGU_RESOURCES_DIR` para que la app localice `chdman` y `7-Zip` integrados.

## CI

El workflow del repo genera:

- Instalador Windows `NSIS`
- Linux `AppImage`
- Linux `DEB`

Y deja preparado un job separado para `Flatpak`.

## Sidecars

Antes de empaquetar en Linux, el script asegura permisos de ejecucion para:

- `third_party/chdman/linux/chdman`
- `third_party/7zip/linux/7zz`
