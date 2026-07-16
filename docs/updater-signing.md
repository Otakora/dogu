# Updater signing keys and GitHub Secrets

English is the primary language for this technical document. Spanish follows
below.

## English

Dogu uses Tauri's updater signing system. Every update artifact must be signed
with a private key, and installed Dogu builds verify updates with the matching
public key.

Stable and Beta must use the same official keypair. This lets users move between
channels without breaking updater trust.

## Key types

| Item | Secret? | Stored where | Purpose |
| --- | --- | --- | --- |
| Official public key | No | GitHub Secret `DOGU_UPDATER_PUBLIC_KEY`, maintainer vault | Compiled into Dogu and injected into Tauri config so updates can be verified. |
| Official private key | Yes | GitHub Secret `TAURI_SIGNING_PRIVATE_KEY`, maintainer vault | Signs NSIS/AppImage updater artifacts in CI. |
| Official private key password | Yes | GitHub Secret `TAURI_SIGNING_PRIVATE_KEY_PASSWORD`, maintainer vault | Unlocks the password-protected private key during release builds. |
| Local development keypair | Private key is local only | `.dev-secrets/` | Lets contributors run `npm run tauri:dev` without official secrets. |

Never commit official private keys, passwords, `.dev-secrets/`, or copied secret
values.

## Generating the official keypair

Generate the official key outside the repository's ignored development folder.
Choose a strong password when prompted.

```powershell
$OfficialDir = Join-Path $env:USERPROFILE ".tauri"
$OfficialKey = Join-Path $OfficialDir "dogu-official-updater.key"
New-Item -ItemType Directory -Force -Path $OfficialDir | Out-Null

node .\node_modules\@tauri-apps\cli\tauri.js signer generate -w $OfficialKey

$OfficialPub = "$OfficialKey.pub"
Get-Item $OfficialKey, $OfficialPub
```

Store these three values in the maintainer vault:

```powershell
Get-Content -LiteralPath $OfficialPub -Raw
Get-Content -LiteralPath $OfficialKey -Raw
```

Also store the password entered during key generation.

## Configuring GitHub Secrets

Run these commands from the repository root after generating the official keypair.
They upload the values as encrypted GitHub Actions secrets.

```powershell
Get-Content -LiteralPath $OfficialPub -Raw |
  gh secret set DOGU_UPDATER_PUBLIC_KEY --repo Otakora/dogu

Get-Content -LiteralPath $OfficialKey -Raw |
  gh secret set TAURI_SIGNING_PRIVATE_KEY --repo Otakora/dogu

$SecurePassword = Read-Host "Updater signing password" -AsSecureString
$Bstr = [Runtime.InteropServices.Marshal]::SecureStringToBSTR($SecurePassword)
try {
  [Runtime.InteropServices.Marshal]::PtrToStringBSTR($Bstr) |
    gh secret set TAURI_SIGNING_PRIVATE_KEY_PASSWORD --repo Otakora/dogu
} finally {
  [Runtime.InteropServices.Marshal]::ZeroFreeBSTR($Bstr)
}

gh secret list --repo Otakora/dogu
```

`TAURI_SIGNING_PRIVATE_KEY_PASSWORD` is the password used to protect
`dogu-official-updater.key`. It is not the private key and it is not the public
key.

## Development contributors

Contributors do not need the official keys. `npm run tauri:dev` automatically
generates a local keypair in `.dev-secrets/` when it is missing. Those keys are
disposable and ignored by Git.

To rotate local development keys:

```powershell
npm run updater:generate-dev-key -- --force
```

## Rotation policy

Development keys may be regenerated freely.

Official keys should not be rotated casually after a public updater-enabled
release exists. Installed Dogu builds trust the public key compiled into them; if
future releases are signed with a different private key, those installations will
reject the update.

If official key rotation is ever required, plan a migration:

1. Publish a final update signed by the old key that can trust the new update
   metadata path or otherwise guide the user.
