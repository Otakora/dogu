# Changelog

## 0.2.6-beta.3

### English

- Dreamcast GDI conversion now recognizes referenced ISO tracks and avoids converting them as separate discs.
- Improved identification of BIN and RAW tracks referenced by GDI or CUE files, including remote sources.

### Español

- La conversión de GDI de Dreamcast reconoce las pistas ISO referenciadas y evita convertirlas como discos independientes.
- Mejor identificación de pistas BIN y RAW referenciadas por archivos GDI o CUE, también en ubicaciones remotas.

## 0.2.6-beta.2

### English

- Added customizable extraction folders and disc suffix removal, with queue previews.
- Added Windows path length checks and clearer errors for affected operations.
- Improved file selection, window controls, queue display and M3U generation.

### Español

- Nuevas carpetas de extracción personalizables y eliminación de sufijos de disco, con fantasmas en la cola.
- Comprobaciones de longitud de rutas en Windows y errores más claros en las operaciones afectadas.
- Mejoras en la selección de archivos, controles de ventana, visualización de la cola y generación M3U.

## 0.2.6-beta.1

### English

- Improved queue planning, parallel execution, recovery, progress and ghost previews.
- Safer remote transfers and editing, more reliable SMB paths and connection management.
- Improved Linux utilities, file browsing, search, settings and application layout.
- Update checks now ask before downloading and show download progress.

### Español

- Mejoras en la planificación, ejecución paralela, recuperación, progreso y fantasmas de la cola.
- Transferencias y edición remotas más seguras, rutas SMB y gestión de conexiones más fiables.
- Mejoras en utilidades Linux, exploración, búsqueda, configuración y disposición de la aplicación.
- Las actualizaciones se consultan antes de descargarse y muestran su progreso.

## 0.2.5

### English

- Quieter Windows builds: external tools no longer pop empty console windows.
- Smarter queue: clearer chained operations, relation map, real removal marks, and better progress for remote transfers.
- CHD workflows are safer: Dreamcast GDI restore is detected correctly and accented folders are handled around chdman limitations.
- Remote connections recover SSH fingerprints more naturally when old profiles need to trust a host again.
- Dogu now prepares stable/beta update channels and release packaging for winget.

### Español

- Builds de Windows más tranquilas: las herramientas externas ya no abren consolas vacías.
- Cola más clara: operaciones encadenadas, mapa de relaciones, marcas de elementos que desaparecerán y mejor progreso en transferencias remotas.
- Flujos CHD más seguros: la restauración Dreamcast GDI se detecta correctamente y las carpetas con acentos se gestionan alrededor de las limitaciones de chdman.
- Las conexiones remotas recuperan huellas SSH de forma más natural cuando un perfil antiguo necesita volver a confiar en el host.
- Dogu prepara canales de actualización stable/beta y empaquetado de release para winget.
