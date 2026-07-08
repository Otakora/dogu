# Build y distribucion

## Enfoque

La base actual del proyecto es `Tauri 2` + Svelte 5 + Tailwind CSS v4.

La estrategia canonica de compilacion es:

1. Windows se compila en Windows.
2. Linux se compila en Linux.
3. CI genera ambos artefactos de manera reproducible.

No forzamos cross-compilacion entre Windows y Linux como ruta principal.

Para las releases oficiales automatizadas en GitHub estamos fijando, por ahora, el conjunto estable de artefactos: `Windows + Linux DEB + Linux Flatpak`. El AppImage sigue disponible en local, pero no forma parte del flujo oficial hasta resolver un fallo especifico de `linuxdeploy` en el runner de Ubuntu.

## Requisitos generales

- Node.js 20+
- Rust toolchain estable (`rustup toolchain install stable`)
- Python 3.12+ (solo para los scripts de release)
- `npm`

---

## Desarrollo local

```powershell
npm install
npm run tauri:dev
```

Esto arranca el backend Rust y el servidor Vite en `127.0.0.1:1420`. La ventana de la app se abre automaticamente con hot-reload activo en el frontend.

### Comandos utiles durante el desarrollo

| Comando | Que hace |
|---------|----------|
| `npm run tauri:dev` | App completa con hot-reload — el comando habitual |
| `npm run dev` | Solo el frontend Vite en el navegador (sin Tauri; los `invoke()` fallan) |
| `npm run check` | `svelte-check` — verifica tipos en todos los `.svelte` y `.ts` |
| `npm run build` | Compila el frontend a `dist/` (Tauri lo llama internamente) |
| `cargo check` | Verifica compilacion del backend Rust (ejecutar desde `src-tauri/`) |

> **`npm run tauri:build`** existe pero produce artefactos en `src-tauri/target/release/bundle/`
> sin mover nada a `dist/` ni aplicar permisos de sidecars. Usarlo solo si sabes lo que haces;
> para distribucion usa siempre `tools/release.py`.

---

## Build Windows

Ejecutar en Windows:

```powershell
python tools/release.py build-windows
```

Salida esperada:

- `dist/dogu-windows-x64-<version>-setup.exe`

El script instala dependencias npm si es necesario, lanza `tauri build --bundles nsis` y mueve el instalador a `dist/`.

---

## Build Linux nativo

Ejecutar en Linux:

```bash
python3 tools/release.py build-linux-native
```

Salida esperada:

- `dist/dogu-linux-x86_64-<version>.AppImage`
- `dist/dogu-linux-x86_64-<version>.deb`

El script aplica permisos de ejecucion a los sidecars (`chdman`, `7zz`) antes de compilar.

Si solo necesitas el paquete DEB, puedes usar:

```bash
python3 tools/release.py build-linux-deb
```

Salida esperada:

- `dist/dogu-linux-x86_64-<version>.deb`

Dependencias de sistema habituales para Tauri en Debian/Ubuntu:

```bash
sudo apt update
sudo apt install -y \
  build-essential \
  curl \
  wget \
  file \
  libfuse2t64 \
  libsmbclient \
  libsmbclient-dev \
  pkg-config \
  libgtk-3-dev \
  libwebkit2gtk-4.1-dev \
  libsoup-3.0-dev \
  libayatana-appindicator3-dev \
  librsvg2-dev \
  patchelf
```

`libfuse2t64` permite ejecutar `linuxdeploy` y sus plugins AppImage en Ubuntu 24.04+, que es lo que usa Tauri durante el bundle AppImage. `libsmbclient` y `libsmbclient-dev` son necesarios porque el backend SMB enlaza contra la libreria del sistema en Linux. El resto de paquetes cubre el toolchain habitual de Tauri para GTK/WebKit y el empaquetado AppImage.

---

## Build Linux Flatpak

Ejecutar en Linux:

```bash
python3 tools/release.py build-linux-flatpak
```

Salida esperada:

- `dist/dogu-linux-x86_64-<version>.flatpak`

Notas importantes:

- El script compila primero el binario release de Tauri sin bundles y despues construye el bundle Flatpak sobre ese resultado.
- Requiere `flatpak` y `flatpak-builder` instalados.
- Usa el runtime `org.freedesktop.Platform//24.08` y las extensiones `node20` y `rust-stable`.
- El wrapper Flatpak fija `DOGU_RESOURCES_DIR` para que la app localice `chdman` y `7-Zip` integrados.

Dependencias adicionales para Flatpak:

```bash
sudo apt install -y flatpak flatpak-builder
```

---

## CI

El proyecto usa dos workflows separados:

### 1. CI de validacion

`/.github/workflows/ci.yml` se ejecuta en cada `pull_request` y en cada `push` a `main` o `dev`.

Comprueba:

| Job | Verificacion |
|-----|--------------|
| `frontend` | `npm run check` |
| `rust-windows` | `cargo check` + `cargo test --lib remote::tests -- --nocapture` |
| `rust-linux` | `cargo check` + `cargo test --lib remote::tests -- --nocapture` |

El job de Linux instala solo las dependencias necesarias para validar el backend Tauri, sin intentar empaquetar instaladores.

### 2. Release oficial

`/.github/workflows/build-artifacts.yml` ya no corre en cada push a `main`.

Ahora se ejecuta:

- manualmente desde GitHub Actions (`workflow_dispatch`)
- automaticamente al subir un tag `v*` como `v0.2.3`

En ese flujo se generan los instaladores y paquetes de distribucion:

| Job | Artefacto |
|-----|-----------|
| `windows` | `dogu-windows-x64-<version>-setup.exe` |
| `linux-native` | `dogu-linux-x86_64-<version>.AppImage` + `dogu-linux-x86_64-<version>.deb` |
| `linux-flatpak` | `dogu-linux-x86_64-<version>.flatpak` |

Cuando el trigger es un tag `v*`, los artefactos generados se adjuntan automaticamente a la GitHub Release correspondiente.

La `<version>` se toma automaticamente del `version` del proyecto, priorizando `src-tauri/Cargo.toml` y usando `package.json` como respaldo.

Por ahora los jobs Linux estan marcados como no bloqueantes en el flujo de release para no impedir publicar una version oficial mientras se estabiliza el empaquetado Linux.

---

## Sidecars

Los binarios de terceros se incluyen en `third_party/` y Tauri los empaqueta como sidecars:

| Binario | Windows | Linux |
|---------|---------|-------|
| chdman | `third_party/chdman/chdman.exe` | `third_party/chdman/linux/chdman` |
| 7-Zip | `third_party/7zip/windows/7z.exe` + `7z.dll` para RAR; `7za.exe` + `7za.dll` legado | `third_party/7zip/linux/7zz` |

En Linux el script de release aplica `chmod +x` a estos binarios antes de compilar. En Windows no es necesario.

La extraccion `.rar` debe funcionar sin depender de una instalacion externa de
7-Zip. Por eso el build de Windows comprueba que `7z.exe` y `7z.dll` esten
presentes antes de generar el instalador. `7za.exe` por si solo no basta para
listar ni extraer RAR modernos.

La compresion `.rar` no se empaqueta con Dogu. Se habilita solo cuando el
sistema del usuario aporta `rar`/WinRAR.