2. Document manual reinstall instructions for users who cannot update in place.
3. Keep old-key backups until the migration window is complete.

## Español

Dogu usa el sistema de firma del updater de Tauri. Cada artefacto de update debe
firmarse con una clave privada, y Dogu instalado verifica esos updates con la
clave pública correspondiente.

Stable y Beta deben usar el mismo par de claves oficial. Así el usuario puede
moverse entre canales sin romper la confianza del updater.

## Tipos de claves

| Elemento | ¿Secreto? | Dónde se guarda | Para qué sirve |
| --- | --- | --- | --- |
| Clave pública oficial | No | GitHub Secret `DOGU_UPDATER_PUBLIC_KEY`, vault del maintainer | Se compila en Dogu y se inyecta en la config Tauri para verificar updates. |
| Clave privada oficial | Sí | GitHub Secret `TAURI_SIGNING_PRIVATE_KEY`, vault del maintainer | Firma artefactos NSIS/AppImage en CI. |
| Password de la clave privada oficial | Sí | GitHub Secret `TAURI_SIGNING_PRIVATE_KEY_PASSWORD`, vault del maintainer | Desbloquea la clave privada durante builds de release. |
| Claves locales de desarrollo | La privada solo local | `.dev-secrets/` | Permiten ejecutar `npm run tauri:dev` sin secrets oficiales. |

Nunca commitear claves privadas oficiales, passwords, `.dev-secrets/` ni copias
de valores secretos.

## Generar la clave oficial

Generar la clave oficial fuera de `.dev-secrets/`. Cuando la CLI pregunte, usar
una contraseña fuerte.

```powershell
$OfficialDir = Join-Path $env:USERPROFILE ".tauri"
$OfficialKey = Join-Path $OfficialDir "dogu-official-updater.key"
New-Item -ItemType Directory -Force -Path $OfficialDir | Out-Null

node .\node_modules\@tauri-apps\cli\tauri.js signer generate -w $OfficialKey

$OfficialPub = "$OfficialKey.pub"
Get-Item $OfficialKey, $OfficialPub
```

Guardar en el vault:

```powershell
Get-Content -LiteralPath $OfficialPub -Raw
Get-Content -LiteralPath $OfficialKey -Raw
```

Guardar también la password usada al generar la clave.

## Configurar GitHub Secrets

```powershell
Get-Content -LiteralPath $OfficialPub -Raw |
  gh secret set DOGU_UPDATER_PUBLIC_KEY --repo Otakora/dogu

Get-Content -LiteralPath $OfficialKey -Raw |
  gh secret set TAURI_SIGNING_PRIVATE_KEY --repo Otakora/dogu

$SecurePassword = Read-Host "Updater signing password" -AsSecureString
$Bstr = [Runtime.InteropServices.Marshal]::SecureStringToBSTR($SecurePassword)
try {
  [Runtime.InteropServices.Marshal]::PtrToStringBSTR($Bstr) |
    gh secret set TAURI_SIGNING_PRIVATE_KEY_PASSWORD --repo Otakora/dogu
} finally {
  [Runtime.InteropServices.Marshal]::ZeroFreeBSTR($Bstr)
}

gh secret list --repo Otakora/dogu
```

`TAURI_SIGNING_PRIVATE_KEY_PASSWORD` es la contraseña que protege
`dogu-official-updater.key`. No es la clave privada ni la clave pública.

## Colaboradores

Los colaboradores no necesitan las claves oficiales. `npm run tauri:dev` genera
automáticamente un par local en `.dev-secrets/` si no existe. Son claves
desechables e ignoradas por Git.

## Rotación

Las claves de desarrollo pueden regenerarse libremente.

Las claves oficiales no deberían rotarse sin planificación después de publicar
una release con updater. Las instalaciones existentes confían en la clave pública
compilada en ellas y rechazarán updates firmados con otro par de claves.
