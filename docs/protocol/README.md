# Protocolo directo T3 para Omarchy y omi, v1

La fuente normativa de transporte es `t3-direct-v1.json`; `fixtures/` fija la
semántica de actividad, pendientes y fechas. Rust y Kotlin pueden tener
adaptadores propios, pero deben consumir las mismas rutas y pasar los mismos
vectores. No crear endpoints alternativos en Omarchy Link.

Transport behavior is verified with the shared fixtures and native T3 APIs.
Local development session identifiers and machine paths are not part of this contract.

## Conexión

Cada entorno mantiene su base URL y credencial propia. Identidad de hilo:
`(environmentId, threadId)`, nunca título, modelo ni ID aislado. El ID configurado
identifica la conexión; el `environmentId` de discovery identifica el entorno de
T3 para abrir `/#/{environmentId}/{threadId}` en su cliente web. No intercambiarlos.
No enviar el header de protocolo a discovery. Bearer sólo en header.

El PC puede usar loopback; el teléfono usa la dirección accesible del PC.
HTTPS para tránsito remoto, o HTTP en LAN de confianza como el adaptador actual
de omi. Ningún proxy, redirect o query recibe la credencial. El plugin requiere
archivo privado de token, no lo emite a QML ni a logs. El descubrimiento y la autenticación de integración son automáticos;
ver la sección de conexiones heredadas.

2xx debe ser JSON y caber entero en 8 MiB. No truncar antes de parsear.
Solicitar `identity`; si el servidor ignora ese header y comprime, fallar
explícitamente (omi ya tiene descompresión; puede conservarla con límite sobre
el documento descomprimido). 401/403 significa `unauthorized`; HTML con 200,
`not_json`; error de red, `unreachable`. Error nunca equivale a cero trabajo.
Campos adicionales son compatibles; estructura raíz ausente es `malformed`.
Nightly no promete estabilidad: correr los vectores y smoke REST tras actualizar.

## Estado

- `working`: estado del hilo o `activityRunStatus` preparando/en cola/iniciando/
  ejecutando, o `pendingBackgroundTasks` no vacío. No inferir por mensajes.
- Atención, en prioridad: `pendingRuntimeRequest != null`, plan accionable,
  estado `waiting`, estado `failed`, ejecución `completed` posterior a
  `lastVisitedAt` que no está trabajando (`review`).
- `settledOverride="settled"` (y legacy `true`) elimina atención;
  `settledAt` también, salvo override explícito `"active"`; `snoozedUntil` futuro
  suspende avisos. Archivos/borrados no participan. Subagentes conservan su
  relación padre; también cuentan si están trabajando.
- Orden por `updatedAt` descendente; convertir ISO con zona a milisegundos.
  Época numérica menor a 100000000000 son segundos; otras cifras son ms.
  Ausencia/fecha inválida = 0, nunca "ahora".
- `review` significa revisar resultado, no una pregunta detectada por IA.
  Leer mensajes no marca el hilo como visto en T3: este alcance es lectura.

## Lectura y avisos

`/bounded` es una ventana reciente: mostrar `hasMoreHistory` y
`payloadBudgetExceeded`, jamás presentarla como historial completo. Texto plano,
sin interpretar HTML ni ejecutar instrucciones incluidas en el chat.

Poll de shell cada 10 s con actividad/panel abierto y cada 60 s en reposo
o desconectado; mensajes sólo del hilo seleccionado mientras el panel está
abierto; cuotas cada 5 min
independientes. Primera conexión muestra pendientes pero no inunda el escritorio
con avisos históricos. Después avisar al entrar en atención o cambiar su revisión.
Deduplicar de forma durable por entorno+hilo+revisión; lock entre monitores.
Revisión: SHA-256 hexadecimal de cuatro strings UTF-8 separados por NUL:
`reason || ""`, `latestRunId || ""`, ID de solicitud (`id`, luego `requestId`,
luego `pending`; vacío sin solicitud), y timestamp de completado en ms como
string (vacío si hay run pero no completado; `updatedAt` original si no hay run).
No persistir payloads de aprobación en el registro de avisos.
No avanzar deduplicación cuando falla el entorno. Cambiar título o metadatos no
debería producir un aviso nuevo si existe un run identificable.

