# Auditoría de puntos de contacto con T3 (ot3 → `t3-protocol`)

- Repo: `omarchy-t3-code` @ `191c1fc` (v0.2.22), rama `main`, árbol limpio.
- Biblioteca: `vendor/t3-protocol` (v0.2.0, 3 824 líneas en `src/`).
- Método: lectura de `vendor/t3-protocol/src/lib.rs` y `README.md`, y greps sobre `src/` y `tests/` de `reqwest`, `tungstenite`, `"/api/`, `Client::new`, `.get(e,`, `fetch(`, `fetch_headers(`, `rpc(`, `request_json(`, `subscribe`, `Connection::new`, `curl`, `TcpStream`, `event-stream`. Cada sitio se abrió y se leyó.
- No se modificó nada del repo ni se ejecutó `cargo test` (compilar escribiría en `target/`). Las tablas generadas por grep están en los apéndices A–D. Las anotaciones manuales están marcadas con «(dinámico)».

---

## 0. Hallazgos que corrigen la premisa del encargo

1. **`Client::get` ya delega en la lib.** `src/lib.rs:472-478` construye un `protocol::Connection::with_http_client(...)` y llama a `request_json("GET", …)`. No hace el GET con `reqwest` propio. Solo `Client::fetch` y `Client::fetch_headers` (`src/lib.rs:410-471`) hacen HTTP propio. Hoy solo los usa `src/local_quotas.rs:181,192,193,334`, contra APIs de terceros (OpenAI, Kimi, Nous, opencode). No es tráfico T3 (ver §2).
2. **De los 27 archivos que importan `reqwest::`, 18 solo usan `reqwest::Url`** para parsear URLs. Los 9 restantes construyen un cliente HTTP:
   - Seis hablan con un servidor T3 sin pasar por la lib: `lib.rs`, `notification_icon.rs`, `checkpoint_operations.rs`, `connections.rs`, `connections_management.rs`, `omabridge_route.rs`.
   - Tres no son T3: `images.rs` (URL pública de Markdown), `desktop_cloud.rs` (Clerk), `server_updates.rs` (GitHub).
3. **No hay WebSocket propio en `src/`.** `tungstenite` solo aparece en `src/device_screen.rs` (5 referencias a `tungstenite::Message` y `would_block`) y en `tests/`. El socket lo abre la lib (`Connection::device_socket`, `rpc`, `subscribe`). El problema es de tipos: `device_screen` maneja `tungstenite::Message`, y `device_hub.rs` de la lib expone `reqwest::blocking::Response` (`device_mjpeg`) y `tungstenite::Message` (`input_message`), que no sirven para JNI.
4. **No hay SSE.** 0 coincidencias de `text/event-stream` o `EventSource`. Todas las suscripciones son streams Effect-RPC sobre WebSocket (`Connection::subscribe` / `subscribe_probed`, 13 sitios en `src/`).
5. **El trabajo real no son esos 6 sitios.** Son estos:
   - 27 constructores de `Connection::new` / `with_http_client`: 11 funciones `connection(e)` casi idénticas más 16 copias en línea. Cada una decide a mano el token de lectura o de escritura.
   - 153 sitios RPC con 98 tags de petición escritos en ot3, más 12 tags de stream. La lib no tiene constructores tipados para casi ninguno.
   - Lógica de dominio en `src/` que un cliente Kotlin tendría que reescribir (§4).
6. **`vendor/t3-protocol` no es idéntico a `lib-t3-rust` en `~/Work/lib-t3-rust`.** Difieren 10 archivos de `src/`: `activity`, `compaction`, `documents`, `ffi`, `ffi_stream`, `lib`, `projection`, `queue`, `sidebar` y `stream`. `device_hub.rs` y `long_rpc.rs` solo existen en el vendor. El `Cargo.toml` de ot3 fija `rev = 1a616ab6…` pero lo sobreescribe con `[patch]` a `path = "vendor/t3-protocol"`. El repo `lib-t3-rust` tiene además cambios sin commit en `src/ffi.rs`, `src/ffi_stream.rs` y `src/lib.rs`. El vendor tampoco trae `tests/`: los tests de la lib (`protocol.rs`, `stream.rs`, `long_rpc.rs`, `rpc_errors.rs`, `models.rs`, `lifecycle.rs`, `ffi_events.rs`, `sidebar_pr.rs`, `bounded_snapshot.rs`) viven solo en `lib-t3-rust/tests`. **Antes de cualquier lote hay que sincronizar vendor y repo de la lib y retaguear.** Si no, el Android reutilizaría una lib distinta de la que prueba ot3.

---

## 1. Puntos de contacto con T3 que NO pasan por `t3_protocol`

### 1.A Sitios con cliente HTTP propio hacia un servidor T3

Es la tabla pedida (la lista cerrada). 6 funciones, 8 peticiones HTTP, 6 archivos.

| # | archivo:línea | tipo | ruta / tag exacto | helper usado | qué lo reemplazaría |
|---|---|---|---|---|---|
| A1 | `src/lib.rs:71-85` (`attachment_bytes`) | asset firmado (GET) | RPC `assets.createUrl` (vía `write::rpc`, `lib.rs:74`) y luego `GET <relativeUrl>` bajo `/api/assets/` | `reqwest::blocking::Client::builder()` en `lib.rs:79`: `no_proxy`, redirect none, timeout 120 s, `take(limit+1)`. Consumidor: `videos.rs:24` (límite 50 MiB) | **NUEVA:** `Connection::asset_bytes(&self, resource: Value, opts: AssetOptions{ max_bytes: u64, timeout: Duration, expect_mime: Option<&str> }) -> Result<AssetBytes{ bytes: Vec<u8>, mime: String }>`. Es la generalización de `image_resource` (vendor `lib.rs:724-734`), que hoy es privada y fuerza 10 MiB, 30 s y firma de imagen. `attachment_bytes` se convertiría en `asset_bytes(attachment_resource(id,name,mime), …)`. |
| A2 | `src/notification_icon.rs:101-141` (`favicon`) | asset firmado (GET) | `assets.createUrl` con `{"_tag":"project-favicon","cwd","path"}` (`:121`) y luego `GET` bajo `/api/assets/` (`:134-141`) | `reqwest::blocking::Client::builder()`, redirect none, timeout 3 s, tope 2 MiB. Rechaza rutas que terminan en `/project-favicon-missing` (`:130`) | **NUEVA:** `Connection::project_favicon(&self, cwd: &str, path: &str) -> Result<Option<(Vec<u8>, String)>>`, apoyada en `asset_bytes`. `Option` para distinguir «no hay favicon» (`…/project-favicon-missing`) de un error. |
| A3 | `src/checkpoint_operations.rs:154-215` (`prepare_attachments`) | asset firmado (GET) + upload | RPC `assets.createUrl` (`:182`, **sin** `disposition`) → `GET` bajo `/api/assets/` (`:196`) → `Connection::upload_attachment` (`:210`) | `Http::builder()` (`:174`): `no_proxy`, redirect none, timeout 30 s. Lee exactamente `sizeBytes` y compara | `Connection::asset_bytes` con `max_bytes = sizeBytes`, más `upload_attachment` (ya en la lib). La comparación de tamaño exacto (`revert_attachment_changed`) queda en el host o se añade `AssetOptions::exact_size`. |
| A4 | `src/connections.rs:167-173` (`prune_sessions`; se lanza en `:231`) | GET + POST (gestión de acceso) | `GET /api/auth/clients` y `POST /api/auth/clients/revoke` (body `{"sessionId"}`) | `reqwest::blocking::Client::builder().timeout(10s)` **sin** `no_proxy` y con redirects por defecto (hasta 10). `bearer_auth(token)` | Ya existe: `Connection::request_json("GET"/"POST", …)`; `connection_access.rs:65,104` ya llama a las mismas rutas por la lib. **NUEVA (opcional):** `Connection::revoke_stale_sessions(subject: &str, now) -> Result<usize>`, para que Android reutilice la política junto con `stale_ot3_sessions` (`connections.rs:157-166`, pura, ver §4). |
| A5 | `src/connections_management.rs:169-236` (rama `bearer`) | GET + POST (descriptor y emparejamiento) | `GET <base>/.well-known/t3/environment` (`:174-186`) y `POST <base>/oauth/token` (`:211-236`), form `grant_type=…token-exchange`, `subject_token_type=urn:t3:params:oauth:token-type:environment-bootstrap`, `client_label=ot3`, `client_device_type=desktop`, `client_os=linux` | `Client::new()?.http` (`:173`, acceso al campo privado `http`), 8 s, sin `Accept`, sin límite de bytes, `.json()` | **NUEVA:** `fetch_descriptor(http_base: &str) -> Result<Value>` (sin credencial). El código ya existe privado dentro de `RelayClient::connect_environment` (vendor `lib.rs:653-658`). **NUEVA:** `pair_bearer(http_base, pairing_code, client: PairingClient{label, device_type, os}) -> Result<PairedBearer{ access_token, descriptor }>`. Hoy la lib solo sabe el canje DPoP del relay (`lib.rs:662-694`), no el canje Bearer de `pairing_code`. |
| A6 | `src/omabridge_route.rs:93-98` (`verify_environment`; lo llama `connections.rs:332`) | GET (descriptor) | `GET <url-del-forward>/.well-known/t3/environment` | `reqwest::blocking::Client::builder().timeout(5s)`, **sin** `no_proxy`, redirects por defecto, `.json()`. Compara `environmentId` | `fetch_descriptor` (A5) más la comparación `descriptor["environmentId"] == id`. **NUEVA (opcional):** `verify_environment(http_base, expected_id)`. El resto de `omabridge_*` no es T3 (§2). |

**Qué no entra aquí (aunque tenga `reqwest`):**
- `Client::fetch` y `fetch_headers` (`lib.rs:410-471`) → tráfico a terceros, ver §2.
- `images.rs:36-41`, `desktop_cloud.rs:222-275` y `server_updates.rs:97-110` → no son T3, ver §2.
- Los 18 archivos con solo `reqwest::Url` → §1.E.

### 1.B Peticiones HTTP a T3 que ya pasan por la lib (para completitud)

Las 42 filas del Apéndice B salen de este grep. Se resumen por helper:
- **`Client::get`** (`lib.rs:472-478`) → `Connection::request_json("GET", path, None, protocol)`. 21 sitios. Siempre usa `Credential::bearer(environment_token(e,false))`, el **token de lectura**. Para entornos relay (`e.connection`) usa esa conexión.
- **`Client::bounded_thread`** (`lib.rs:479-487`) → `get` con `?compactTurnItems=1`, más `bounded_snapshot::restore`. 6 sitios (`write.rs:135,384`, `auto_compact.rs:45`, `live.rs:24`, `failure_context.rs:117`, `lib.rs:489` vía `messages`).
- **`Connection::request_json` directo**: 15 líneas, incluidos `connection_access::request` (`:44`) y `write.rs:301` (`POST /api/pull-requests/diff`). La ruta está escrita a mano en ot3 y no la posee `protocol::pull_requests`.
- **`protocol::history::path`** construye `/api/orchestration/threads/{id}/history?cursor=`, y `write.rs:51-57` lo usa con `Client::get`.

**Rutas HTTP únicas en `src/` (17):**

