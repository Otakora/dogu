# Building and releasing Dogu

This document is the technical build reference. English is the primary language
for maintenance and external collaboration; a Spanish operational summary is
included below.

## English

### Project stack

Dogu is built with Tauri 2, Svelte 5, Tailwind CSS v4, Rust and Node.js.

The canonical build strategy is:

1. Build Windows artifacts on Windows.
2. Build Linux artifacts on Linux.
3. Let CI produce official release artifacts reproducibly.

Cross-compilation is not the primary release path.

Official GitHub releases currently produce:

- Windows NSIS installer.
- Linux AppImage.
- Linux DEB.
- Linux Flatpak bundle.

The AppImage is part of the official flow because Tauri reuses it as the Linux
self-update artifact.

### Requirements

- Node.js 20.19+ or 22.12+.
- Rust stable toolchain.
- Python 3.12+ for release tooling.
- `npm`.
- Git LFS materialized for bundled third-party binaries.

### Local development

```powershell
npm install
npm run updater:generate-dev-key
npm run tauri:dev
```

`npm run tauri:dev` launches Tauri and Vite on `127.0.0.1:1420`. It also loads
the local development updater public key from `.dev-secrets/dogu-updater.key.pub`
and injects it into Tauri with `--config` so the updater plugin can initialize
without manual environment variables.

Useful checks:

| Command | Purpose |
| --- | --- |
| `npm run check` | Run `svelte-check`. |
| `npm run build` | Build the frontend into `dist/`. |
| `cargo check` | Check the Rust backend from `src-tauri/`. |
| `cargo test --manifest-path src-tauri/Cargo.toml` | Run Rust tests. |

`npm run tauri:build` exists, but release artifacts should normally be produced
through `tools/release.py` so files are renamed, sidecars are validated and
release metadata is generated consistently.

### Development updater keys

Development keys live in `.dev-secrets/`, which is ignored by Git.

```powershell
npm run updater:generate-dev-key
```

Generated files:

- `.dev-secrets/dogu-updater.key`: private development key. Do not commit it.
- `.dev-secrets/dogu-updater.key.pub`: public development key used by `tauri:dev`.

To rotate the local development key:

```powershell
npm run updater:generate-dev-key -- --force
```

These keys are only for local development. Official releases must use GitHub
secrets.

For contributors, no official secret is required. `npm run tauri:dev` generates
a local development keypair automatically when `.dev-secrets/` is missing.

### Official updater signing

Tauri updater artifacts must be signed. GitHub release builds require:

- `DOGU_UPDATER_PUBLIC_KEY`: public key compiled into Dogu for update checks.
- `TAURI_SIGNING_PRIVATE_KEY`: private key or private-key path/content used by Tauri.
- `TAURI_SIGNING_PRIVATE_KEY_PASSWORD`: private key password. It is technically
  optional for unprotected keys, but official Dogu release keys should use one.

`tools/release.py` also injects `DOGU_UPDATER_PUBLIC_KEY` into Tauri with
`--config`, because the updater plugin requires `plugins.updater.pubkey` while
the app starts.

The private key must never be committed.

Treat the official private key as long-lived release infrastructure. If it is
rotated after public releases exist, already-installed Dogu builds that trust the
old public key will not accept updates signed with the new private key. Rotation
is still possible, but it requires a planned migration or a manual reinstall path.

To generate an official key pair outside the repository, use Tauri's interactive
signer and choose a strong password when prompted:

```powershell
node .\node_modules\@tauri-apps\cli\tauri.js signer generate -w "$env:USERPROFILE\.tauri\dogu-updater.key"
```

### Windows build

Run on Windows:

```powershell
python tools/release.py build-windows
```

Expected output:

- `dist/dogu-windows-x64-<version>-setup.exe`
- `dist/dogu-windows-x64-<version>-setup.exe.sig`

The script verifies bundled sidecars, installs Node dependencies when needed,
runs `tauri build --bundles nsis`, and copies the installer plus updater
signature to `dist/`.

### Linux native build

Run on Linux:

```bash
python3 tools/release.py build-linux-native
```

Expected output:

- `dist/dogu-linux-x86_64-<version>.AppImage`
- `dist/dogu-linux-x86_64-<version>.AppImage.sig`
- `dist/dogu-linux-x86_64-<version>.deb`

Common Debian/Ubuntu dependencies:

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

`libfuse2t64` is needed by the AppImage tooling on Ubuntu 24.04+. SMB support
links against the system `libsmbclient`.

### Linux Flatpak build

Run on Linux:

```bash
python3 tools/release.py build-linux-flatpak
```

Expected output:

- `dist/dogu-linux-x86_64-<version>.flatpak`

Additional dependencies:

```bash
sudo apt install -y flatpak flatpak-builder
```

The Flatpak flow builds the release binary first and then creates the Flatpak
bundle with `org.freedesktop.Platform//24.08`, `node20` and `rust-stable`
extensions. The Flatpak wrapper sets `DOGU_RESOURCES_DIR` so Dogu can find its
bundled helper tools.

### CI

Dogu uses two GitHub workflows:

| Workflow | Trigger | Purpose |
| --- | --- | --- |
| `.github/workflows/ci.yml` | PRs and pushes to `main`/`dev` | Validation only. |
| `.github/workflows/build-artifacts.yml` | Manual dispatch or `v*` tags | Build, package and optionally publish releases. |

The release workflow validates:

- `package.json`, `src-tauri/Cargo.toml` and `src-tauri/tauri.conf.json` share the same version.
- A `v*` tag matches that version exactly.
- Updater signing secrets exist.
- `CHANGELOG.md` contains a matching release section.

Release artifacts include the installers/packages, updater manifest,
`SHA256SUMS.txt`, GitHub release notes and, for stable releases, winget metadata.

### Stable and Beta channels

| Channel | Version shape | Release target | Manifest |
| --- | --- | --- | --- |
| Stable | `0.2.5` | `v0.2.5` | `latest.json` |
| Beta | `0.2.6-beta.1` | rolling `beta` tag/release | `latest-beta.json` |

Stable is the recommended channel and should be promoted through `main`. Beta is
built from `dev` for earlier testing. If a user switches from Beta back to
Stable, Dogu can offer the latest stable release as a downgrade.

See [release-channels.md](release-channels.md) for the full policy.
See [updater-signing.md](updater-signing.md) for key custody and GitHub Secrets.

### winget

The Windows Package Manager identifier is `Otakora.Dogu`.

The release tooling generates `Otakora.Dogu.yaml` for stable releases only. Before
submitting it to `microsoft/winget-pkgs`, validate it:

```powershell
winget validate .\Otakora.Dogu.yaml
```

winget improves the install/update path and hash validation, but it is not a code
signing certificate. SmartScreen reputation may still take time to improve.

### Bundled sidecars

Third-party helper binaries are stored under `third_party/` and packaged as Tauri
resources:

| Tool | Windows | Linux |
| --- | --- | --- |
| chdman | `third_party/chdman/windows/chdman.exe` | `third_party/chdman/linux/chdman` |
| 7-Zip | `third_party/7zip/windows/7z.exe` + `7z.dll`, plus legacy `7za.exe` + `7za.dll` | `third_party/7zip/linux/7zz` |
| DolphinTool | `third_party/dolphin-tool/windows/DolphinTool.exe` | `third_party/dolphin-tool/linux/DolphinTool` |

These binaries are managed through Git LFS. Release tooling fails if it detects
LFS pointer files instead of real binaries.

## Español

### Enfoque

Dogu usa Tauri 2, Svelte 5, Tailwind CSS v4, Rust y Node.js.

La estrategia de build es:

1. Windows se compila en Windows.
2. Linux se compila en Linux.
3. CI genera los artefactos oficiales de forma reproducible.

Los artefactos oficiales son Windows NSIS, Linux AppImage, Linux DEB y Linux
Flatpak. AppImage forma parte del flujo oficial porque Tauri lo usa para la
autoactualización en Linux.

### Desarrollo local

```powershell
npm install
npm run updater:generate-dev-key
npm run tauri:dev
```

`npm run tauri:dev` carga automáticamente la clave pública de desarrollo desde
`.dev-secrets/dogu-updater.key.pub` y la inyecta en Tauri con `--config`. La
clave privada queda en `.dev-secrets/`, que está ignorado por Git.

Comandos útiles:

| Comando | Uso |
| --- | --- |
| `npm run check` | Validación Svelte/TypeScript. |
| `npm run build` | Build del frontend. |
| `cargo check` | Validación del backend Rust desde `src-tauri/`. |
| `cargo test --manifest-path src-tauri/Cargo.toml` | Tests Rust. |

### Claves updater

Para desarrollo:

```powershell
npm run updater:generate-dev-key
```

Para regenerarlas:

```powershell
npm run updater:generate-dev-key -- --force
```

Los colaboradores no necesitan las claves oficiales. Si `.dev-secrets/` no
existe, `npm run tauri:dev` genera automáticamente un par local de desarrollo.

Para releases oficiales hay que configurar secretos en GitHub:

- `DOGU_UPDATER_PUBLIC_KEY`
- `TAURI_SIGNING_PRIVATE_KEY`
- `TAURI_SIGNING_PRIVATE_KEY_PASSWORD`

La clave privada oficial no debe entrar nunca en Git.

Aunque Tauri permite claves sin password, las claves oficiales de Dogu deben
tener una contraseña fuerte. Las claves locales de desarrollo sí pueden ser
desechables y sin password.

La clave privada oficial debe tratarse como infraestructura de release de larga
vida. Si se rota después de publicar versiones, las instalaciones que confían en
la clave pública anterior no aceptarán updates firmados con la clave nueva salvo
que exista una migración planificada o reinstalación manual.

### Builds de release

Windows:

```powershell
python tools/release.py build-windows
```

Linux nativo:

```bash
python3 tools/release.py build-linux-native
```

Flatpak:

```bash
python3 tools/release.py build-linux-flatpak
```

Usar `tools/release.py` para distribución, no `npm run tauri:build`, porque el
script valida sidecars, renombra artefactos y prepara firmas/metadatos.

### CI, canales y winget

`ci.yml` valida PRs y pushes a `main`/`dev`. `build-artifacts.yml` solo construye
releases al ejecutarse manualmente o con tags `v*`.

Stable usa versiones limpias como `0.2.5`, release `v0.2.5` y `latest.json`.
Beta usa versiones como `0.2.6-beta.1`, release rolling `beta` y
`latest-beta.json`.

El identificador winget es `Otakora.Dogu` y solo se genera para stable.

Más detalle en [release-channels.md](release-channels.md).
La custodia de claves y secrets está en [updater-signing.md](updater-signing.md).