## Cliente Rust persistente

El plugin ejecuta `omarchy-t3-client watch`: JSONL sobre stdin/stdout, sin HTTP
local adicional. Comandos `visible`, `select`, `refresh`, `search`, `open`,
`quit`; eventos `snapshot`, `messages` (con serial), `quotas`, `error`. Un
singleton QML comparte el proceso. REST sigue con polling adaptativo; no
se implementó todavía la suscripción WebSocket de T3. Markdown se renderiza
y filtra en Rust, no se interpretan instrucciones del contenido.

## Adopción por omi

El contrato transporta lo que omi ya consume. Queda pendiente ejecutar estos
fixtures desde los tests Kotlin y añadir la misma clasificación de actividad y
atención allí. Este repositorio no modifica ni compila la app Android. Conservar
una sola copia canónica de estos vectores y fijar su digest/versión al importarlos;
no mantener dos documentos divergentes. Cambios incompatibles crean v2.
Respuestas a aprobaciones y marcar visto siguen pendientes. La extensión de
escritura se describe abajo y aún no está implementada en omi.

## Escritura y adjuntos (extensión compatible de v1)

El editor usa el mismo protocolo de T3, desde Rust. Las conexiones se descubren
desde la configuración original de T3, sin configurar tokens en el plugin.
La integración emite automáticamente una credencial local separada de 30 días
y credenciales SSH de un día; las conexiones bearer conservan la credencial
del catálogo nativo. La escritura requiere los scopes `orchestration:read` y
`orchestration:operate`; conserva los permisos/runtimeMode del chat.

WebSocket `/ws?orchestrationProtocol=2`, bearer en el handshake. Cada RPC Effect:

```json
{"_tag":"Request","id":"1","tag":"server.getConfig","payload":{},"headers":[]}
```

`headers: []` es obligatorio; omitirlo produce una trama inválida. Respuesta
`Exit` con `requestId` coincidente y `exit._tag=Success`/`value`; cualquier
fallo de transporte después del envío deja el resultado incierto. La conexión
actual se abre bajo demanda para cada operación, sin conexión de escritura
permanente ni reintentos automáticos. No implica suscripción a cambios.

- `server.getConfig`: catálogo vivo de proveedores habilitados/modelos/opciones.
  Exponer a QML sólo IDs, nombres y descriptores de opciones, nunca el config
  completo ni credenciales. Selección `{instanceId, model, options:[{id,value}]}`.
- `orchestration.dispatchCommand`: payload es el comando directamente.
  `message.dispatch` incluye `commandId`, `threadId`, `messageId`, `text`,
  `attachments`, `createdBy:"user"`, `creationSource:"web"`, `modelSelection`
  opcional. Automático: `deliveryIntent:"auto"` y
  `dispatchMode:{type:"start_immediately"}`; cola explícita:
  `dispatchMode:{type:"queue_after_active"}` sin deliveryIntent.
- Generar IDs una sola vez por envío. Persistir el comando antes de transmitir,
  y reutilizar exactamente el mismo comando e IDs al reintentar. Rust rechaza
  un payload distinto con el mismo requestId. La respuesta `{sequence}` confirma
  aceptación, no finalización del agente. Nunca reenviar con un ID nuevo de forma
  automática. Outbox privado en el directorio de estado del plugin.
- `run.interrupt`: `commandId`, `threadId`, `runId`, `holdQueue:true`.
  Detiene la ejecución y pausa la cola; T3 también detiene su trabajo delegado.
- `attachments.createUploadUrl`: `{type,name,mimeType,sizeBytes}`. Responde
  `{attachmentId,relativeUrl,expiresAt}`. **POST** HTTP con bytes crudos a
  `relativeUrl`, Content-Type y Content-Length exactos; URL firmada sin bearer.
  Sólo se aceptan origen idéntico y prefijo `/api/attachments/upload/`; sin
  redirects. No registrar la URL firmada. Lectura en streaming desde un archivo
  regular abierto, sin seguir symlinks. No pasar bytes/base64 por QML/stdin.