| # | método | ruta | dónde |
|---|---|---|---|
| 1 | GET | `/api/auth/session` | 14 sitios (7 `Client::get`, 7 `request_json`) |
| 2 | GET | `/api/orchestration/shell` | 11 sitios (7 `Client::get`, 4 `request_json`) |
| 3 | GET | `/api/projects` | `connection_route.rs:25`, `journal_vault.rs:1548,1701` |
| 4 | GET | `/.well-known/t3/environment` | `main.rs:176`, `reconnect.rs:40`, `connection_access.rs:60,111`, A5, A6 |
| 5 | GET | `/api/orchestration/threads/{id}/bounded?compactTurnItems=1` | `lib.rs:479-487` |
| 6 | GET | `/api/orchestration/threads/{id}/history?cursor=…` | `write.rs:55` |
| 7 | POST | `/api/pull-requests/diff` | `write.rs:301` |
| 8 | GET | `/api/auth/clients` | `connection_access.rs:65`, A4 |
| 9 | POST | `/api/auth/clients/revoke` | `connection_access.rs:104`, A4 |
| 10 | POST | `/api/auth/clients/revoke-others` | `connection_access.rs:104` |
| 11 | GET | `/api/auth/pairing-links` | `connection_access.rs:66` |
| 12 | POST | `/api/auth/pairing-links/revoke` | `connection_access.rs:104` |
| 13 | POST | `/api/auth/pairing-token` | `connection_access.rs:100` |
| 14 | GET | `/api/connect/link-state` | `connection_access.rs:69` |
| 15 | POST | `/api/connect/preferences` | `connection_access.rs:118` |
| 16 | POST | `/oauth/token` | A5 (la lib ya la usa para DPoP) |
| 17 | GET | `/api/assets/{firmado}` | A1–A3, más `Connection::attachment_image` y `media_image` en `images.rs:16-20` |

Solo dentro de la lib: `/api/attachments/upload/{grant}` (POST), `/api/auth/websocket-ticket`, `/ws`, `/v1/client/dpop-token`, `/v1/environments/{id}/connect` y `/api/device-hub/*` (la base la configura el servidor, `device_screen.rs:175`).

### 1.C WebSocket / streams / SSE

| archivo:línea | tipo | tag | helper | API de la lib |
|---|---|---|---|---|
| `src/device_screen.rs:304,352,368,404,431` | WebSocket + MJPEG | hub de dispositivos: `device_hub`, `device_mjpeg`, `device_socket`, `device_screenshot` | **Ya usa la lib** (`device_hub.rs`). Pero `:345-346,388,425,446,453` manejan `tungstenite::Error` y `tungstenite::Message` directamente | **NUEVA:** `DeviceStream` con `recv() -> Result<DeviceFrame>` y `send_input(&Value)`, sin exponer `tungstenite` ni `reqwest::Response`. Para JNI hace falta además un modo con *callbacks* o sondeo, como el de `ffi_stream`. |
| `src/live.rs:36` | stream (RPC) | `orchestration.subscribeThread` | `Connection::subscribe_probed` | Existe. El bucle de reconexión, backoff y `held` envelope (`live.rs:12-76`) vive en ot3. **NUEVA:** `ThreadLive` en la lib que posea esa máquina de estados (ya existe un esbozo en `ffi_stream.rs`). |
| `src/shell_watch.rs:87` | stream | `orchestration.subscribeShell` | `subscribe_probed` | Igual que `live.rs`. `shell_state.rs` es el reductor puro (§4). |
| `src/terminal.rs:67` | stream | `terminal.attach` | `subscribe` | Existe (`protocol::terminal`). |
| `src/command_output.rs:80`, `worktree_setup.rs:22`, `local_servers.rs:13`, `preview_transport.rs:309`, `integration_devices.rs:188`, `keybinding_settings.rs:52`, `quota_sources.rs:170`, `storage_cleanup.rs:19`, `provider_setup.rs:12`, `git_operations.rs:212` | streams | `orchestration.subscribeCommandOutput`, `subscribeWorktreeSetup`, `subscribeDiscoveredLocalServers`, `subscribePreviewEvents`, `subscribeDeviceState`, `subscribeServerConfig` (×2), `server.getStorageCleanupReport`, `provider.install.subscribe` / `provider.auth.subscribe`, tag dinámico | `subscribe` | Existen. Les faltan constructores de payload y reductores tipados. |
| (tests) `tests/questions.rs:10`, `suggestions.rs:12`, `queue.rs:9`, `approvals.rs:42`, `project_browser.rs:22`, `runtime_models.rs:7`, `writes.rs:17-58`, `images.rs:5`, `thread_launch.rs:12`, `stream_probe.rs` | servidores WS falsos con `tungstenite::accept` | — | — | Se quedan: son fixtures. `tungstenite` debe pasar a `dev-dependencies` de ot3 cuando `device_screen` deje de usarlo. |
| SSE | — | — | — | No existe en ot3 ni en la lib. |

**Tags de stream únicos: 12** (`orchestration.subscribeShell`, `orchestration.subscribeThread`, `orchestration.subscribeCommandOutput`, `terminal.attach`, `subscribeDeviceState`, `subscribeDiscoveredLocalServers`, `subscribePreviewEvents`, `subscribeServerConfig`, `subscribeWorktreeSetup`, `server.getStorageCleanupReport`, `provider.install.subscribe`, `provider.auth.subscribe`).

### 1.D RPC (Effect-RPC por WebSocket): ya usa la lib, pero con el tag escrito en ot3

155 líneas con `rpc(` / `rpc_with_timeout(` en 47 archivos (`src/write.rs:8-9` es la definición del helper, así que quedan 153 sitios). El Apéndice A las lista una por una con helper y tag. Resumen:
- **`write::rpc(e, tag, payload)`** (`write.rs:6-10`) es el único punto que elige entre `e.connection` (relay) y `Connection::new(base_url, Bearer(token de escritura))`. Otros 11 módulos tienen su propia `connection(e)`: son los 27 constructores del Apéndice C.
- 136 sitios llevan el tag literal. Los otros 17 lo reciben de un llamador (marcados «dinámico» en el Apéndice A).
- **Tags de petición únicos: 98** (lista completa en §1.F). La lib solo tiene constructores tipados para parte de ellos: `terminal.*`, `models.*`, `documents.*`, `projects.*`, `queue.*`, `pull_requests::request`, `device_hub`, `composer`, `approvals`, `questions`, `tasks`, `history`.
- **Deuda de paridad con Android:** si el cliente Kotlin llama `rpc("vcs.refreshStatus", …)` por su cuenta, reimplementa lo que hoy está disperso en `git_controls.rs`, `branch_picker.rs`, `provider_instances.rs`, etc. Por eso §4 propone mover esos módulos y no solo el transporte.

### 1.E Los 18 archivos que importan `reqwest::` solo por `Url`

`clipboard.rs`, `browser_guest.rs`, `connection_cloud.rs`, `browser_address.rs`, `third_party_licenses.rs`, `quota_sources.rs`, `git_operations.rs`, `pull_request_actions.rs`, `videos.rs`, `omabridge_gateway.rs`, `connection_preferences.rs`, `connection_access.rs`, `write.rs`, `document_session.rs`, `browser_import.rs`, `markdown_images.rs`, `main.rs`, `desktop_open.rs`. Más `images.rs`, `desktop_cloud.rs` y `omabridge_route.rs`, que usan `Url` y además un cliente. Y `connections.rs`, `connections_management.rs`, `checkpoint_operations.rs`, `notification_icon.rs`, `server_updates.rs` y `lib.rs`, que tienen los otros usos de §1.A y §2.

Estos usos de `reqwest::Url` son parseo o validación de URLs, o `Url::from_file_path` / `to_file_path` para pasar rutas a QML. Se resuelve con `pub use` de `url::Url` (la lib ya la importa vía `reqwest::Url`) o con la crate `url` directa. **No es tráfico.** Pero es lo que impide quitar `reqwest` de `Cargo.toml` de ot3, así que conviene tratarlo como un paso mecánico propio.

### 1.F Tags RPC únicos en `src/` (98 de petición + 12 de stream = 110)

- `server.*` (19 de petición): `getConfig`, `getSettings`, `updateSettings`, `updateProvider`, `refreshProviders`, `refreshUsageRates`, `getUsageSummary`, `getHostResources`, `getProcessDiagnostics`, `getResourceTelemetryHistory`, `signalProcess`, `discoverSourceControl`, `checkOtlpEndpoint`, `runStorageCleanup`, `reportClientActivity`, `searchAcpRegistry`, `prepareAcpRegistryAgent`, `upsertKeybinding`, `removeKeybinding`, entre otros.
- `orchestration.*`: `dispatchCommand`, `getArchivedShellSnapshot`, `getThreadProjection`, `getTurnItem`, `getTurnDiff`, `getFullThreadDiff`, `getWorkflowScript`, `stopWorkflow`, `searchThreads`, `launchThread`.
- `vcs.*`: `refreshStatus`, `listRefs`, `createRef`, `switchRef`, `pull`, `init`. Más `git.runStackedAction` y `git.preparePullRequestThread`.
- `review.*`: `getDiffPreview`, `getDiffFileContents`.
- `projects.*`: `readFile`, `searchEntries`, `mutate`, `createNew`. Más `projectClone.start`, `filesystem.browse`, `filesystem.getMetadata`.
- `preview.*` (9): `list`, `open`, `navigate`, `resize`, `adjust`, `refresh`, `close`, `clearProfile`, `reportStatus`.
- `pullRequests.*` (12): `detail`, `checks`, `activity`, `comment`, `labelCandidates`, `setLabels`, `requestReviewers`, `reviewerCandidates`, `runAction`, `submitReview`, `routing`, `routingIdentity`.
- `provider.*` (7), `device.*` (7: `list`, `configure`, `testHost`, `open`, `detail`, `action`, `close`), `scheduledTasks.*` (5), `mcpApps.*` (3), `agentSessions.*` (2).
- Sueltos: `assets.createUrl`, `attachments.delete`, `secrets.answerRequest`, `shell.openInEditor`, `worktreeSetup.cancel`, `terminal.open`, `terminal.write`.

---

## 2. Lo que NO es tráfico T3 y debe quedarse en ot3

