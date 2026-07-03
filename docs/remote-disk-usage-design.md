# Diseno tecnico: uso de disco en ubicaciones remotas

## Objetivo

Mostrar espacio total y espacio disponible para ubicaciones remotas con una UX comparable a la de discos locales, pero sin prometer datos falsos en protocolos que no ofrecen una forma portable y fiable de obtenerlos.

La meta no es "paridad absoluta entre protocolos", sino "mejor dato posible segun las capacidades reales del protocolo y del backend".

## Resumen ejecutivo

La estrategia recomendada es:

1. Priorizar metodos nativos del protocolo.
2. Usar fallbacks solo cuando la semantica siga siendo razonable.
3. Devolver datos parciales o "no disponible" cuando el protocolo no garantice una lectura fiable.
4. Cachear por sesion y ruta efectiva para evitar reconexiones y lecturas repetidas.

La implementacion debe empezar por:

1. `SFTP` con `statvfs@openssh.com`.
2. `SSH/SCP` con fallback `df -Pk` para hosts Unix-like con shell.
3. `SMB` en host Windows con `GetDiskFreeSpaceExW` sobre ruta UNC, solo si el path remoto puede resolverse con seguridad.
4. `FTP/FTPS` como no soportado por defecto.

## Lo que hace hoy la aplicacion

### Local

- Los discos locales se enumeran en `src-tauri/src/ops.rs` con `sysinfo`.
- El contrato actual es `VolumeDto` en `src-tauri/src/models.rs`.
- La sidebar muestra barra de uso y bytes libres para cada volumen local.

### Remoto

- La capa remota vive en `src-tauri/src/remote.rs`.
- Cada sesion activa guarda:
  - `profile`
  - `root_provider_path`
  - `fs: Box<dyn RemoteFs + Send>`
- Los protocolos soportados hoy son `ssh`, `smb`, `ftp` y `ftps`.
- El frontend solo renderiza conexiones remotas activas, no perfiles desconectados.

## Hallazgos externos relevantes

### WinSCP

- En el codigo de `SFTP` aparecen las extensiones `space-available` y `statvfs@openssh.com`.
- En `FTP` aparecen `AVBL` y `XQUOTA`.
- Esto sugiere una estrategia por protocolo y por capacidad, no un metodo universal.

### Cyberduck / Mountain Duck

- Documenta explicitamente que, para `SFTP`, el espacio libre se calcula usando:
  - `space-available`
  - `statvfs@openssh.com`
- Tambien documenta que el valor puede ser incorrecto si el servidor responde con el filesystem equivocado para la ruta montada.
- Tiene una opcion para desactivar esa funcionalidad.

### Conclusiones de mercado

- Los clientes maduros no usan una unica tecnica comun para todos los remotos.
- `SFTP` tiene una historia razonable gracias a extensiones del protocolo.
- `FTP/FTPS` depende de extensiones y comandos no universales.
- `SMB` no muestra una via claramente portable y uniforme entre plataformas host.

## Limitaciones tecnicas del stack actual

### `RemoteFs`

El trait base `RemoteFs` si expone `exec()`, pero:

- `remotefs-ssh` lo implementa para `sftp` y `scp`.
- `remotefs-ftp` y `remotefs-smb` devuelven `UnsupportedFeature`.

### `exec()` en `remotefs-ssh`

Aunque `exec()` existe, internamente ejecuta siempre `sh -c ...`.

Consecuencias:

- Sirve bien para Linux, BSD, macOS y NAS Unix-like.
- No es una base fiable para un servidor SSH Windows puro.
- No debe ser el mecanismo principal si existe una via SFTP nativa mejor.

### `ssh2`

La crate `ssh2` ya presente en el proyecto expone `statvfs`, pero sobre un handle SFTP abierto, no por ruta directa.

Esto significa:

- El modo nativo `SFTP` es viable.
- Necesitamos abrir un handle al directorio o archivo objetivo antes de pedir `statvfs`.
- La solucion mas limpia a corto plazo no es refactorizar `RemoteFs`, sino abrir una conexion auxiliar `ssh2` on-demand y cachearla logicamente por TTL.

## Principios de diseno

### 1. Protocol-native first

El orden de prioridad por protocolo debe ser:

- `SFTP native` antes que `SSH shell`.
- `SMB native host API` antes que heuristicas.
- `FTP/FTPS` sin soporte por defecto si no hay capacidad fiable.

### 2. Datos parciales son validos

No todos los protocolos devolveran `total`, `used` y `free`.

El contrato no debe asumir que siempre tendremos:

- `total_bytes`
- `used_bytes`
- `free_bytes`

### 3. La ruta importa

El dato debe obtenerse para una ruta remota concreta, no para "la conexion" en abstracto.

Dos rutas de la misma sesion pueden aterrizar en filesystems distintos.

### 4. Cache corta y explicita

La lectura no debe ocurrir en cada render de sidebar ni en cada cambio de seleccion.

TTL recomendado:

- 30 segundos al navegar
- 60 segundos si queremos reducir aun mas reconexiones

