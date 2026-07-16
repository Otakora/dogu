# Release channels and updates

English is the primary language for this technical document. Spanish follows in
the second half.

## English

Dogu exposes two user-facing update channels:

- `Stable`: recommended official releases.
- `Beta`: builds prepared from `dev` so recent work can be tested earlier.

Automatic update checks are enabled by default. Users can change the channel from
Settings. When a user moves from Beta back to Stable, Dogu can offer the latest
stable release even if that is a downgrade from the current beta build.

### Stable

- Expected source branch: `main`.
- Version shape: clean semver, for example `0.2.5`.
- Tag: `v0.2.5`.
- GitHub Release: `Dogu 0.2.5`.
- Updater manifest: `latest.json`.
- Dogu endpoint: `https://github.com/Otakora/dogu/releases/latest/download/latest.json`.
- winget: stable only, package identifier `Otakora.Dogu`.

Stable releases must have a matching `CHANGELOG.md` section.

### Beta

- Expected source branch: `dev`.
- Version shape: semver prerelease with `-beta.N`, for example `0.2.6-beta.1`.
- Public release tag: rolling `beta`.
- GitHub Release: `Dogu Beta 0.2.6-beta.1`.
- Updater manifest: `latest-beta.json`.
- Dogu endpoint: `https://github.com/Otakora/dogu/releases/download/beta/latest-beta.json`.
- winget: beta is not submitted to winget.

The release workflow keeps a rolling `beta` release so Dogu has a fixed URL for
beta update checks. When a beta is published, the `beta` tag is moved to the
published commit. This only happens when the release workflow is run with
publishing enabled.

Beta releases may use an exact changelog section (`## 0.2.6-beta.1`) or the base
section (`## 0.2.6`) when several betas share the same short summary.

### Update support matrix

| Package | Update notice in Dogu | Automatic installation in Dogu | Channels |
| --- | --- | --- | --- |
| Windows NSIS | Yes | Yes | Stable/Beta |
| Linux AppImage | Yes | Yes | Stable/Beta |
| Linux DEB | Yes | No, open release and install manually | Stable/Beta |
| Linux Flatpak | Yes | No, open release and install manually | Stable/Beta |

Dogu signs updater artifacts for NSIS and AppImage. DEB and Flatpak remain
official packages, but they do not self-install through Dogu until a proper APT,
Flatpak repository or equivalent hosted update path exists.

### GitHub secrets

Official update publishing requires:

- `DOGU_UPDATER_PUBLIC_KEY`: public key compiled into Dogu for update validation.
- `TAURI_SIGNING_PRIVATE_KEY`: private key or key path/content used by Tauri.
- `TAURI_SIGNING_PRIVATE_KEY_PASSWORD`: private key password. Official release
  keys should be password-protected.

The private key must never be committed.

Do not rotate the official signing key casually. Once users have installed a
release that trusts a public key, updates signed with a different private key are
not trusted by that installation. Development keys are disposable; official
release keys are not.

Stable and Beta must use the same official keypair so users can move between
channels without breaking update trust.

See [updater-signing.md](updater-signing.md) for generation, storage and GitHub
Secrets commands.

### Release checklist

1. Update `package.json`, `src-tauri/Cargo.toml` and `src-tauri/tauri.conf.json`.
2. Add a short bilingual section to `CHANGELOG.md`.
3. For beta, work from `dev` and use `x.y.z-beta.N`.
4. For stable, promote approved work to `main` and use `x.y.z`.
5. Run the `Release Artifacts` workflow manually or push `v<version>`.
6. For stable, validate the generated winget manifest before submitting it to
   `microsoft/winget-pkgs`.

## Español

Dogu expone dos canales de actualización:

- `Stable`: releases oficiales recomendadas.
- `Beta`: builds preparadas desde `dev` para probar cambios recientes antes.

La comprobación automática viene activada por defecto. Si el usuario cambia de
Beta a Stable, Dogu puede ofrecer la última stable aunque sea un downgrade frente
a la beta actual.

### Stable

- Rama esperada: `main`.
- Versión: semver limpio, por ejemplo `0.2.5`.
- Tag: `v0.2.5`.
- GitHub Release: `Dogu 0.2.5`.
- Manifest updater: `latest.json`.
- Endpoint: `https://github.com/Otakora/dogu/releases/latest/download/latest.json`.
- winget: solo stable, identificador `Otakora.Dogu`.

Stable debe tener sección equivalente en `CHANGELOG.md`.

### Beta

- Rama esperada: `dev`.
- Versión: semver prerelease con `-beta.N`, por ejemplo `0.2.6-beta.1`.
- Tag público: `beta`.
- GitHub Release: `Dogu Beta 0.2.6-beta.1`.
- Manifest updater: `latest-beta.json`.
- Endpoint: `https://github.com/Otakora/dogu/releases/download/beta/latest-beta.json`.
- winget: beta no se envía a winget.

El workflow mantiene una release rolling `beta` para tener una URL fija. Cuando
se publica beta, el tag `beta` se mueve al commit publicado.

### Matriz de soporte

| Paquete | Aviso en Dogu | Instalación automática en Dogu | Canales |
| --- | --- | --- | --- |
| Windows NSIS | Sí | Sí | Stable/Beta |
| Linux AppImage | Sí | Sí | Stable/Beta |
| Linux DEB | Sí | No, abrir release e instalar manualmente | Stable/Beta |
| Linux Flatpak | Sí | No, abrir release e instalar manualmente | Stable/Beta |

NSIS y AppImage son los formatos autoactualizables mediante Dogu. DEB y Flatpak
pueden avisar, pero requieren instalación manual mientras no exista repositorio
APT/Flatpak propio o integración equivalente.

La clave privada oficial no debe rotarse de forma casual. Cuando una instalación
confía en una clave pública, no aceptará updates firmados con otra clave privada.
Las claves de desarrollo son desechables; las oficiales no.

Stable y Beta deben usar el mismo par de claves oficial para que el usuario pueda
moverse entre canales sin romper la confianza del updater.

Consulta [updater-signing.md](updater-signing.md) para generación, custodia y
comandos de GitHub Secrets.

### Checklist breve

1. Actualizar las tres versiones del proyecto.
2. Añadir changelog bilingüe y muy resumido.
3. Usar `x.y.z-beta.N` para beta desde `dev`.
4. Usar `x.y.z` para stable desde `main`.
5. Ejecutar el workflow de release o subir `v<version>`.
6. En stable, validar el manifest winget antes de enviarlo.