| archivo (líneas) | qué hace | justificación |
|---|---|---|
| `src/local_quotas.rs` (503; HTTP en `:181,192,193,334`) | `Client::fetch[_headers]` a `chatgpt.com/backend-api/wham/usage`, `api.kimi.com/coding/v1/usages`, `portal.nousresearch.com/api/oauth/account` y `opencode.ai/zen/go/v1/usage`, con tokens de proveedores leídos de archivos locales (`~/.hermes/.env`) | APIs de terceros con credenciales ajenas a T3. La lib fuerza mismo origen en `endpoint()` (vendor `lib.rs:82-91`) y **debe** rechazar esto. `Client::fetch*` queda solo para esto: renombrar a `ExternalHttp` y moverlo a `local_quotas.rs`. |
| `src/lib.rs:393-471` (`Client` + `fetch*`) | cliente genérico 8 s / 3 s, `bearer_auth` a URL absoluta | Ídem. Cuando A1–A6 migren, `Client::get` y `bounded_thread` pasan a ser azúcar sobre `Connection`. |
| `src/server_updates.rs:95-110` (`latest`) | `GET api.github.com/repos/pingdotgg/t3code/releases`, con `GH_TOKEN` / `GITHUB_TOKEN` opcionales | GitHub API, no un servidor T3. Las partes puras de este archivo sí son portables (§4). |
| `src/desktop_cloud.rs:222-275` (+ `native_config`, `native_client_token`) | Clerk: `GET {clerk}/v1/client?_is_native=1` y `POST …/sessions/{id}/tokens/{template}`. Lee el almacén cifrado del cliente oficial de T3 (`decrypt_storage`) y raspa la configuración pública del bundle web | Es el login de la cuenta, y el README de la lib dice «Hosts own account login and key storage». Lo que viene después (`connect_relay`, `RelayClient`) ya usa la lib (`desktop_cloud.rs:281,290,301`). Android usará su propio flujo de sesión. |
| `src/images.rs:29-45` (`direct`) | Descarga imágenes Markdown públicas `http(s)://` o `data:` (redirect ≤ 3, 20 s, 10 MiB) | Sin credenciales T3 (el comentario de `images.rs:27` lo dice). La caché `~/.cache/…/images` (`fs`, `libc::geteuid`, SHA-256) es del host. La parte T3 (`attachment_image`, `media_image`) ya usa la lib. |
| `src/omabridge_route.rs`, `omabridge_gateway.rs`, `omabridge_device.rs`, `omabridge_install.rs` | Proceso OmaBridge (`connect`, forward, `state_dir`, instalación) y validación de URLs del relay | Transporte propio del host: procesos y estado en disco. **Excepción:** `verify_environment` (`omabridge_route.rs:93-98`) es una petición T3, A6 del §1.A. |
| `src/connections.rs` (SSH, descubrimiento, catálogo) | `ssh -L` con `prctl`, `REMOTE_DISCOVERY`, `decrypt_catalog`, lectura de `server-runtime.json`, `TcpListener` | Procesos, SSH, rutas del host. **Excepción:** `prune_sessions` (A4). |
| `src/connection_cloud.rs` | Adaptador del CLI de T3 Connect: `Command::new(&cli.executable)` (`:334`) | Procesos; sin HTTP. |
| `src/connection_access.rs:13-37,73-91` | `local_credential` (ejecuta `t3 auth session issue`), `qrencode`, escritura de token 0600 | Procesos y archivos. El HTTP de este archivo ya usa la lib. |
| `src/journal_vault.rs` (2 054) | git, `gh repo create`, `xdg-open`; solo lee remotos por RPC (`RpcFiles`, `:191,200`) | Bitácora Obsidian: filesystem, procesos y git. Sus RPC están en el Apéndice A. |
| `src/desktop_open.rs`, `documents.rs:38`, `journal_vault.rs:1310,1844`, `qml_preview.rs`, `document_session.rs` | `xdg-open`, procesos de renderizado QML | Abrir URLs/archivos y UI del escritorio. |
| `src/browser_*.rs`, `preview_transport.rs`, `preview_actions.rs`, `preview_command.rs`, `preview_watch.rs` | Navegador embebido (cookies, perfiles, SQLite, `browser_guest`) | Específico de Qt WebEngine. Las RPC `preview.*` se listan en el Apéndice A, y Android no tendrá este navegador. |
| `src/clipboard.rs`, `scroll.rs`, `window_layout.rs`, `notification_icon.rs` (salvo A2), `ui_state.rs`, `drafts.rs`, `prompt_stash.rs`, `localization.rs`, `keybinding_watch.rs` | wl-clipboard, Hyprland, notificaciones, estado en disco | UI y sistema del escritorio. |
| `src/third_party_licenses.rs`, `tools/build-licenses.cjs` | Licencias: solo valida URLs, sin red | Datos generados en build. |
| `src/videos.rs` (35) | Caché de video en disco; la parte T3 es `attachment_bytes` (A1) | La caché y el `Url::from_file_path` para QML son del host. |
| `native/dictate/src/main.rs:42` (HuggingFace), `bin/fetch-omabridge`, `install.sh` | Descarga de modelos Whisper, binarios y el instalador | Fuera de `src/`. No toca T3. |
| `src/backend_service.rs`, `build_cleanup.rs`, `disk_space.rs`, `process_diagnostics.rs` (RPC), `storage_cleanup.rs` (RPC), `usage_summary.rs` (RPC) | Limpieza de disco local y servicios systemd | Sistema del escritorio. Las RPC están en el Apéndice A y son candidatas a lote 4. |

---

## 3. Diferencias de comportamiento: `Client::fetch_headers` (ot3) vs `Connection::request_json` (lib)

Referencias: ot3 `src/lib.rs:393-471`. Lib: `vendor/t3-protocol/src/lib.rs` (`http()` `:92-99`, `decode_with_limit` `:103-142`, `request_json` `:361-393`). `Client::get` ya envuelve `request_json` (`lib.rs:472-478`), así que **la comparación efectiva es entre «cliente HTTP de ot3 inyectado» y «cliente HTTP de la lib»**.