## Matriz de soporte recomendada

| Protocolo | Metodo principal | Metodo fallback | Estado recomendado |
|---|---|---|---|
| `ssh` con `sftp` | `ssh2 + SFTP + statvfs` | `df -Pk` si falla y el host parece Unix-like | Soporte oficial |
| `ssh` con `scp` | `df -Pk` | ninguno | Soporte parcial |
| `smb` en host Windows | `GetDiskFreeSpaceExW` sobre UNC | ninguno | Soporte parcial, opt-in por plataforma |
| `smb` en host Linux | ninguno con stack actual | ninguno | No soportado por ahora |
| `ftp` | ninguno por defecto | `AVBL` o `XQUOTA` solo si se decide modo experimental | No soportado por ahora |
| `ftps` | ninguno por defecto | `AVBL` o `XQUOTA` solo si se decide modo experimental | No soportado por ahora |

## Contrato de datos propuesto

Nuevo DTO backend/frontend:

```rust
pub struct DiskUsageDto {
    pub scope_path: String,
    pub total_bytes: Option<u64>,
    pub used_bytes: Option<u64>,
    pub free_bytes: Option<u64>,
    pub method: String,
    pub is_precise: bool,
    pub fetched_at_unix_ms: u64,
    pub note: Option<String>,
}
```

Valores esperados de `method`:

- `local_sysinfo`
- `sftp_statvfs`
- `ssh_df`
- `smb_unc_windows`
- `unsupported`
- `error`

Semantica:

- `is_precise = true` cuando el dato viene de una fuente nativa y consistente con la ruta.
- `is_precise = false` cuando venga de fallback shell o de una extension que solo de parte del dato.
- `note` se usa para explicar limitaciones o degradaciones.

## API backend propuesta

Nuevo comando Tauri:

```rust
get_remote_disk_usage(session_id: String, path: String, force_refresh: Option<bool>) -> Result<DiskUsageDto, String>
```

Reglas:

- `session_id` debe referirse a una sesion activa.
- `path` debe ser una ruta virtual remota de esa sesion.
- Si la ruta no pertenece al `session_id`, devolver error.
- Si el protocolo no soporta consulta de uso, devolver `DiskUsageDto` con campos `None` y `method = "unsupported"` en vez de error duro.

## Cache backend propuesta

Nuevo almacenamiento en `RemoteManager`:

```rust
disk_usage_cache: Mutex<HashMap<String, CachedDiskUsage>>
```

Clave sugerida:

- minima viable: `"{session_id}|{normalized_path}"`

Posible mejora posterior:

- si `statvfs` devuelve identificador de filesystem reutilizable, cachear por filesystem y no por ruta.

TTL inicial recomendado:

- 30 segundos

Invalidacion:

- al desconectar sesion
- al cambiar de protocolo o perfil
- al forzar refresh

## Estrategia concreta por protocolo

### A. `ssh` con `sftp`: via nativa prioritaria

#### Propuesta

Abrir una conexion auxiliar `ssh2::Session` usando las credenciales del perfil ya guardadas en la sesion activa.

Pasos:

1. Abrir TCP al host y puerto.
2. Handshake SSH.
3. Autenticacion con usuario y password.
4. Inicializar SFTP.
5. Resolver la ruta remota real que queremos medir.
6. Abrir handle con `opendir()` si la ruta es directorio.
7. Si la ruta es fichero, abrir su directorio padre o el propio fichero.
8. Llamar a `File::statvfs()`.
9. Convertir:
   - `total = f_blocks * f_frsize`
   - `free = f_bavail * f_frsize`
   - `used = total - free`

#### Ventajas

- Sigue lo que hacen clientes maduros.
- No depende de shell.
- Soporta mejor SFTP-only.
- Es mas correcto cuando la ruta remota cae en un mount especifico.

#### Riesgos

- Algunos servidores no anuncian o no implementan `statvfs@openssh.com`.
- Algunos devuelven informacion incorrecta para ciertos mounts.
- Requiere conexion adicional si mantenemos el `RemoteFs` actual intacto.

#### Decision

Debe ser la base de la implementacion para `SFTP`.

### B. `ssh` con `sftp`: fallback `df -Pk`

Usarlo solo cuando:

- falle `statvfs`
- o el servidor no soporte esa extension

Comando recomendado:

```sh
LC_ALL=C df -Pk -- "<ruta>"
```

Notas:

- `LC_ALL=C` reduce problemas de parseo localizados.
- Hay que escapar ruta con mucho cuidado.
- Solo es valido para hosts Unix-like con shell POSIX.

#### Decision

Fallback aceptable, no mecanismo principal.

### C. `ssh` con `scp`

`SCP` no da acceso nativo equivalente para cuota/filesystem en nuestra abstraccion actual.

Estrategia:

- usar `df -Pk`
- marcar resultado como no preciso
- si falla, devolver no disponible

#### Decision

Soporte parcial y claramente degradado.

### D. `smb` en host Windows

Si la ruta remota puede transformarse de forma segura a UNC:

```text
\\server\share\subpath
```