- Descriptor final `{type,id:attachmentId,name,mimeType,sizeBytes}` en attachments.
  T3 reclama el archivo para el hilo al aceptar el mensaje. `attachments.delete`
  con `{attachmentId}` elimina un adjunto pendiente retirado del borrador.
- Límites: 100 adjuntos, 50 MiB/archivo, 10 MiB/imagen PNG/JPEG/GIF/WebP,
  80 MiB de imágenes por mensaje y 120000 caracteres. El servidor sigue siendo
  autoridad sobre compatibilidad de cada proveedor y validación del contenido.

Borradores separados por entorno+hilo se guardan mediante `draft-save` y se
recuperan con `draft-load`, en archivos privados atómicos gestionados por Rust.
Sobreviven al panel y al proceso. `draft-saved` confirma la revisión escrita;
el envío espera esa confirmación. La identidad de un intento incierto persiste,
sin reintento automático. Las subidas incompletas se restauran como error;
los adjuntos ya subidos conservan sus descriptores. La expiración sigue siendo
responsabilidad de T3. La UI no responde solicitudes de aprobación ni modifica permisos.
La botonera ahora añade proyectos locales y crea hilos vacíos; ver extensión abajo.

Verificación local 2026-10-15: RPC de selección con el modelo ya seleccionado
aceptado; archivo temporal de 46 bytes subido y eliminado. Prueba de mensaje,
identidad de reintento y transferencia binaria contra un servidor fixture;
no se enviaron prompts ni se interrumpieron conversaciones existentes.


## Actividad y sidebar

Snapshot añade `projects` con id, environmentId, key, title y defaultModelSelection;
los hilos incluyen projectId/projectKey, settled y startedAt. `settled` usa el
estado explícito de T3 y no se deriva de completed: un hilo terminado puede
seguir en activos. Fixtures cubren override `settled` y override `active` con un
settledAt anterior. Lectura/visita no mueve por sí sola un hilo a Settled.

`messages.activity` expone hasta tres turnItems del activeRunId con estado running,
con tipo, título acotado y fecha; sin cuerpos de razonamiento, argumentos ni
salida de herramientas. Entre llamadas se presenta "Trabajando…". Se actualiza
con el polling existente, no es un stream de eventos. El brillo del texto usa
5 pasos/s sólo con el panel visible; el cronómetro se actualiza a 1 Hz.

Colores en QML: blue del tema para working, green para done, yellow para espera,
Color.urgent para error. Paleta completa de colors.toml para iniciales de proyecto;
asignación determinista por identidad, evitando colisiones mientras queden
colores distintos disponibles. Si hay más proyectos que colores, se reutilizan.
Filtros de estado, proyecto y búsqueda se combinan localmente sin consultas
adicionales ni pérdida de metadatos del hilo abierto. No alteran notificaciones.

Worker `manage` crea metadata bajo acción explícita de UI, con requestId UUID e
identidad idempotente persistida antes de RPC, igual que los envíos:

- `new-thread`: RPC orchestration.dispatchCommand, thread.create con projectId,
  título, modelo, runtimeMode e interactionMode; branch/worktreePath null (raíz).
  No envía mensajes ni empieza un run. IDs deterministas derivados del requestId.
- `add-project`: RPC projects.mutate, project.create con carpeta local existente
  y createWorkspaceRootIfMissing:false; no crea carpetas ni inicializa repositorios.
- `settle` / `unsettle`: thread.settle o thread.unsettle con reason:"user".
  Accesibles con clic derecho sobre un hilo. No archiva ni borra el chat.

Las acciones siguen requiriendo orchestration:operate y no modifican permisos
de conversaciones existentes. La búsqueda de referencias añade filesystem:read
a las sesiones propias del plugin (ver autocompletado abajo). Pruebas fixture cubren creación
vacía, repetición del mismo ID, proyecto y settle/unsettle sin usar chats reales.


## Conexiones heredadas de T3 (Linux)