| Aspecto | `Client::fetch_headers` | `Connection::request_json` | Consecuencia |
|---|---|---|---|
| Método y cuerpo | solo GET, sin cuerpo | cualquier método, cuerpo JSON opcional (`req.json`) | La lib es un superconjunto. |
| Destino | URL absoluta a cualquier host | solo ruta relativa de **mismo origen** (`endpoint()` rechaza `//`, `#`, `\`, otro origen) | `fetch` con token a terceros no es migrable. Se queda en ot3 (§2). |
| Credencial | `bearer_auth(token)` | `Credential::Bearer` (mismo `authorization: Bearer`) o `Dpop` (prueba ES256 por petición, `ath`, `jti`) | Equivalente para Bearer. La elección entre token de lectura y de escritura (`environment_token(e, write)`) es del host. |
| Cabeceras fijas | `Accept: application/json`, `Accept-Encoding: identity`, `x-t3-orchestration-protocol: 2` si `protocol` | Las mismas | Idénticas. `tests/transport.rs:24-48` las fija para `fetch`. |
| Cabeceras extra | `&[(&str,&str)]` | **no existe** | Solo las usan los terceros de §2 (`ChatGPT-Account-Id`, `User-Agent`). No hace falta en la lib. |
| Timeout | 8 s total, 3 s de conexión, `pool_idle_timeout` 90 s (`:398-408`) | 12 s total, sin timeout de conexión ni `pool_idle` | `Client::get` inyecta el cliente de ot3 (`with_http_client`), así que esos sitios conservan 8 s / 3 s. Los 26 `Connection::new` directos **ya** usan 12 s. Hoy hay dos políticas de timeout para las mismas rutas. **La lib debe exponer** `ConnectionOptions{ request_timeout, connect_timeout }` con valores por defecto explícitos. |
| Redirects | `Policy::none()`, y `302` da `http_302` | `Policy::none()` | Igual. Solo `prune_sessions` (A4) y `verify_environment` (A6) siguen redirects (límite 10 de reqwest). |
| Proxy | `no_proxy()` | `no_proxy()` | Igual. **A4 y A6 no lo desactivan:** hoy respetan `HTTP(S)_PROXY`. Migrarlos lo corrige, pero es un cambio de comportamiento a anotar. |
| 401 / 403 | `unauthorized` | `unauthorized` | Igual. |
| Otro no-2xx | `http_<código>` | `http_<código>` | Igual. Ninguno devuelve el cuerpo del error HTTP (las RPC sí devuelven `code: reason`, `rejection`). |
| Error de red | `unreachable` | `unreachable` | Igual. |
| `Content-Type` | compara `.to_lowercase()` con `application/json` | compara **sin pasar a minúsculas** (`:111-123`) | **Diferencia real:** `Application/JSON` se rechazaría con `not_json`. La lib debe normalizar a minúsculas. |
| `Content-Encoding` | acepta ausente, vacío o `identity` | igual (`None \| identity \| ""`) | Igual (`unsupported_encoding`). |
| Límite de bytes | 8 MiB fijo (`MAX_BYTES`, `too_large`) | 8 MiB, **64 MiB** para `GET /api/orchestration/threads/{id}/bounded` con `protocol=true` (`THREAD_SNAPSHOT_MAX_BYTES`). El WS usa 64 MiB para `subscribeThread` con `acceptBoundedSnapshot` | `bounded_thread` ya recibe 64 MiB por la lib. El límite se decide por la forma de la ruta (`:385-391`), ignorando la query. **La lib debería exponer el límite por llamada** (`opts.max_bytes`). |
| Buffer | `Vec<u8>` | `Zeroizing<Vec<u8>>` | La lib es más estricta. |
| Respuesta vacía / 204 | no-JSON → `not_json` | igual | `prune_sessions` (A4) ignora el cuerpo. Si el endpoint responde 2xx sin JSON, `request_json` fallaría. **Hace falta** `request_ok(method, path, body) -> Result<()>` que acepte cualquier 2xx (o confirmar que `/api/auth/clients/revoke` devuelve JSON: `connection_access.rs:104` ya lo usa con `request_json`, así que sí). |
| Reintentos | ninguno | ninguno (las mutaciones nunca se reintentan; `rpc_outcome_unknown`) | Igual. |
| Pool de conexiones | `Client::new()` crea un pool nuevo en cada llamada (23 sitios en 18 archivos) | `Connection::new` crea un `reqwest` nuevo en cada llamada (26 sitios), y cada `rpc` abre TCP + TLS + upgrade nuevos (`:451-483`) | Sin *keep-alive* entre llamadas en toda la app. Para ot3 es coste. Para Android es batería. **Mantener un `Connection` por entorno y credencial.** La lib ya es `Clone` y comparte `Http`. |
| Resolución DNS / conexión WS | — | `to_socket_addrs()` bloqueante (sin timeout propio), `connect_timeout` 8 s, lectura y escritura 12 s, plazo de `rpc` 20 s fijo (`:426-437,459`) | `rpc` **no usa** el cliente inyectado: no hereda 8 s / 3 s. `rpc_with_timeout` (`long_rpc.rs`, máx. 300 s) existe, y ot3 la usa en 9 sitios. |
| Validación de base | `validate_base` (`lib.rs:378-392`): ruta `""` o `/`, sin query ni userinfo, puerto ≠ 0 | `http_url` + ruta `/` exacta + ws mismo origen (`:253-298`) | Equivalentes. Cuidado con una base con `/` final, que la lib acepta. |
| Cuerpo de error del servidor | descartado | descartado (HTTP). Solo la RPC añade `reason` (primeros 300 car.) | Para Android, `error_code()` ya separa código y motivo. |

**Lo que la lib tendría que absorber para que ot3 no pierda comportamiento:**

1. `ConnectionOptions` con timeout de petición y de conexión (hoy 8 s / 3 s en lecturas de ot3 frente a 12 s en la lib), `pool_idle_timeout` y `max_bytes` por llamada. Valores por defecto fijados en un solo sitio.
2. Comparación de `Content-Type` insensible a mayúsculas.
3. `request_ok` (2xx sin cuerpo JSON) para las revocaciones.
4. `fetch_descriptor` sin credencial y `pair_bearer` (A5/A6), con las mismas garantías de origen y de no enviar credenciales antes de verificar el `environmentId`.
5. `asset_bytes` genérico (A1–A3) con los códigos de error que hoy distinguen los llamadores (`attachment_unreachable`, `attachment_unavailable`, `attachment_too_large`, `revert_attachment_unavailable`, `revert_attachment_changed`, `image_unreachable`, `image_unavailable`, `image_limit_10MiB`). La lib hoy solo tiene los códigos `image_*`.
6. Constructor de `Connection` reutilizable por entorno (`with_http_client` ya existe) y un par de credenciales lectura/escritura.
7. `bounded_snapshot::restore` (`bounded_snapshot.rs`, 39 líneas, puro) y la ruta `bounded?compactTurnItems=1`. Hoy están partidos entre `Client::bounded_thread` y la regla de 64 MiB de la lib.
8. Para `device_screen`, un tipo de frame neutro, sin `tungstenite` ni `reqwest::Response` en la API pública.

**Riesgos de migración que conviene decidir antes:** las peticiones A4 y A6 pasan de «respeta el proxy del sistema, sigue redirects» a «sin proxy, sin redirects», que ya es la política del resto de las lecturas. Sería un cambio de comportamiento para quien tenga `HTTP_PROXY` definido. A5 conserva 8 s.

---

## 4. Módulos de `src/` portables a la lib

Criterio: lógica pura sobre JSON de T3, sin `std::fs`, procesos, keyring, Quickshell, SSH ni rutas del host. Los números de línea son del `HEAD` auditado. Las «líneas portables» son una estimación a partir de dónde empieza el código con `Environment`, `Client` o `write::rpc`.

**Prerrequisito común.** Casi todos hacen `use crate::*` y dependen de los ayudantes de la raíz de `lib.rs`: `text`, `truth`, `valid_id`, `epoch`, `Result` y `json!` (`lib.rs:105-143`, unas 40 líneas puras). Hay que moverlos primero a la lib, en un `util` público, y reexportarlos desde ot3 para no tocar los módulos.

### 4.A Portables ya (puros; mover tal cual)

| módulo | líneas | portables | dependencias internas | usado por | tests que lo cubren | notas |
|---|---|---|---|---|---|---|
| `checkpoint_revert.rs` | 327 | 327 | `serde_json`, `uuid` | `lib.rs:509` (`normalize_messages_cached`), `checkpoint_operations.rs` | `tests/checkpoint_operations.rs`, `tests/checkpoint_document.rs`, QML `checkpoint-revert`, `checkpoint-flow` | `Safety`, `command`, `observation`, `reference`, `can_preview`. |
| `plan_workflow.rs` | 347 | 347 | `serde_json`, `chrono` (solo `parse_from_rfc3339`) | `lib.rs:519,526`, `checkpoint_operations.rs` (que además usa `crate::active_run_id`, `lib.rs:596`) | `tests/plan_workflow.rs`, `tests/plan_transport.rs`, QML `plan-workflow`, `plan-flow`, `plan-preference` | `view(p, rendered_markdown)` recibe el renderizador como parámetro (ya inyectado). |
| `prompt_recall.rs` | 290 | 290 | ninguna | `lib.rs:506,509` (`attribution`, `recall`) | `tests/prompt_recall.rs` | Sin dependencias del host. |
| `latex.rs` | 305 | 305 | ninguna | `lib.rs:162-170` (`markdown_render` con `math`) | tests en el archivo (`:284,300`), `lib.rs` (`math_markdown_tests`) | **Puro, pero emite el HTML de rich text de Qt.** Para Android conviene parametrizar el renderizador (trait `RichTextSink`) o dejar la salida como está si Kotlin usa `Html.fromHtml`. |
| `bounded_snapshot.rs` | 39 | 39 | ninguna | `lib.rs:486` (`Client::bounded_thread`) | `tests/bounded_snapshot.rs` (en ot3) | Debe ir junto con la ruta `bounded?compactTurnItems=1` (§3, punto 7). |
| `message_text.rs` | 13 | 13 | ninguna | `lib.rs:507` | tests de `normalize_messages` | Trivial. |
| `storage_settings.rs` | 210 | 210 | `text`, `json!`, `Result` de la raíz | `native_settings.rs`, `scoped_settings.rs`, `write.rs` | `tests/storage_settings.rs`, QML `storage-settings`, `storage-page` | Ajustes del servidor, sin I/O. |

Total puro completo: **1 531 líneas**.

### 4.B Parte pura portable ya; la orquestación necesita una dependencia inyectada

La columna «parte host» dice qué hay que inyectar. En casi todos basta con recibir `&protocol::Connection` en lugar de `&Environment`. La elección de token y de entorno (`environment_token`, `e.connection_error`, `e.connection`) queda en ot3.

| módulo | líneas | parte pura (portable) | parte host (a inyectar) | dependencias internas | tests |
|---|---|---|---|---|---|
| `diff_review.rs` | 213 | 1-188: `present`, `scoped_result` y el parser de diff (≈ 188) | `execute` (`:189-213`): `Connection::new` + `rpc(tag)` → `&Connection` | `review_document` (solo en `execute`) | `tests/diff_review.rs`, QML `diff-review`, `native-diff-flow` |
| `tool_inspector.rs` | 353 | `compact_reference`, `scope`, `request`, `parse` y el acotado de salida (≈ 334) | `execute` (`:102-120`): `Connection::new(token de lectura)` → `&Connection` | usado por `lib.rs:599` y `failure_context` | `tests/tool_inspector.rs`, QML `tool-inspector`, `activity-inspector` |
| `quota_sources.rs` | 183 | `limits`, `providers`, `sources`, `apply` (1-135) | `connection`, `authorize`, `subscribe` (136-183) → `&Connection` | `quotas.rs` | `tests/quota_sources.rs`, QML `usage-limits` |
| `git_controls.rs` | 379 | `scope`, `build_action`, `build_verified_action` y helpers (≈ 330; `sha2`, `uuid`) | `execute` (`:225-274`): `Connection::new` + `request_json shell` + 3 `rpc` | `working_review` (`scope`) | `tests/git_controls.rs`, QML `git-controls`, `git-flow` |
| `provider_instances.rs` | 226 | `projection`, `mutation`, `confirm_target` (≈ 203) | `execute` (`:204-226`): `Client::get session` + 5 `write::rpc` | ninguna | `tests/provider_instances.rs`, QML `provider-instances` |
| `scoped_settings.rs` | 193 | `project_file`, `effective`, `project_patch`, `aggregate`, `validated_targets`, `write_was_dropped` (1-123) | `execute_in(&[Environment])` (`:124-193`): multi-entorno, `Client` + `write::rpc` + `projects.readFile` → `Fn(env_id) -> &Connection` | `native_settings`, `storage_settings` | `tests/scoped_settings.rs`, QML `preferences-scope`, `project-preferences` |
| `native_settings.rs` | 229 | `patch`, `snapshot`, `idle_compaction*`, `model_selection` (≈ 200) | `execute` (`:214-229`) | `storage_settings` | `tests/native_settings.rs`, QML `preferences*` |
| `branch_picker.rs` | 223 | `dedupe`, `derive_local`, `sanitize`, `selection_target`, `temporary_branch`, `mismatch` (≈ 100) | `refs`, `switch`, `check`, `resolve`: `Client::get shell` + 7 `rpc` | ninguna | tests en el archivo (`:130,214`), QML `branch-fork`, `branch-mismatch`, `git-flow` |
| `pull_request_actions.rs` | 180 | `payload`, `thread_command`, validadores (≈ 100) | `run`, `thread_link`, `open_thread`: `rpc` | **solapa con `vendor/t3-protocol/src/pull_requests.rs`** (245 líneas); consolidar | tests en el archivo (`:90,149`), QML `pr-actions`, `pull-requests` |
| `thread_details.rs` | 74 | `editors`, `workspace` (≈ 25) | `read`, `open`: `Client::get shell` + `server.getConfig`, `shell.openInEditor` | ninguna | tests en el archivo (`:61,69`), QML `thread-details` |
| `failure_context.rs` | 173 | `clip`, `output_text`, `failed`, `present`, `detail`, `input_summary`, `candidates` (1-101) | `read_item`, `run`, `tool` (`:102-139`): `bounded_thread` + `rpc` | `tool_inspector::request` | tests en el archivo (`:143,159,169`), QML `failure-context` |
| `working_review.rs` | 286 | `present`, `option`, `name`, `refs_payload`, `refs_present` (≈ 190) | `connection`, `scope`, `authorize`, `refs`, `execute` (`:6-24,126-140,211-286`) | `git_controls::scope`, `diff_review`, `review_document` | `tests/working_review.rs`, QML `working-review-flow` |
| `review_document.rs` | 201 | `has_text_patch`, `remember*` (`:14-69`) | **Estado global** `static SNAPSHOTS` (`:10`): pasar a un store inyectado. `bundle()` (`:95`) embebe el visor JS con `include_bytes!`, y `render` (`:107-134`) genera el HTML de ese visor: eso es **UI de Qt**, se queda en ot3 | `working_review`, `documents` | `tests/review_document.rs`, `tests/documents.rs` |
| `shell_state.rs` | 245 | el reductor completo (1-165) | solo el `static SHELLS` (`:18`) y `with()` (`:20`): convertir a `struct ShellStore` y que ot3 guarde la instancia | `shell_watch`, `disk_space.rs:109`, `lib.rs:834` | 6 tests en el archivo (`:176-236`) |
| `suggestions.rs` | 33 | la consulta y el recorte de resultados | caché global `OnceLock<Mutex<…>>` con `Instant` y `Client::new()` | ninguna | `tests/suggestions.rs` |
| `integration_devices.rs` | 522 | `present_host`, `present_state`, `host` (≈ 195) | `connection`, `read`, `watch`, `apply`, `subscribe_state` (`:18-32,183-300,362-…`) | `provider_setup`, `local_servers`, `storage_cleanup`, `device_screen` usan su `connection` | `tests/integration_devices.rs`, QML `integrations` |
| `keybinding_settings.rs` | 296 | `present`, `payload` (`:106-216`) | `read`, `apply`, `open`, `subscribe_changes` | ninguna | `tests/keybinding_settings.rs`, QML `keybindings-*` |
| `archive_management.rs` | 408 | `present` (`:74-214`, ≈ 140) | outbox en disco con `xdg(...)`, `read_dir` (`:126-131,229`) → trait `Outbox`; `read`, `action`, `observe` | `checkpoint_journal` (patrón compartido) | `tests/archive_management.rs`, QML `archive` |
| `usage_summary.rs` | 134 | `window`, `aggregate` (`:22-93`, `chrono`) | `time_zone` (`:9-20`: `std::env::var("TZ")` y `read_link("/etc/localtime")`) → parámetro; `read` (`:94`) | ninguna | test en el archivo, QML `usage-page` |
| `server_updates.rs` | 194 | `exact`, `channel`, `compare`, `newest`, `advisory`, `quiet`, `due` (`:29-139`, ≈ 100) | `latest` (GitHub, §2) y `route` (lee `userdata/environment-id` y ejecuta `timeout`) | ninguna | `tests/server_updates.rs`, QML `server-updates` |
| `reconnect.rs` | 302 | `transient`, `Backoff`, `Waiter`, constantes (≈ 100) | `reachable` (`:39`, `Client::get` → `Connection::fetch_descriptor`), `wake` (`:88`, `gdbus`) | ninguna | 5 tests en el archivo, QML `backend-reconnect` |
| `connection_github.rs` | 35 | `permission`, `identities_match` (`:4-13`) | `read` (`:14-35`): enruta entre entornos con `environments()` y `read_catalog()` | `connection_preferences` | tests de `connection_preferences`, QML `connection-routing` |
| `connection_preferences.rs` | 241 | `github_*`, `choose_load_balanced_environment`, `choose_shared_project_environment` (`:147-241`, ≈ 95) | lectura y escritura de `client-settings.json` con `fs`, `flock`, `t3_home()` (`:1-146`) → trait de almacenamiento | `connection_github`, `connection_route` | `tests/connection_preferences.rs`, QML `connection-preferences` |
| `connections.rs::stale_ot3_sessions` | 10 | `:157-166` (puro, `chrono`) | — | `prune_sessions` (A4) | `tests/connections.rs` | Debe ir con A4 (`revoke_stale_sessions`). |
| `connection_route.rs` | 49 | `eligible_projects`, `provider_eligible` (`:5-20`) | `choose` (`:21-49`): `Client::get /api/projects`, `write::rpc`, `fs::read_to_string(environment-id)`, hilos | `connection_preferences` | `tests/connection_route.rs`, QML `connection-routing` |

### 4.C Fragmentos de `src/lib.rs` (≈ 450 líneas puras de derivación)

El `lib.rs` de ot3 tiene la mayor parte de lo que un cliente Android reescribiría. **No son módulos aparte**, pero hay que contarlos.

| líneas | contenido | dependencia host |
|---|---|---|
| `144-263` | `markdown_render`, `markdown_tables`, `markdown_search_text`, `inline_code_path`, `file_references` | `pulldown-cmark` (la lib ya lo declara). Los enlaces `ot3-image-open://`, `ot3-code-run://`, `ot3-table-copy://` y la etiqueta `<!--ot3-qml:…-->` son contrato **QML**. Parametrizar el esquema o dejarlo detrás de una opción `Surface::Qt`. `math_enabled()` (`:146-154`) lee `client-settings.json` y se sustituye por un parámetro `math: bool` (ya existe en `markdown_render`). |
| `493-527` | `normalize_messages[_cached]`, `RenderCache` | usa `message_text`, `markdown_images` (`workspace_path` usa `Url::to_file_path`), `checkpoint_revert`, `prompt_recall`, `plan_workflow`, `protocol::queue/approvals/questions/tasks/code_blocks`. El `math_enabled()` de `:500` es la única llamada al host. |
| `529-590` | `subagent_title`, `subagent_summary`, `context_window`, `thread_workflows`, `is_settled` | puro |
| `596-688` | `active_run_id`, `live_activity`, `conversation_timeline`, `rolled_back_runs` | usa `protocol::activity` y `tool_inspector::compact_reference` |
| `690-825` | `normalize` (sidebar), `project_identity` | usa `protocol::sidebar` |
| `826-899` | `snapshot(&Client)` | **No portable:** compone `shell_state` con `Client::get`, grupos locales y estado de UI. |

### 4.D No portables (se quedan en ot3)

| módulo | motivo |
|---|---|
| `checkpoint_journal.rs` (165) | `fs`, `flock`, `OpenOptions` y rutas `xdg("XDG_STATE_HOME")` (`:20,29,147`). Es la persistencia del outbox. Se abstrae como `trait Journal` y Android implementa la suya. |
| `checkpoint_operations.rs` (426) | **Casi portable con tres inyecciones:** `Rpc` (`write::rpc` y `Connection`), `Journal` (arriba) y `Assets`/`upload` (A3). La lógica de `observe_record`, `request` y `scope` es de dominio. El riesgo es alto: lleva el protocolo de recuperación atómica que prueban sus tests. |
| `quotas.rs` (309) | Hilos, `emit`, `Mutex`, `Instant` y `local_quotas` (archivos del host). Solo reutiliza `quota_sources::apply`. |
| `auto_compact.rs` (360) | Ledger en `fs`, hilo vigilante, `emit` y `ui_state`. La política está en `protocol::compaction`. |
| `git_operations.rs` (389) | Orquesta pasos con `terminal` y RPC, y lee estado del host. Es candidata solo después de L4. |
| `device_screen.rs` (521) | `ffmpeg` como proceso, `emit`, hilos y frames al disco. La parte T3 ya usa la lib (§1.C). |
| `browser_*`, `preview_*`, `omabridge_*`, `connections.rs`, `desktop_cloud.rs`, `journal_vault.rs`, `local_quotas.rs`, `main.rs`, `ui_state.rs`, `drafts.rs`, `clipboard.rs`, `backend_service.rs`, `build_cleanup.rs`, `disk_space.rs` | Procesos, archivos, SSH, keyring, Qt, Quickshell (§2). |

### 4.E Grafo de dependencias entre candidatos

```
util (text, truth, valid_id, epoch, Result)         ← todos
latex ─────────────────────────────► markdown_render ──┐
prompt_recall ─────────────────────► normalize_messages_cached
checkpoint_revert ─────────────────►        (lib.rs:493)  ◄── plan_workflow, message_text, markdown_images
                      └──► checkpoint_operations ◄── checkpoint_journal (host)
bounded_snapshot ──► bounded_thread (ruta + límite de 64 MiB)
tool_inspector::compact_reference ──► live_activity (lib.rs:597)
tool_inspector::request ──► failure_context
git_controls::scope ──► working_review ──► diff_review ──► review_document ◄── documents
storage_settings ──► native_settings ──► scoped_settings
quota_sources::apply ──► quotas (host)
shell_state ──► shell_watch (host) · disk_space (host) · lib.rs snapshot (host)
connection_preferences ──► connection_github · connection_route
stale_ot3_sessions ──► prune_sessions (A4)
```

Orden mínimo sin ciclos: `util` → {`checkpoint_revert`, `plan_workflow`, `prompt_recall`, `latex`, `bounded_snapshot`, `storage_settings`} → {`tool_inspector`, `diff_review`, `quota_sources`} → {`git_controls`, `review_document`} → `working_review` → {`native_settings`, `scoped_settings`, `provider_instances`, `branch_picker`, `pull_request_actions`, `failure_context`, `thread_details`} → `lib.rs` 144-825.

---

## 5. Orden de migración propuesto (5 lotes, de menor a mayor riesgo)

Cada lote deja `cargo build` y los tests en verde y se puede fusionar solo. Antes de cada lote hay una condición de salida comprobable con `grep` para que no se pueda dar por terminado sin cumplirla.

Notas de ejecución:
- Tests de ot3: `cargo test --locked` en la raíz (93 unitarios en `src/` y 318 de integración en 79 archivos de `tests/`).
- Tests de la lib: `cargo test --locked` y `tools/test-c-abi` en `~/Work/lib-t3-rust`, porque el vendor no incluye `tests/`.
- Tests QML: `tools/test-qml <nombre>` (154 archivos en `tests/qml/`; el script valida el nombre contra una lista cerrada).
- La asociación QML ↔ módulo se hizo por el nombre del test: **no abrí cada archivo QML** para confirmar qué contrato ejercita.
- Línea base conocida, según `Projects/omarchy-t3-code/MEMORY.md` (2026-10-24, **no la verifiqué en esta auditoría**): 8 tests QML de preferencias fallan ya en `main` (`integrations-preferences`, `licenses-preferences`, `keybindings-flow`, `archive-preferences`, `preferences-scope`, `preferences-defaults`, `preferences-design`, `server-updates`) y `row-motion` es intermitente. Hay que registrar esa línea base antes del lote 1 y comparar contra ella, no contra «todo verde».

### Lote 1 — Higiene sin cambio funcional (riesgo bajo)

Qué entra:
1. **Sincronizar** `vendor/t3-protocol` con `lib-t3-rust` (§0.6), fusionar los dos conjuntos de cambios, taguear y actualizar el `rev` de `Cargo.toml`. Sin esto, todo lo demás se apoya en una lib que Android no verá.
2. Un único `Environment::connection(&self, write: bool) -> Result<Connection>` en ot3 que sustituya los 11 `fn connection(e)` y las 16 copias en línea (27 constructores, Apéndice C). **Debe conservar el token de lectura o de escritura que cada sitio usa hoy** (columna «token» del Apéndice C: 15 de escritura, 8 de lectura, 2 variables —`archive_management`, `keybinding_settings`— y 2 con credencial propia: `connection_access.rs:40` emite una con el CLI de T3 y `connections_management.rs:242` usa el token recién canjeado). Memoizar la conexión por `(entorno, write)` para reutilizar el pool de `reqwest` (§3, fila «Pool»).
3. `pub use` de `Url` desde la lib (o la crate `url`) y sustituir `reqwest::Url` en los 18 archivos de §1.E.
4. Un único `Client` compartido en lugar de 23 `Client::new()`.

Tests que lo cubren: `tests/transport.rs` (6), `tests/connections.rs`, `tests/connection_access.rs`, `tests/writes.rs`, `tests/live.rs`, `tests/stream_probe.rs`, `tests/images.rs`, `tests/integration_devices.rs`, `tests/archive_management.rs`, `tests/preview_transport.rs`, `tests/checkpoint_operations.rs`, `tests/git_operations.rs`, `tests/runtime_models.rs`; en la lib, `protocol.rs`, `stream.rs`, `long_rpc.rs`. QML: `connections`, `connection-access`, `backend-reconnect`, `clipboard`, `markdown-images`.

Condición de salida: `grep -rn 'Connection::new(' src` devuelve una sola línea. `grep -rn 'reqwest::Url' src` no devuelve nada.

### Lote 2 — Transporte: cerrar los 6 sitios crudos y las diferencias de §3 (riesgo medio)

Qué entra, todo en la lib con su test de fixture:
1. `ConnectionOptions` (timeouts, `max_bytes`, `pool_idle`) y `Content-Type` insensible a mayúsculas.
2. `Connection::asset_bytes` / `project_favicon` → sustituye A1, A2, A3 y el cliente de `images.rs` para la parte T3.
3. `fetch_descriptor`, `pair_bearer` → A5 y A6. `revoke_stale_sessions` o `request_ok` → A4.
4. `bounded_snapshot` y `Connection::bounded_thread(id)` dentro de la lib (§3 punto 7).
5. Renombrar `Client::fetch*` a `ExternalHttp` dentro de `local_quotas.rs` (único consumidor). `Client` queda solo como azúcar de `Connection`.

Tests que lo cubren: `tests/images.rs`, `tests/notification_icon.rs`, `tests/checkpoint_operations.rs`, `tests/connection_management.rs`, `tests/omabridge_route.rs`, `tests/connections.rs`, `tests/bounded_snapshot.rs`, `tests/transport.rs` (reescribir `headers_and_failure_boundaries` y `discovery_omits_protocol_header`, que hoy llaman a `Client::fetch`, para que usen `Connection::request_json`; fijan las cabeceras y los códigos de error de §3). **Tests nuevos necesarios** (hoy no hay): `attachment_bytes`/`videos.rs` (no hay test de la ruta de 50 MiB), `prune_sessions` (solo se prueba `stale_ot3_sessions`) y `verify_environment`. QML: `connection-omabridge`, `omabridge-manage`, `connection-network`, `live-media`, `checkpoint-flow`.

Condición de salida: `grep -rn 'reqwest::blocking\|Http::builder' src` solo debe aparecer en `images.rs` (`direct`), `desktop_cloud.rs`, `server_updates.rs` y `local_quotas.rs`.

Riesgo que decidir antes de empezar: A4 y A6 pasan a `no_proxy` y sin redirects (§3, última nota).

### Lote 3 — Lógica pura a la lib, sin red (riesgo bajo-medio)

Qué entra: el prerrequisito `util`, §4.A completo, y las partes puras de §4.B que no cambian firmas: `diff_review`, `tool_inspector`, `quota_sources`, `git_controls` (`scope`, `build_*`), `provider_instances`, `scoped_settings`, `native_settings`, `branch_picker`, `pull_request_actions` (fusionar con `protocol::pull_requests`), `thread_details`, `failure_context`, `usage_summary`, `server_updates` y `reconnect::Backoff`. En ot3 queda `pub use t3_protocol::<módulo> as <módulo>` para no tocar los llamadores.

Tests que lo cubren: `tests/checkpoint_operations.rs`, `checkpoint_document.rs`, `plan_workflow.rs`, `plan_transport.rs`, `prompt_recall.rs`, `diff_review.rs`, `tool_inspector.rs`, `quota_sources.rs`, `git_controls.rs`, `provider_instances.rs`, `scoped_settings.rs`, `native_settings.rs`, `storage_settings.rs`, `server_updates.rs`, `bounded_snapshot.rs`, más los unitarios en `src/` que se mueven con el código (`latex`, `branch_picker`, `pull_request_actions`, `thread_details`, `failure_context`, `usage_summary`, `reconnect`). QML: `checkpoint-revert`, `plan-workflow`, `tool-inspector`, `diff-review`, `git-controls`, `provider-instances`, `thread-details`, `failure-context`, `branch-fork`, `branch-mismatch`, `pr-actions`, `pull-requests`, `usage-limits`, `usage-page`, `markdown-links`.

Riesgo: salida de `latex` y de `markdown_render` es específica de Qt (§4.A, §4.C). **No** mover la parte de rich text hasta decidir el renderizador de Android. Mover `latex` solo con `Surface::Qt` fijo.

Condición de salida: cada módulo movido no tiene `use crate::` ni `Environment`, y `cargo test` de la lib corre sus tests sin ot3.

### Lote 4 — Dominio con RPC sobre `&Connection` y constructores de tags (riesgo medio-alto)

Qué entra: la orquestación de §4.B (columna «parte host») con `&Connection`: `git_controls::execute`, `provider_instances::execute`, `scoped_settings::execute_in`, `branch_picker::{refs,switch,check,resolve}`, `pull_request_actions::{run,thread_link,open_thread}`, `thread_details::{read,open}`, `failure_context::{run,tool}`, `working_review`, `diff_review::execute`, `tool_inspector::execute`, `quota_sources::subscribe`, `integration_devices::*`, `keybinding_settings::*`, `suggestions`, `reconnect::reachable`. Los 98 tags de petición se agrupan en módulos tipados de la lib (`vcs`, `server`, `review`, `preview`, `scheduled_tasks`, `provider`, `device`, `agent_sessions`, `mcp_apps`, …), de modo que Kotlin llame `vcs::refresh_status(&conn, cwd)` y no `rpc("vcs.refreshStatus", …)`. Se empieza por los de mayor uso (`server.getConfig` 25 sitios, `server.getSettings` 13, `orchestration.dispatchCommand` 11, `server.updateSettings` 6, `preview.list` 4) y se deja `rpc(tag, payload)` como puerta de escape.

Tests que lo cubren: `tests/writes.rs`, `tests/thread_launch.rs`, `tests/thread_actions.rs`, `tests/runtime_models.rs`, `tests/queue.rs`, `tests/questions.rs`, `tests/approvals.rs`, `tests/suggestions.rs`, `tests/project_browser.rs`, `tests/project_creation.rs`, `tests/projects.rs`, `tests/documents.rs`, `tests/working_review.rs`, `tests/review_document.rs`, `tests/git_operations.rs`, `tests/integration_devices.rs`, `tests/keybinding_settings.rs`, `tests/archive_management.rs`, `tests/preview_transport.rs` (todos usan servidores falsos con `tungstenite::accept` y fijan `req["tag"]` y el `payload`, así que **detectan cualquier cambio en un tag**). QML: `git-flow`, `working-review-flow`, `provider-setup`, `scheduled-tasks`, `integrations`, `keybindings-*`, `pr-actions`, `mcp-app`.

Riesgo: los tests de servidor falso son literales sobre los tags, de modo que un renombrado accidental falla de inmediato (bueno). El peligro es mover la lógica de **qué token usar** (lectura o escritura) a la lib sin querer: la lib recibe la `Connection` ya construida.

### Lote 5 — Estado, streams y contratos con la UI (riesgo alto)

Qué entra, cada punto aparte y reversible:
1. `lib.rs:144-825` (§4.C): `normalize_messages`, timeline, `normalize` de sidebar y markdown, con un parámetro de superficie para los enlaces `ot3-*://`.
2. `shell_state` como `ShellStore` y los bucles de `live.rs` (`:12-76`) y `shell_watch.rs` como `ThreadLive` y `ShellLive` de la lib (reconexión, backoff, `held` envelope, `probe`), alineados con `ffi_stream.rs`.
3. `checkpoint_operations` con `Journal`, `Rpc` y `Assets` inyectados.
4. `device_screen`: frames y entradas sin `tungstenite` ni `reqwest::Response` en la API pública (§1.C).
5. Quitar `reqwest` y `tungstenite` de `[dependencies]` de ot3 (quedan `tungstenite` en `dev-dependencies` para los fixtures y `reqwest` solo si `images.rs`, `desktop_cloud.rs` y `server_updates.rs` lo siguen necesitando).

Tests que lo cubren: `tests/live.rs`, `tests/stream_probe.rs`, `tests/history.rs`, `tests/backend_watch.rs`, `tests/bounded_snapshot.rs`, `tests/checkpoint_operations.rs` (el de recuperación atómica), `tests/archive_management.rs`, `tests/native_terminal.rs`, y la suite QML casi completa, porque `normalize_messages` define el JSON que consumen `timeline`, `composer`, `queue`, `approvals`, `questions`, `tasks`, `work-details`, `context-divider`, `copy-actions`, `code-actions`, `markdown-*`, `history-find`, `sidebar*`, `thread-*`. Hay que correr las 154 y compararlas con la línea base.

Riesgo: es el único lote donde un cambio sutil de formato no rompe `cargo test` pero sí la interfaz. Conviene congelar antes un fixture de `normalize_messages` con un hilo real anonimizado y compararlo byte a byte.

---

## Apéndices (generados con grep sobre `src/`, `HEAD 191c1fc`)

### Apéndice A — Sitios RPC (155 líneas con `rpc(` / `rpc_with_timeout(`; 136 con tag literal, 17 dinámicos y 2 definiciones del helper)

"Dinámico" indica que el tag llega de un llamador; el texto siguiente es el origen, anotado a mano. Las llamadas de varias líneas se resolvieron mirando las 3 líneas siguientes.

| # | archivo:línea | helper | tag |
|---|---|---|---|
| 1 | `src/quotas.rs:204` | Connection::rpc | `server.refreshProviders` |
| 2 | `src/desktop_cloud.rs:309` | Connection::rpc | `server.getConfig` |
| 3 | `src/suggestions.rs:22` | write::rpc | `projects.searchEntries` |
| 4 | `src/suggestions.rs:26` | write::rpc | `server.refreshProviders` |
| 5 | `src/terminal.rs:61` | Connection::rpc | `server.getConfig` |
| 6 | `src/notification_icon.rs:121` | Connection::rpc | `assets.createUrl` |
| 7 | `src/connection_access.rs:62` | write::rpc | `server.getConfig` |
| 8 | `src/connection_access.rs:113` | write::rpc | `server.updateSettings` |
| 9 | `src/connection_route.rs:38` | write::rpc | `server.getConfig` |
| 10 | `src/connection_route.rs:40` | write::rpc | `server.getHostResources` |
| 11 | `src/connection_github.rs:15` | write::rpc | (dinámico) pullRequests.detail|checks|activity|routing|routingIdentity (tag del llamador) |
| 12 | `src/connection_github.rs:18` | write::rpc | (dinámico) pullRequests.detail|checks|activity|routing|routingIdentity (tag del llamador) |
| 13 | `src/connection_github.rs:20` | write::rpc | `pullRequests.routing` |
| 14 | `src/connection_github.rs:21` | write::rpc | (dinámico) pullRequests.detail|checks|activity|routing|routingIdentity (tag del llamador) |
| 15 | `src/connection_github.rs:26` | write::rpc | `pullRequests.routingIdentity` |
| 16 | `src/connection_github.rs:29` | write::rpc | (dinámico) pullRequests.detail|checks|activity|routing|routingIdentity (tag del llamador) |
| 17 | `src/connection_github.rs:34` | write::rpc | (dinámico) pullRequests.detail|checks|activity|routing|routingIdentity (tag del llamador) |
| 18 | `src/tool_inspector.rs:111` | Connection::rpc | (dinámico) orchestration.getTurnItem (const METHOD) |
| 19 | `src/tool_inspector.rs:118` | Connection::rpc | (dinámico) orchestration.getTurnItem (const METHOD) |
| 20 | `src/diff_review.rs:203` | Connection::rpc | (dinámico) tag del llamador (review.getDiffPreview / orchestration.get*Diff) |
| 21 | `src/git_controls.rs:247` | Connection::rpc | `server.getSettings` |
| 22 | `src/git_controls.rs:249` | Connection::rpc | `vcs.refreshStatus` |
| 23 | `src/git_operations.rs:255` | Connection::rpc | (dinámico) tag del paso (UI): git.runStackedAction, vcs.pull, terminal.open, terminal.write, … |
| 24 | `src/checkpoint_operations.rs:35` | write::rpc | `orchestration.getThreadProjection` |
| 25 | `src/checkpoint_operations.rs:47` | write::rpc | `server.getConfig` |
| 26 | `src/checkpoint_operations.rs:182` | Connection::rpc | `assets.createUrl` |
| 27 | `src/checkpoint_operations.rs:271` | write::rpc | `orchestration.dispatchCommand` |
| 28 | `src/checkpoint_operations.rs:302` | write::rpc | `orchestration.dispatchCommand` |
| 29 | `src/working_review.rs:220` | Connection::rpc | `vcs.listRefs` |
| 30 | `src/working_review.rs:237` | Connection::rpc | `review.getDiffPreview` |
| 31 | `src/working_review.rs:253` | Connection::rpc | `review.getDiffPreview` |
| 32 | `src/review_document.rs:78` | Connection::rpc | (dinámico) snapshot["tag"] (review.getDiffPreview / getDiffFileContents) |
| 33 | `src/review_document.rs:182` | Connection::rpc | `review.getDiffFileContents` |
| 34 | `src/review_document.rs:187` | Connection::rpc | `review.getDiffPreview` |
| 35 | `src/archive_management.rs:163` | Connection::rpc | `orchestration.getArchivedShellSnapshot` |
| 36 | `src/archive_management.rs:342` | Connection::rpc | `orchestration.dispatchCommand` |
| 37 | `src/keybinding_settings.rs:180` | Connection::rpc | `server.getConfig` |
| 38 | `src/keybinding_settings.rs:232` | Connection::rpc | `server.getConfig` |
| 39 | `src/keybinding_settings.rs:250` | Connection::rpc | (dinámico) server.upsertKeybinding | server.removeKeybinding |
| 40 | `src/keybinding_settings.rs:252` | Connection::rpc | `server.getConfig` |
| 41 | `src/keybinding_settings.rs:280` | Connection::rpc | `server.getConfig` |
| 42 | `src/keybinding_settings.rs:291` | Connection::rpc | `shell.openInEditor` |
| 43 | `src/preview_transport.rs:195` | Connection::rpc | `server.getConfig` |
| 44 | `src/preview_transport.rs:209` | Connection::rpc | `preview.list` |
| 45 | `src/preview_transport.rs:256` | Connection::rpc | `preview.list` |
| 46 | `src/preview_transport.rs:290` | Connection::rpc | `preview.reportStatus` |
| 47 | `src/preview_transport.rs:316` | Connection::rpc | `preview.list` |
| 48 | `src/preview_actions.rs:135` | Connection::rpc | `preview.list` |
| 49 | `src/preview_actions.rs:164` | Connection::rpc | (dinámico) preview.open|navigate|resize|adjust|refresh|close|clearProfile |
| 50 | `src/provider_instances.rs:213` | write::rpc | `server.getSettings` |
| 51 | `src/provider_instances.rs:215` | write::rpc | `server.updateSettings` |
| 52 | `src/provider_instances.rs:217` | write::rpc | `server.getSettings` |
| 53 | `src/provider_instances.rs:223` | write::rpc | `server.getSettings` |
| 54 | `src/provider_instances.rs:224` | write::rpc | `server.getConfig` |
| 55 | `src/branch_picker.rs:78` | rpc (re-export write::rpc) | `vcs.listRefs` |
| 56 | `src/branch_picker.rs:99` | rpc (re-export write::rpc) | `vcs.createRef` |
| 57 | `src/branch_picker.rs:108` | rpc (re-export write::rpc) | `vcs.switchRef` |
| 58 | `src/branch_picker.rs:118` | rpc (re-export write::rpc) | `orchestration.dispatchCommand` |
| 59 | `src/branch_picker.rs:181` | rpc (re-export write::rpc) | `vcs.refreshStatus` |
| 60 | `src/branch_picker.rs:194` | rpc (re-export write::rpc) | `vcs.refreshStatus` |
| 61 | `src/branch_picker.rs:202` | rpc (re-export write::rpc) | `vcs.switchRef` |
| 62 | `src/branch_picker.rs:207` | rpc (re-export write::rpc) | `orchestration.dispatchCommand` |
| 63 | `src/thread_search.rs:16` | rpc (re-export write::rpc) | `orchestration.searchThreads` |
| 64 | `src/pull_request_actions.rs:81` | rpc (re-export write::rpc) | (dinámico) pullRequests.* / orchestration.dispatchCommand / git.preparePullRequestThread |
| 65 | `src/pull_request_actions.rs:110` | rpc (re-export write::rpc) | `orchestration.dispatchCommand` |
| 66 | `src/pull_request_actions.rs:176` | rpc (re-export write::rpc) | `git.preparePullRequestThread` |
| 67 | `src/pull_request_actions.rs:178` | rpc (re-export write::rpc) | `orchestration.dispatchCommand` |
| 68 | `src/usage_summary.rs:97` | rpc (re-export write::rpc) | `server.refreshUsageRates` |
| 69 | `src/usage_summary.rs:100` | rpc (re-export write::rpc) | `server.getUsageSummary` |
| 70 | `src/scheduled_tasks.rs:58` | rpc (re-export write::rpc) | `scheduledTasks.list` |
| 71 | `src/scheduled_tasks.rs:59` | rpc (re-export write::rpc) | `scheduledTasks.upsert` |
| 72 | `src/scheduled_tasks.rs:60` | rpc (re-export write::rpc) | `scheduledTasks.setEnabled` |
| 73 | `src/scheduled_tasks.rs:61` | rpc (re-export write::rpc) | `scheduledTasks.delete` |
| 74 | `src/scheduled_tasks.rs:62` | rpc (re-export write::rpc) | `scheduledTasks.runNow` |
| 75 | `src/scheduled_tasks.rs:66` | rpc (re-export write::rpc) | `scheduledTasks.list` |
| 76 | `src/storage_cleanup.rs:35` | rpc (re-export write::rpc) | `server.runStorageCleanup` |
| 77 | `src/mcp_apps.rs:42` | rpc (re-export write::rpc) | (dinámico) mcpApps.callTool|readResource|updateModelContext |
| 78 | `src/agent_sessions.rs:9` | rpc (re-export write::rpc) | `agentSessions.scan` |
| 79 | `src/agent_sessions.rs:20` | rpc (re-export write::rpc) | `projects.mutate` |
| 80 | `src/agent_sessions.rs:24` | rpc (re-export write::rpc) | `agentSessions.import` |
| 81 | `src/process_diagnostics.rs:15` | rpc (re-export write::rpc) | `server.signalProcess` |
| 82 | `src/process_diagnostics.rs:17` | rpc (re-export write::rpc) | `server.getProcessDiagnostics` |
| 83 | `src/process_diagnostics.rs:27` | rpc (re-export write::rpc) | `server.getResourceTelemetryHistory` |
| 84 | `src/acp_registry.rs:10` | rpc (re-export write::rpc) | `server.searchAcpRegistry` |
| 85 | `src/acp_registry.rs:16` | rpc (re-export write::rpc) | `server.prepareAcpRegistryAgent` |
| 86 | `src/documents.rs:18` | Connection::rpc | `projects.readFile` |
| 87 | `src/thread_details.rs:35` | write::rpc | `server.getConfig` |
| 88 | `src/thread_details.rs:45` | write::rpc | `server.getConfig` |
| 89 | `src/thread_details.rs:53` | write::rpc | `shell.openInEditor` |
| 90 | `src/provider_updates.rs:50` | Connection::rpc_with_timeout | `server.updateProvider` |
| 91 | `src/provider_updates.rs:53` | Connection::rpc_with_timeout | `server.updateProvider` |
| 92 | `src/provider_updates.rs:57` | write::rpc | `server.getConfig` |
| 93 | `src/provider_updates.rs:74` | write::rpc | `server.getConfig` |
| 94 | `src/failure_context.rs:103` | write::rpc | `orchestration.getTurnItem` |
| 95 | `src/failure_context.rs:129` | write::rpc | (dinámico) orchestration.getTurnItem (tool_inspector::request) |
| 96 | `src/browser_profiles.rs:48` | Connection::rpc | `server.getConfig` |
| 97 | `src/browser_profiles.rs:220` | Connection::rpc_with_timeout | `preview.clearProfile` |
| 98 | `src/device_screen.rs:87` | Connection::rpc_with_timeout | `device.detail` |
| 99 | `src/device_screen.rs:92` | Connection::rpc_with_timeout | `device.action` |
| 100 | `src/device_screen.rs:93` | Connection::rpc_with_timeout | `device.detail` |
| 101 | `src/device_screen.rs:112` | Connection::rpc_with_timeout | `device.close` |
| 102 | `src/device_screen.rs:280` | Connection::rpc_with_timeout | `device.open` |
| 103 | `src/integration_devices.rs:281` | Connection::rpc | `server.getSettings` |
| 104 | `src/integration_devices.rs:346` | Connection::rpc_with_timeout | (dinámico) device.list|configure|testHost / server.updateSettings |
| 105 | `src/integration_devices.rs:383` | Connection::rpc | `server.getSettings` |
| 106 | `src/provider_setup.rs:44` | rpc (re-export write::rpc) | `provider.install.start` |
| 107 | `src/provider_setup.rs:45` | rpc (re-export write::rpc) | `provider.install.remove` |
| 108 | `src/provider_setup.rs:46` | rpc (re-export write::rpc) | `provider.install.cancel` |
| 109 | `src/provider_setup.rs:53` | rpc (re-export write::rpc) | `provider.auth.start` |
| 110 | `src/provider_setup.rs:55` | rpc (re-export write::rpc) | `provider.auth.logout` |
| 111 | `src/provider_setup.rs:56` | rpc (re-export write::rpc) | `provider.auth.cancel` |
| 112 | `src/provider_setup.rs:57` | rpc (re-export write::rpc) | `provider.auth.respond` |
| 113 | `src/write.rs:8` | definición del helper | (cualquiera; ver sitios de write::rpc) |
| 114 | `src/write.rs:9` | definición del helper | (cualquiera; ver sitios de write::rpc) |
| 115 | `src/write.rs:19` | rpc (write::rpc local) | `server.getConfig` |
| 116 | `src/write.rs:35` | rpc (write::rpc local) | `server.getConfig` |
| 117 | `src/write.rs:36` | rpc (write::rpc local) | `server.discoverSourceControl` |
| 118 | `src/write.rs:44` | rpc (write::rpc local) | `server.updateSettings` |
| 119 | `src/write.rs:45` | rpc (write::rpc local) | `server.getSettings` |
| 120 | `src/write.rs:172` | rpc (write::rpc local) | `orchestration.dispatchCommand` |
| 121 | `src/write.rs:173` | rpc (write::rpc local) | `orchestration.dispatchCommand` |
| 122 | `src/write.rs:174` | rpc (write::rpc local) | `orchestration.dispatchCommand` |
| 123 | `src/write.rs:208` | rpc (write::rpc local) | `filesystem.getMetadata` |
| 124 | `src/write.rs:258` | rpc (write::rpc local) | (dinámico) tag del comando (projects.createNew, projectClone.start, orchestration.*, vcs.*, …) |
| 125 | `src/write.rs:309` | rpc (write::rpc local) | `filesystem.browse` |
| 126 | `src/write.rs:312` | rpc (write::rpc local) | `server.getSettings` |
| 127 | `src/write.rs:344` | rpc (write::rpc local) | `worktreeSetup.cancel` |
| 128 | `src/write.rs:345` | rpc (write::rpc local) | `server.checkOtlpEndpoint` |
| 129 | `src/write.rs:346` | rpc (write::rpc local) | `orchestration.getWorkflowScript` |
| 130 | `src/write.rs:347` | rpc (write::rpc local) | `orchestration.stopWorkflow` |
| 131 | `src/write.rs:348` | rpc (write::rpc local) | `vcs.init` |
| 132 | `src/write.rs:363` | rpc (write::rpc local) | `orchestration.dispatchCommand` |
| 133 | `src/write.rs:365` | rpc (write::rpc local) | `attachments.delete` |
| 134 | `src/write.rs:377` | rpc (write::rpc local) | `server.getConfig` |
| 135 | `src/write.rs:403` | rpc (write::rpc local) | `secrets.answerRequest` |
| 136 | `src/write.rs:414` | rpc (write::rpc local) | `orchestration.dispatchCommand` |
| 137 | `src/server_capabilities.rs:27` | Connection::rpc | `server.getConfig` |
| 138 | `src/journal_vault.rs:191` | write::rpc | `projects.readFile` |
| 139 | `src/journal_vault.rs:200` | write::rpc | `filesystem.getMetadata` |
| 140 | `src/auto_compact.rs:56` | write::rpc | `server.getConfig` |
| 141 | `src/auto_compact.rs:81` | write::rpc | `server.getSettings` |
| 142 | `src/auto_compact.rs:273` | write::rpc | `orchestration.dispatchCommand` |
| 143 | `src/auto_compact.rs:290` | write::rpc | `orchestration.dispatchCommand` |
| 144 | `src/native_settings.rs:221` | write::rpc | `server.getConfig` |
| 145 | `src/native_settings.rs:223` | write::rpc | `server.getSettings` |
| 146 | `src/native_settings.rs:224` | write::rpc | `server.updateSettings` |
| 147 | `src/native_settings.rs:228` | write::rpc | `server.getSettings` |
| 148 | `src/scoped_settings.rs:142` | write::rpc | `server.getConfig` |
| 149 | `src/scoped_settings.rs:144` | write::rpc | `server.getSettings` |
| 150 | `src/scoped_settings.rs:157` | write::rpc | `server.updateSettings` |
| 151 | `src/scoped_settings.rs:158` | write::rpc | `server.getSettings` |
| 152 | `src/scoped_settings.rs:162` | write::rpc | `projects.readFile` |
| 153 | `src/lib.rs:74` | write::rpc | `assets.createUrl` |
| 154 | `src/main.rs:157` | write::rpc | `server.reportClientActivity` |
| 155 | `src/server_update.rs:91` | write::rpc | `server.getConfig` |

### Apéndice B — HTTP a T3 que ya pasa por la lib (`Client::get`, `bounded_thread`, `request_json`)

42 filas: 21 `Client::get`, 6 `bounded_thread`, 15 `request_json`. Cinco son definiciones o envoltorios (`lib.rs:474,477,486,489`, `connection_access.rs:44`). La ruta «(ruta calculada)» se construye con `protocol::history::path`, `Client::bounded_thread` o un `path` recibido.

| archivo:línea | helper | método | ruta |
|---|---|---|---|
| `src/desktop_cloud.rs:308` | request_json | GET | `/api/orchestration/shell` |
| `src/suggestions.rs:12` | Client::get | GET | `/api/orchestration/shell` |
| `src/terminal.rs:60` | Client::get | GET | `/api/orchestration/shell` |
| `src/connection_access.rs:44` | request_json | GET | `(ruta calculada)` |
| `src/connection_route.rs:25` | Client::get | GET | `/api/projects` |
| `src/git_controls.rs:241` | request_json | GET | `/api/orchestration/shell` |
| `src/git_operations.rs:33` | Client::get | GET | `/api/auth/session` |
| `src/checkpoint_operations.rs:19` | Client::get | GET | `/api/auth/session` |
| `src/working_review.rs:21` | request_json | GET | `/api/orchestration/shell` |
| `src/working_review.rs:127` | Client::get | GET | `/api/auth/session` |
| `src/archive_management.rs:28` | request_json | GET | `/api/auth/session` |
| `src/archive_management.rs:174` | request_json | GET | `/api/orchestration/shell` |
| `src/keybinding_settings.rs:28` | request_json | GET | `/api/auth/session` |
| `src/preview_transport.rs:183` | request_json | GET | `/api/auth/session` |
| `src/quota_sources.rs:150` | request_json | GET | `/api/auth/session` |
| `src/provider_instances.rs:206` | Client::get | GET | `/api/auth/session` |
| `src/branch_picker.rs:8` | Client::get | GET | `/api/orchestration/shell` |
| `src/thread_details.rs:27` | Client::get | GET | `/api/orchestration/shell` |
| `src/failure_context.rs:117` | bounded_thread | GET | `(ruta calculada)` |
| `src/browser_profiles.rs:35` | request_json | GET | `/api/auth/session` |
| `src/connections_management.rs:243` | request_json | GET | `/api/auth/session` |
| `src/integration_devices.rs:32` | request_json | GET | `/api/auth/session` |
| `src/write.rs:29` | Client::get | GET | `/api/auth/session` |
| `src/write.rs:55` | Client::get | GET | `(ruta calculada)` |
| `src/write.rs:135` | bounded_thread | GET | `(ruta calculada)` |
| `src/write.rs:301` | request_json | POST | `/api/pull-requests/diff` |
| `src/write.rs:374` | Client::get | GET | `/api/auth/session` |
| `src/write.rs:384` | bounded_thread | GET | `(ruta calculada)` |
| `src/disk_space.rs:109` | Client::get | GET | `/api/orchestration/shell` |
| `src/live.rs:24` | bounded_thread | GET | `(ruta calculada)` |
| `src/reconnect.rs:40` | Client::get | GET | `/.well-known/t3/environment` |
| `src/journal_vault.rs:1548` | Client::get | GET | `/api/projects` |
| `src/journal_vault.rs:1701` | Client::get | GET | `/api/projects` |
| `src/auto_compact.rs:45` | bounded_thread | GET | `(ruta calculada)` |
| `src/scoped_settings.rs:135` | Client::get | GET | `/api/auth/session` |
| `src/scoped_settings.rs:146` | Client::get | GET | `/api/orchestration/shell` |
| `src/lib.rs:474` | request_json | GET | `(ruta calculada)` |
| `src/lib.rs:477` | request_json | GET | `(ruta calculada)` |
| `src/lib.rs:486` | Client::get | GET | `(ruta calculada)` |
| `src/lib.rs:489` | bounded_thread | GET | `(ruta calculada)` |
| `src/lib.rs:834` | Client::get | GET | `/api/orchestration/shell` |
| `src/main.rs:176` | Client::get | GET | `/.well-known/t3/environment` |

### Apéndice C — Constructores de `Connection` (27)

| archivo:línea | constructor | token |
|---|---|---|
| `src/images.rs:15` | Connection::new | escritura |
| `src/clipboard.rs:38` | Connection::new | escritura |
| `src/terminal.rs:18` | Connection::new | escritura |
| `src/notification_icon.rs:113` | Connection::new | lectura |
| `src/connection_access.rs:40` | Connection::new | credencial local (`t3 auth session issue`) |
| `src/connection_access.rs:42` | Connection::new | escritura |
| `src/tool_inspector.rs:113` | Connection::new | lectura |
| `src/diff_review.rs:201` | Connection::new | lectura |
| `src/git_controls.rs:235` | Connection::new | lectura |
| `src/git_operations.rs:26` | Connection::new | escritura |
| `src/checkpoint_operations.rs:12` | Connection::new | escritura |
| `src/working_review.rs:13` | Connection::new | lectura |
| `src/archive_management.rs:21` | Connection::new | variable |
| `src/keybinding_settings.rs:21` | Connection::new | variable |
| `src/preview_transport.rs:175` | Connection::new | escritura |
| `src/quota_sources.rs:143` | Connection::new | lectura |
| `src/documents.rs:12` | Connection::new | escritura |
| `src/provider_updates.rs:52` | Connection::new | escritura |
| `src/browser_profiles.rs:27` | Connection::new | escritura |
| `src/connections_management.rs:242` | Connection::new | token recién canjeado (`/oauth/token`) |
| `src/integration_devices.rs:25` | Connection::new | escritura |
| `src/write.rs:9` | Connection::new | escritura |
| `src/write.rs:73` | Connection::new | escritura |
| `src/write.rs:301` | Connection::new | lectura |
| `src/live.rs:30` | Connection::new | escritura |
| `src/shell_watch.rs:70` | Connection::new | escritura |
| `src/lib.rs:476` | with_http_client | lectura |

### Apéndice D — `Client::new()` (23 sitios en 18 archivos)

`src/quotas.rs:272`, `src/suggestions.rs:12`, `src/terminal.rs:60`, `src/connection_route.rs:21`, `src/git_operations.rs:33`, `src/checkpoint_operations.rs:19`, `src/working_review.rs:127`, `src/provider_instances.rs:205`, `src/branch_picker.rs:8`, `src/thread_details.rs:27`, `src/failure_context.rs:117`, `src/connections_management.rs:173`, `src/write.rs:29`, `src/write.rs:55`, `src/write.rs:135`, `src/write.rs:374`, `src/write.rs:384`, `src/reconnect.rs:40`, `src/journal_vault.rs:1546`, `src/journal_vault.rs:1687`, `src/auto_compact.rs:121`, `src/scoped_settings.rs:130`, `src/main.rs:604`

---

## 6. Resumen (10 líneas)

1. **Sitios de contacto con T3 en `src/`: 215** = 153 RPC + 13 suscripciones + 37 HTTP vía lib (42 filas menos 5 de definición) + 8 peticiones HTTP crudas + 4 de asset o upload vía lib.
2. **Sitios que NO pasan por `t3_protocol`: 6 funciones / 8 peticiones HTTP, en 6 archivos** (`lib.rs`, `notification_icon.rs`, `checkpoint_operations.rs`, `connections.rs`, `connections_management.rs`, `omabridge_route.rs`). WebSocket propio: 0. SSE: 0.
3. **Archivos con contacto T3: 63 de 105** en `src/`. Los 27 que importan `reqwest::` son 18 solo por `Url`, 9 con cliente HTTP y solo 6 de ellos hacia T3.
4. **Rutas HTTP únicas: 17** en ot3 (más 6 solo dentro de la lib). **Tags RPC únicos: 110** (98 de petición y 12 de stream).
5. **`Client::get` ya usa `Connection::request_json`.** `Client::fetch*` solo sirve a `local_quotas.rs` (4 llamadas a terceros) y debe quedarse fuera de la lib.
6. **Constructores duplicados:** 27 `Connection::new` / `with_http_client` (11 funciones `connection(e)` y 16 copias en línea) y 23 `Client::new()` en 18 archivos. Sin reutilización de pool: cada `rpc` abre un WebSocket nuevo.
7. **Diferencias a absorber en la lib:** timeouts (8 s / 3 s frente a 12 s sin conexión), `Content-Type` sensible a mayúsculas, límite por llamada, respuestas 2xx sin JSON, `no_proxy` en A4/A6 y errores `*_attachment_*`.
8. **Portables:** 1 531 líneas puras completas (7 módulos) y ≈ 2 000 líneas puras dentro de 18 módulos mixtos, más ≈ 450 de derivación en `lib.rs:144-825`. No portables: `checkpoint_journal`, `quotas`, `auto_compact`, `device_screen`, `browser_*`, `omabridge_*`, `journal_vault`.
9. **5 lotes:** (1) higiene y sincronizar el vendor, (2) transporte, (3) lógica pura, (4) dominio con RPC y tags tipados, (5) estado, streams y contratos de UI. Tests de apoyo: 93 unitarios, 318 de integración, 154 QML.
10. **Bloqueante previo:** `vendor/t3-protocol` difiere de `lib-t3-rust` en 12 archivos de `src/` (10 distintos y 2 que solo existen en el vendor), y el repo de la lib tiene además cambios sin commit. Hay que sincronizar y retaguear antes de dar nada por reutilizable en Android.