podemos usar `GetDiskFreeSpaceExW`.

Requisitos:

- solo en host Windows
- solo si el backend SMB y el path remoto permiten reconstruir una UNC fiable
- gestionar trailing backslash y permisos

Limitaciones:

- no resuelve el caso host Linux
- puede depender de autenticacion efectiva del proceso host

#### Decision

No es fase 1. Dejarlo como fase posterior especifica de plataforma.

### E. `ftp` y `ftps`

Opciones identificadas:

- `AVBL`
- `XQUOTA`
- comandos `SITE` especificos de servidor

Problemas:

- no son universales
- `AVBL` proviene de un draft expirado
- no garantizan `total`, `used` y `free`
- la compatibilidad real es muy heterogenea

#### Decision

No soportar por defecto.

Como mucho, fase futura experimental tras detectar `FEAT` y con UX explicita de capacidad parcial.

## Diseno frontend propuesto

### Estado

Nuevo mapa en store:

```ts
remoteDiskUsageBySession: Record<string, DiskUsageDto | null>
remoteDiskUsageLoadingBySession: Record<string, boolean>
```

### Disparadores

Solicitar lectura cuando:

1. una conexion remota se abre
2. el usuario navega a la raiz de esa conexion
3. el usuario cambia a una ruta remota de otra sesion
4. el cache haya expirado

### Render

En la seccion de conexiones remotas de la sidebar:

- si hay `free` y `total`, mostrar barra como en local
- si solo hay `free`, mostrar solo texto
- si no hay datos, mostrar nada o un texto corto tipo `Espacio no disponible`

### UX importante

No bloquear la navegacion esperando el dato.

La UI debe tolerar:

- carga
- datos parciales
- no soportado
- error transitorio

## Logging y diagnostico

Registrar en backend:

- protocolo
- metodo usado
- duracion
- hit/miss de cache
- causa de fallback
- causa de `unsupported`

No registrar:

- password
- comandos completos con credenciales

## Plan de implementacion por fases

### Fase 1: base y UX segura

1. Crear `DiskUsageDto`.
2. Crear cache en `RemoteManager`.
3. Crear comando `get_remote_disk_usage`.
4. Renderizar datos remotos en sidebar con estado de carga y no disponible.
5. Devolver `unsupported` para todo excepto `ssh`.

Resultado:

- API, cache y frontend quedan preparados.

### Fase 2: `SFTP` nativo

1. Implementar conexion auxiliar `ssh2`.
2. Resolver ruta objetivo.
3. Usar `sftp.opendir(...).statvfs()`.
4. Traducir a bytes.
5. Marcar `method = "sftp_statvfs"`.

Resultado:

- soporte bueno para la mayoria de servidores SFTP reales.

### Fase 3: fallback `df` para `ssh` / `scp`

1. Reusar perfil de sesion.
2. Ejecutar `df -Pk`.
3. Parsear salida robustamente.
4. Marcar `is_precise = false`.

Resultado:

- mejora cobertura para Unix-like sin extension `statvfs`.

### Fase 4: `SMB` Windows host

1. Verificar conversion segura a UNC.
2. Llamar `GetDiskFreeSpaceExW`.
3. Activarlo solo en Windows.

Resultado:

- soporte parcial util para el caso Windows + shares SMB.

### Fase 5: experimental `FTP/FTPS`

Solo si realmente se quiere:

1. detectar `FEAT`
2. probar `AVBL` o `XQUOTA`
3. exponerlo como dato parcial y no garantizado

Resultado:

- cobertura oportunista, no contractual

## Riesgos principales

### Riesgo 1: filesystem equivocado para la ruta

Mitigacion:

- siempre consultar sobre la ruta real navegada
- no cachear ciegamente por sesion

### Riesgo 2: sobrecoste por conexiones auxiliares

Mitigacion:

- TTL corto
- cargar bajo demanda
- no refrescar en cada render

### Riesgo 3: servidores SSH Windows

Mitigacion:

- usar `SFTP native` primero
- no depender de `df` salvo fallback Unix-like

### Riesgo 4: falsas expectativas en FTP/FTPS

Mitigacion:

- devolver `unsupported` por defecto
- no inventar heuristicas

## Validacion y pruebas

Minimo necesario:

1. SFTP Linux con `statvfs` funcionando.
2. SFTP NAS con mount distinto al root del sistema.
3. SFTP-only sin shell interactiva.
4. SCP sobre host Unix-like con `df`.
5. SSH Windows con OpenSSH para verificar degradacion correcta.
6. SMB en Windows host si se implementa fase 4.
7. FTP y FTPS mostrando "no disponible" sin errores visuales.

## Decision final

La implementacion correcta para Dogu es:

- `protocol-native first`
- `path-scoped`
- `partial-data friendly`
- `unsupported by design` cuando no exista una base fiable

La primera implementacion no debe intentar resolver todos los protocolos al mismo nivel. Debe resolver muy bien `SFTP`, razonablemente `SSH/SCP` Unix-like y dejar el resto explicitamente fuera hasta tener una via tecnica robusta.