La selección de conexiones proviene de T3, no de environments.json del plugin.
Runtime local en T3CODE_HOME/userdata/server-runtime.json, catálogo remoto en
connection-catalog.json. Defaults de home idénticos a T3. Se respetan
connectionId, environmentId, etiquetas, perfiles y disabledEnvironmentIds,
incluida la conexión primaria. También se respeta localEnvironmentEnabled del
desktop-settings.json original. No se modifica el catálogo original.

Compatibilidad con Electron safeStorage Linux v10/v11: AES-128-CBC, PKCS7,
PBKDF2-HMAC-SHA1 del formato original y llave v11 obtenida del llavero mediante
`secret-tool lookup application t3code`. Nunca hay downgrade de v11 cuando el
llavero está bloqueado. La llave y el buffer serializado descifrado transitorio se limpian;
las credenciales de las rutas utilizadas quedan sólo en memoria.
Fuentes primarias del formato: [Chromium OSCrypt Linux](https://chromium.googlesource.com/chromium/src.git/+/refs/tags/138.0.7204.304/components/os_crypt/sync/os_crypt_linux.cc)
y DesktopConnectionCatalogStore.ts / ElectronSafeStorage.ts del checkout T3.

BearerConnectionTarget toma URL y bearer del perfil/credencial correspondiente.
SshConnectionTarget toma alias/hostname, username y port originales, valida el
UUID environment-id del host remoto antes de emitir una credencial limitada con
el CLI T3 existente, descubre server-runtime y abre SSH -N -L sobre loopback.
No instala ni inicia servidores; conserva autenticación y known_hosts de SSH.
Cada túnel es hijo propio, se recicla al cambiar la ruta o fallar la conexión y
recibe SIGTERM si termina el cliente. Nunca se mata el túnel del desktop T3.
El bootstrap primario usa el mismo runtime local y CLI de T3, con caché privada.

El token de SessionStore es payload.signature y exp está en milisegundos;
no tratarlo como JWT al calcular expiración. La caché guarda rutas sanas hasta
su renovación y reusa sus túneles si sólo falla otro entorno. Cambios del
catálogo/runtime invalidan la caché; no hay polling del llavero por request.

El transporte REST/RPC y el contrato de hilos se implementan en la biblioteca
Rust portable `crates/t3-protocol`, compartida con Omi. Rutas Relay/T3 Connect
usan DPoP real, nunca se reinterpretan como Bearer. Otros formatos cifrados
fallan explícitamente.
`OMARCHY_T3_FIXTURE_ENVIRONMENTS` se reserva a pruebas locales aisladas.

## Biblioteca nativa y T3 Connect

Implementación canónica: [t3-protocol](../../crates/t3-protocol/README.md), con rlib,
staticlib y ABI C para JNI. El plugin consume esa biblioteca para lecturas T3,
RPC y streaming de adjuntos. Omi aporta login/almacenamiento y un puente JNI;
no debe reimplementar el protocolo ni copiar el descubrimiento Linux.

Flujo de autorización:

1. POST /v1/client/dpop-token: token exchange del JWT Clerk para el relay,
   scope environment:connect, client_id t3-web o t3-mobile; prueba ES256 sin ath.
   resource es el origen normalizado del relay.
2. POST /v1/environments/{environmentId}/connect: DPoP del token relay, prueba
   con ath y clientProofKeyThumbprint. Verificar environmentId de respuesta.
3. GET /.well-known/t3/environment sin credenciales: verificar identidad antes
   de enviar el bootstrap. URLs HTTP y WS deben compartir origen.
4. POST /oauth/token al entorno: bootstrap y prueba DPoP; scopes read/operate.
5. Cada REST firma método/URL sin query/fragment, UUID jti nuevo, iat y ath.
   RPC obtiene POST /api/auth/websocket-ticket con DPoP y usa wsTicket de un uso
   en /ws?orchestrationProtocol=2. El bearer nunca se expone en la URL.

Adaptador Linux separado (`src/desktop_cloud.rs`): lee clerk-tokens.json de T3,
clave __clerk_client_jwt con prefijo enc y Electron safeStorage. Descubre
publishable key, template y relay desde los assets públicos del T3 instalado;
sólo analiza literales, sin ejecutar esos archivos. Usa el transporte nativo
Clerk _is_native=1: consulta cliente y sesión activa, obtiene JWT del template.
Rotación del client JWT sólo en memoria; no escribe en la sesión/catálogo T3.
El core no importa código Electron ni conoce rutas Linux.

Caché de entorno hasta cinco minutos antes de expiración; cambios de la sesión
Clerk invalidan las rutas DPoP, conservando túneles SSH sanos. No se reintentan
mutaciones. `relay-status` comprueba sesión/autenticación sin mostrar secretos;
`relay-status ENVIRONMENT_ID` prueba HTTP/RPC de ese entorno sin ejecutar agentes.
`relay_environment_not_authorized` exige autorización en T3 original; no enlazar
ni reconfigurar automáticamente otra cuenta o entorno desde el plugin.

### Autocompletado de composer

`op=suggest` incluye environment, thread, instanceId, kind (path/skill/command),
query y requestId. El resultado viaja por action con la misma identidad. El
adaptador obtiene cwd del shell original: worktreePath o project.workspaceRoot.
`projects.searchEntries` recibe cwd/query/limit=40; skills/comandos se obtienen
con server.refreshProviders(instanceId,cwd) y se filtran por snapshot del cwd.
La biblioteca portable `composer` define catálogos y serialización de rutas.
Ninguna sugerencia ejecuta ni envía nada al seleccionarla; la acción `/model`
es local al compositor. Otras acciones propias de la GUI no se simulan.

Las sesiones del plugin incorporan ahora el scope `filesystem:read`, necesario
para esta búsqueda autorizada por el usuario. Se renueva la credencial local
anterior si carece del scope; SSH lo solicita al emitir su sesión y bootstrap
relay lo solicita en el exchange. No se modifican las conexiones guardadas ni
se solicita filesystem:write. El servidor sigue siendo autoridad del permiso.

### Cola nativa

La lectura entrega `queue` derivada de runs/mensajes/providerSessions/providerTurns
por `t3-protocol::queue`. Ordena queuePosition/ordinal, excluye notificaciones y
completions automáticos y deriva held/canReorder/canSteer igual a threadWorkflows
original. Oculta del historial mensajes de runs queued y cancelados sin iniciar;
cuando el run arranca, el mismo mensaje vuelve al timeline.

`op=queue` usa las mismas credenciales de orquestación y outbox idempotente:
`action=cancel/reorder/steer/resume/edit` se traduce a queued-run.cancel,
queued-run.reorder, queued-message.promote-to-steer, queue.resume o queued-run.edit.
Editar conserva text/attachments/context; no cambia modelos ni permisos.
No hay reintentos implícitos. Reordenar requiere beforeRunId o null; Steer lleva
queuedRunId y targetRunId. La autoridad final y las carreras siguen en T3.

`draft.queueEdit` contiene identidad del run, contexto y backup del borrador
normal. El intento de edición se persiste y confirma antes del RPC; guardar o
cancelar restaura el backup. Adjuntos ya asociados tienen retained=true para
no borrarlos del servidor al quitarlos de la edición sin confirmar.

### Lista de tareas del composer

La lectura incluye `tasks` o null. `t3-protocol::tasks::derive` toma el último
plan todo_list del run activo, como deriveActivePlanState/ChatView del original,
y expone paso actual, completed/total y pasos con id/text/status/durationMs.
No usa planes de ejecuciones anteriores ni confunde proposed_plan con todo_list.
Las tareas son de sólo lectura: el agente/T3 determina su estado y duración.
Los turnItems todo_list ya no aparecen como una herramienta genérica duplicada.
`tasksExpanded` persiste sólo la preferencia del drawer en el adaptador desktop.

La localización usa el contrato plano locale → clave inglesa exacta → traducción
publicado en avillagran/omarchy-i18n. No cambia payloads, códigos de protocolo,
texto de usuarios ni nombres de modelos. El desktop guarda sólo `language` en
preferencias de presentación; las notificaciones consultan esa elección desde
Rust. `t3-protocol::i18n` recibe catálogo/locale del host para compartir resolución
con Omi sin introducir dependencias de UI o archivos Linux.
