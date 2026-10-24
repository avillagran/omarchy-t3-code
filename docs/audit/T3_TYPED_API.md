# Superficie tipada de `t3-protocol`: 110 tags → funciones Rust y ABI JSON

- Tipo: diseño. No se modificó código; esta es la única escritura.
- Fecha: 2026-10-24.
- Bases leídas: ot3 `191c1fc` (rama `refactor/lib-first-lote1`); lib `~/Work/lib-t3-rust` `a2f9ccf` (Cargo 0.3.0, `v0.2.0-21`, árbol limpio); `vendor/t3-protocol` (Cargo 0.2.0). El vendor ya coincide con la lib en todo `src/` salvo `ffi.rs` y `ffi_stream.rs` (le faltan el op `rpcWithTimeout` y el campo `message`/`errorMessage`). Los números de línea de la lib son los de `lib-t3-rust`.
- Las rutas `archivo.rs:línea` de ot3 son las de `HEAD` `191c1fc` (las comprobé con `git show HEAD:src/…`). Al escribir esto el árbol de trabajo ya tenía 32 archivos de `src/` modificados sin commit (el Lote 1, otra tarea; yo no los toqué), que pueden mover líneas.
- Contrato (fuente de verdad): `~/Work/t3code-nightly` en `680152e27` (2026-10-21). Rutas `packages/contracts/src/…` y `apps/server/src/…`.
- Método: extraje con un script los 110 tags de ot3 (los literales de `src/` que existen en `WS_METHODS`, `rpc.ts:380`, y en `ORCHESTRATION_V2_WS_METHODS`, `orchestrationV2.ts:3080`) y para cada uno el bloque `Rpc.make(…)`, el scope de `RPC_REQUIRED_SCOPES` y el handler de `ws.ts`. Los campos de payload y respuesta los leí a mano de los esquemas. Los sitios de ot3 salen del Apéndice A de `T3_TOUCHPOINTS.md`, más los que ese apéndice marca «dinámico», que resolví leyendo el código.
- No ejecuté `cargo`, ni los tests de ot3 o de la lib, ni contra un servidor T3 real.

**Notación.** `conn` es `&Connection`. `Id` = id que T3 emite (hilo, proyecto, comando, run, …). Un campo con `?` es opcional. «vacío» = la respuesta es `Schema.Void`, que la lib devuelve como `Value::Null`.

| Kind | Significa | Qué hace el host tras `rpc_outcome_unknown` |
|---|---|---|
| `L` | lectura | repetir |
| `M·id` | mutación con `commandId`; el servidor deduplica por recibo (`EventSink.ts:602-625`) | reenviar **el mismo** payload (mismo `commandId`) |
| `M·id†` | lleva `commandId` pero no verifiqué que el servidor lo deduplique | tratar como `M!` |
| `M·set` | mutación idempotente por valor (fija un estado) | repetir es inocuo; la lib nunca lo hace sola |
| `M!` | mutación no repetible | leer el estado (columna «reconciliar» en §4.3) antes de decidir |

La lib no reintenta ninguna llamada, de ningún kind (`README.md`, «Mutation requests are never automatically retried»).

---

## 0. Hallazgos que cambian el encargo

1. **Los 110 tags existen en el contrato; ninguno es obsoleto.** Todos resuelven en `WS_METHODS` o `ORCHESTRATION_V2_WS_METHODS` y tienen su `Rpc.make` y su handler en `ws.ts` (tablas de referencia por módulo, §1). Son **97 unarios y 13 streams**, no 98 y 12: `git.runStackedAction` es `stream: true` en el contrato (`rpc.ts:1379-1385`). ot3 ya lo trata como stream (`git_operations.rs:212`, `subscribe`), pero la auditoría lo contó como petición porque el tag llega dentro de un plan (`git_controls.rs:364`). `Connection::rpc` no sirve para él: ignora los `Chunk` sin responder `Ack`, así que no entrega el progreso y el servidor puede quedar esperando hasta el plazo de 20 s (`rpc_outcome_unknown`). No lo probé contra un servidor.
2. **El grep del que salieron los 120 literales traía 10 que no son RPC**: `preview.toggle`, `preview.focusUrl`, `preview.zoomIn`, `preview.zoomOut`, `preview.resetZoom`, `terminal.toggle`, `terminal.split`, `terminal.splitVertical`, `terminal.new`, `terminal.close`. Son ids de atajo de teclado (`src/keybinding_commands.json:5-9,18-23`), sin ninguna llamada `rpc(`. (`terminal.close` sí existe como RPC, `rpc.ts:1488`, pero ot3 no lo llama.) Al excluirlos quedan los 110.
3. **Conteos de uso.** `server.getConfig` 25 y `server.getSettings` 13 coinciden con el audit. `orchestration.dispatchCommand`: el audit dice 11; hay 14 filas en el Apéndice A y 6 literales más dentro del despachador dinámico `write.rs:203-243` (el grep da 20 líneas). `server.updateSettings`: el audit dice 6; hay 7 sitios efectivos (se suma `write.rs:312`, la rama `model-favorite`, que está en la misma línea que un `getSettings`). `preview.list` 4 coincide. Uso estos números donde importan para ordenar las tandas (§5).
4. **T3 autoriza con 17 scopes, ot3 con 2 tokens.** `auth.ts:30-46` define los scopes y `RpcAuthorization.ts:40-211` fija uno por tag. Los 110 usan 10 distintos: `orchestration:read` (40), `orchestration:operate` (19), `source-control:write` (12), `providers:manage` (10), `preview:operate` (8), `settings:write` (7), `filesystem:read` (6), `diagnostics:read` (4), `terminal:operate` (3) y `environment:maintain` (1). Seis son dinámicos (marcados con `*` en las tablas): dependen del payload (`RpcAuthorization.ts:254-268`, `settings.ts:1551,1575`, `clientRpcPermissions.ts:58-77`). Consecuencia de diseño: la lib **publica** el scope de cada función como metadato y el host decide qué token usar. La lib no elige token.
5. **Bug latente en `rejection()` (`lib.rs:538`).** Clasifica un fallo como `write_not_authorized` si el JSON entero de la causa contiene `"Auth"`, `"Forbidden"` o `"scope"`, incluido el texto libre del mensaje. Un `git commit` que falla con «Author identity unknown» o un `vcs.pull` con «Authentication failed for …» saldría como falta de permiso, y ot3 lo muestra como `permission_denied` (`integration_devices.rs:350`, `browser_profiles.rs:226`). El contrato da una señal exacta: `EnvironmentAuthorizationError` (`auth.ts:332`) con `requiredScope`. Hay que arreglarlo en la tanda 1, porque cambia el contrato de errores que Kotlin va a ver.
6. **Límites del FFI de streams** (detalle en §2.2): la cola rechaza cualquier ítem de más de 8 MiB, pero `subscribeThread` con `acceptBoundedSnapshot` admite tramas de 64 MiB (`stream.rs:16` frente a `ffi_stream.rs:34-35`); los marcadores de fin de chunk (`receive(None)`) se pierden; no hay forma de sondear sockets tras una suspensión (`subscribe_probed` no tiene op); `maxSubscriptions` es 4 y cada una abre su propio WebSocket.
7. **El adaptador Kotlin descarta la razón del error.** `T3Client.kt:21-24` (`decode`) lee solo `error`, y `T3Subscription.poll` (`T3Client.kt:78-79`) lee solo `error`, aunque `lib-t3-rust/src/ffi.rs:174-184` ya emite `message` y `ffi_stream.rs:96-99` `errorMessage`. Un `rpc_rejected` llega a Kotlin sin el motivo.
8. **`rpc` tiene un plazo fijo de 20 s** (`lib.rs:451-484`); `rpc_with_timeout` admite hasta 300 s. ot3 ya usa plazos de 20, 30, 60, 180 y 300 s en 9 sitios. El registro debe llevar el plazo por función (§3).
9. **Codificaciones de Effect que los tipos de respuesta deben tolerar**: `Option` llega como `{"_tag":"Some","value":…}` o `{"_tag":"None"}` (`process_diagnostics.rs:25` lee `error.value.message`); `ForwardCompatibleArray`/`ForwardCompatibleUnion` aceptan miembros que un cliente viejo no conoce (por eso existe `unknown-event` en `live.rs:47`); `Void` es `null`.
10. **La lib ya cubre parte de la superficie** y esa parte no debe duplicarse (§1, tabla «Fusiones»): `pull_requests::request`, `projects::{command,folders}`, `queue/questions/approvals::command`, `models::{runtime_mode_command,model_catalog,set_model_favorite}`, `settings::{snapshot,patch}`, `terminal::*` + `Connection::terminal_write/terminal_resize`, `documents`/`image_resource` (usan `assets.createUrl`), `composer::{paths,catalog}`.

---

## 1. Módulos de la lib

Layout propuesto: `t3_protocol::api::<módulo>`, un archivo por módulo bajo `src/api/`. Cada función es `pub fn nombre(conn: &Connection, …) -> Result<Value>` (convenciones en §4) y se registra una sola vez en una tabla estática que usa el FFI (§3). Los módulos existentes (`projects`, `pull_requests`, `terminal`, `settings`, `models`, `documents`…) siguen donde están y se reexportan desde `api`.

| Módulo | Archivo | Unarios | Streams | Tags |
|---|---|---|---|---|
| `server` | `src/api/server.rs` | 9 | 1 | `server.getConfig`, `server.getSettings`, `server.updateSettings`, `server.discoverSourceControl`, `server.reportClientActivity`, `server.upsertKeybinding`, `server.removeKeybinding`, `server.searchAcpRegistry`, `server.prepareAcpRegistryAgent` · streams: `subscribeServerConfig` |
| `diagnostics` | `src/api/diagnostics.rs` | 8 | 1 | `server.getHostResources`, `server.getProcessDiagnostics`, `server.getResourceTelemetryHistory`, `server.signalProcess`, `server.checkOtlpEndpoint`, `server.getUsageSummary`, `server.refreshUsageRates`, `server.runStorageCleanup` · streams: `server.getStorageCleanupReport` |
| `provider` | `src/api/provider.rs` | 9 | 2 | `server.refreshProviders`, `server.updateProvider`, `provider.auth.start`, `provider.auth.respond`, `provider.auth.cancel`, `provider.auth.logout`, `provider.install.start`, `provider.install.cancel`, `provider.install.remove` · streams: `provider.install.subscribe`, `provider.auth.subscribe` |
| `orchestration` | `src/api/orchestration.rs` | 10 | 3 | `orchestration.dispatchCommand`, `orchestration.launchThread`, `orchestration.getThreadProjection`, `orchestration.getTurnItem`, `orchestration.getTurnDiff`, `orchestration.getFullThreadDiff`, `orchestration.getWorkflowScript`, `orchestration.stopWorkflow`, `orchestration.searchThreads`, `orchestration.getArchivedShellSnapshot` · streams: `orchestration.subscribeShell`, `orchestration.subscribeThread`, `orchestration.subscribeCommandOutput` |
| `vcs`, `git` | `src/api/vcs.rs` | 7 | 1 | `vcs.refreshStatus`, `vcs.listRefs`, `vcs.createRef`, `vcs.switchRef`, `vcs.pull`, `vcs.init`, `git.preparePullRequestThread` · streams: `git.runStackedAction` |
| `review` | `src/api/review.rs` | 2 | 0 | `review.getDiffPreview`, `review.getDiffFileContents` |
| `projects`, `project_clone`, `filesystem` | `src/api/projects.rs, src/api/filesystem.rs` | 7 | 0 | `projects.readFile`, `projects.searchEntries`, `projects.mutate`, `projects.createNew`, `projectClone.start`, `filesystem.browse`, `filesystem.getMetadata` |
| `pull_requests` | `src/api/pull_requests.rs (fusiona con src/pull_requests.rs)` | 12 | 0 | `pullRequests.detail`, `pullRequests.checks`, `pullRequests.activity`, `pullRequests.routing`, `pullRequests.routingIdentity`, `pullRequests.reviewerCandidates`, `pullRequests.labelCandidates`, `pullRequests.comment`, `pullRequests.submitReview`, `pullRequests.runAction`, `pullRequests.requestReviewers`, `pullRequests.setLabels` |
| `preview` | `src/api/preview.rs` | 9 | 2 | `preview.list`, `preview.open`, `preview.navigate`, `preview.resize`, `preview.adjust`, `preview.refresh`, `preview.close`, `preview.clearProfile`, `preview.reportStatus` · streams: `subscribePreviewEvents`, `subscribeDiscoveredLocalServers` |
| `device` | `src/api/device.rs` | 7 | 1 | `device.list`, `device.configure`, `device.testHost`, `device.open`, `device.close`, `device.detail`, `device.action` · streams: `subscribeDeviceState` |
| `scheduled_tasks` | `src/api/scheduled_tasks.rs` | 5 | 0 | `scheduledTasks.list`, `scheduledTasks.upsert`, `scheduledTasks.setEnabled`, `scheduledTasks.delete`, `scheduledTasks.runNow` |
| `agent_sessions`, `mcp_apps`, `assets`, `shell`, `secrets`, `worktree_setup`, `terminal` | `src/api/{agent_sessions,mcp_apps,assets,shell,secrets,worktree_setup,terminal}.rs` | 12 | 2 | `agentSessions.scan`, `agentSessions.import`, `mcpApps.callTool`, `mcpApps.readResource`, `mcpApps.updateModelContext`, `assets.createUrl`, `attachments.delete`, `shell.openInEditor`, `secrets.answerRequest`, `worktreeSetup.cancel`, `terminal.open`, `terminal.write` · streams: `terminal.attach`, `subscribeWorktreeSetup` |

Totales: 97 unarios + 13 streams = 110. Por kind: `L` 41, `M·id` 3, `M·id†` 1, `M·set` 32, `M!` 20.

Los sitios de ot3 son rutas relativas a `src/`. «dinámico» = el tag llega de un llamador y el sitio con el literal está entre paréntesis. Los plazos que ot3 ya usa con `rpc_with_timeout` figuran en la columna de sitios.

### 1.1 `server` — configuración, ajustes y teclas

Ajustes y configuración del entorno. `getConfig`/`getSettings` son la lectura más repetida de ot3 (38 sitios entre las dos).

| Tag | Función propuesta | Payload | Respuesta | Kind | Sitios ot3 | En la lib hoy / fusión |
|---|---|---|---|---|---|---|
| `server.getConfig` | `server::config(conn)` | `{}` | `ServerConfig`: environment, auth, cwd, keybindings, issues, providers[], availableEditors[], settings, observability; opcionales: directEndpoints, remoteOpenTargets, scratchWorkspaceRoot, newProjectsRoot, flags de capacidad (`threadSnapshotPagination`, `threadFind`, `threadResumeCompletionMarker`, `reasoningMessages`…) | `L` | desktop_cloud.rs:309 terminal.rs:61 connection_access.rs:62 connection_route.rs:38 checkpoint_operations.rs:47 keybinding_settings.rs:180,232,252,280 preview_transport.rs:195 provider_instances.rs:224 thread_details.rs:35,45 provider_updates.rs:57,74 browser_profiles.rs:48 write.rs:19,35,377 server_capabilities.rs:27 auto_compact.rs:56 native_settings.rs:221 scoped_settings.rs:142 server_update.rs:91 suggestions.rs:26 | `models::model_catalog` (models.rs:42) ya llama al tag: pasa a `models::catalog(&server::config(conn)?)`. `server_capabilities.rs` (ot3) deriva los flags de este mismo JSON: candidato a `server::capabilities(&Value)->Capabilities` puro. |
| `server.getSettings` | `server::settings(conn)` | `{}` | `ServerSettings` (≈100 campos con default; `favorites` es `null` hasta que algún cliente migra; `providerInstances`, `deviceHosts`, `sourceControlHosts`… contienen secretos redactados) | `L` | git_controls.rs:247 provider_instances.rs:213,217,223 integration_devices.rs:281,383 write.rs:45,312 auto_compact.rs:81 native_settings.rs:223,228 scoped_settings.rs:144,158 | `settings::snapshot(raw)` (settings.rs:51) es la vista nativa y no cambia; `models::set_model_favorite` (models.rs:43) lee este tag y pasa a usar `server::settings`. |
| `server.updateSettings` | `server::update_settings(conn, patch, provider_instance_mutation?)` | `patch: ServerSettingsPatch` (todas las claves opcionales, ver `settings.ts:1386`); `providerInstanceMutation?: {operation:"create"/"upsert", instanceId, instance}` o `{operation:"remove", instanceId}` | `ServerSettings` completo tras aplicar el parche | `M·set` | connection_access.rs:113 provider_instances.rs:215 write.rs:44,312 native_settings.rs:224 scoped_settings.rs:157 integration_devices.rs:346 (dinámico; literal en :466) | `settings::patch({key,value})` (settings.rs:80) y `models::favorite_payload` (models.rs:37) ya construyen `{patch}`: se conservan como validadores y la función acepta su salida. El scope depende de las claves (`settings:write` o `providers:manage`, settings.ts:1551), así que el registro marca el scope como «dinámico». |
| `server.discoverSourceControl` | `server::discover_source_control(conn)` | `{}` | `{versionControlSystems[], sourceControlProviders[]}` | `L` | write.rs:36 | — |
| `server.reportClientActivity` | `server::report_client_activity(conn, &ClientActivity)` | `clientId`, `clientKind`, `visible`, `focused`, `recentlyInteracted`, `scopes[]`, `observedAt` (ISO UTC); opcionales: `environmentId`, `appState`, `lowPowerMode`, `batteryState`, `networkType`, `ttlMs` | vacío (`null`) | `M·set` | main.rs:157 | — (el payload ya prevé clientes móviles: `appState`, `lowPowerMode`, `batteryState`, `networkType`; `background.ts:67`) |
| `server.upsertKeybinding` | `server::keybindings::upsert(conn, key, command, when?, replace?)` | `key`, `command`, `when?`, `replace?: {key, command, when?}` | `{keybindings, issues}` | `M·set` | keybinding_settings.rs:246-252 (rpc en :250) | La parte pura de `keybinding_settings.rs` (ot3, §4.B de la auditoría) se mueve junto. |
| `server.removeKeybinding` | `server::keybindings::remove(conn, key, command, when?)` | `key`, `command`, `when?` | `{keybindings, issues}` | `M·set` | keybinding_settings.rs:248-252 (rpc en :250) | ídem |
| `server.searchAcpRegistry` | `server::acp::search(conn, query, limit?)` | `query` (≤ 120), `limit?` (1..máx. del registro) | `{agents[]}` | `L` | acp_registry.rs:10 | — |
| `server.prepareAcpRegistryAgent` | `server::acp::prepare(conn, agent_id)` | `agentId` | `{agentId, version, distribution, prepared:true}` | `M·set` | acp_registry.rs:16 | — (descarga un binario; hoy usa el plazo de 20 s de `rpc`: ver `timeout` en §3) |

Referencias de contrato (archivo:línea bajo `packages/contracts/src/`; `RpcAuthorization.ts` está en `apps/server/src/auth/` y `ws.ts` en `apps/server/src/`):

| Tag | `Rpc.make` | Clave del método | Payload (esquema) | Respuesta (esquema) | Scope exigido | Handler |
|---|---|---|---|---|---|---|
| `server.getConfig` | `rpc.ts:602` | `rpc.ts:474` | `Schema.Struct({})` `inline` | `ServerConfig` `server.ts:603` | `orchestration:read` (RpcAuthorization.ts:60) | `ws.ts:2079` |
| `server.getSettings` | `rpc.ts:758` | `rpc.ts:485` | `Schema.Struct({})` `inline` | `ServerSettings` `settings.ts:1093` | `orchestration:read` (RpcAuthorization.ts:85) | `ws.ts:2383` |
| `server.updateSettings` | `rpc.ts:764` | `rpc.ts:486` | `inline` `rpc.ts:764` | `ServerSettings` `settings.ts:1093` | `settings:write*` (RpcAuthorization.ts:86) | `ws.ts:2385` |
| `server.discoverSourceControl` | `rpc.ts:773` | `rpc.ts:487` | `Schema.Struct({})` `inline` | `SourceControlDiscoveryResult` `sourceControl.ts:166` | `orchestration:read` (RpcAuthorization.ts:98) | `ws.ts:2399` |
| `server.reportClientActivity` | `rpc.ts:927` | `rpc.ts:507` | `ClientActivityReportInput` `background.ts:67` | — | `orchestration:read` (RpcAuthorization.ts:110) | `ws.ts:2416` |
| `server.upsertKeybinding` | `rpc.ts:584` | `rpc.ts:480` | `ServerUpsertKeybindingInput` `server.ts:668` | `ServerUpsertKeybindingResult` `server.ts:679` | `settings:write` (RpcAuthorization.ts:81) | `ws.ts:2368` |
| `server.removeKeybinding` | `rpc.ts:590` | `rpc.ts:481` | `ServerRemoveKeybindingInput` `server.ts:676` | `ServerRemoveKeybindingResult` `server.ts:685` | `settings:write` (RpcAuthorization.ts:82) | `ws.ts:2374` |
| `server.searchAcpRegistry` | `rpc.ts:779` | `rpc.ts:488` | `AcpRegistrySearchInput` `acpRegistry.ts:52` | `AcpRegistrySearchResult` `acpRegistry.ts:77` | `orchestration:read` (RpcAuthorization.ts:87) | `ws.ts:2080` |
| `server.prepareAcpRegistryAgent` | `rpc.ts:785` | `rpc.ts:489` | `AcpRegistryPrepareInput` `acpRegistry.ts:82` | `AcpRegistryPrepareResult` `acpRegistry.ts:87` | `providers:manage` (RpcAuthorization.ts:88) | `ws.ts:2084` |

### 1.2 `diagnostics` — telemetría, uso y limpieza

Lecturas de diagnóstico y mantenimiento, con scopes mezclados: `diagnostics:read` (procesos, telemetría, uso), `orchestration:read` (`getHostResources`), `environment:maintain` (`signalProcess`) y `settings:write` (`checkOtlpEndpoint`, `runStorageCleanup`; protegidos por el cliente, `clientRpcPermissions.ts:17-18`).

| Tag | Función propuesta | Payload | Respuesta | Kind | Sitios ot3 | En la lib hoy / fusión |
|---|---|---|---|---|---|---|
| `server.getHostResources` | `diagnostics::host_resources(conn)` | `{}` | `{sampledAt, cpuUtilization (0..1 o null), cpuCount, availableMemoryBytes, totalMemoryBytes}` | `L` | connection_route.rs:40 | — |
| `server.getProcessDiagnostics` | `diagnostics::processes(conn)` | `{}` | `{serverPid, readAt, processCount, totalRssBytes, totalCpuPercent, processes[], error}`. `error` es un `Option` de Effect: llega como `{_tag:"Some",value:{message}}` o `{_tag:"None"}` (ot3 lee `error.value.message`, process_diagnostics.rs:25) | `L` | process_diagnostics.rs:17 | — |
| `server.getResourceTelemetryHistory` | `diagnostics::telemetry_history(conn, window_ms, bucket_ms)` | `windowMs`, `bucketMs` (enteros ≥ 0) | `{readAt, windowMs, bucketMs, sampleIntervalMs, retainedSampleCount, buckets[], topProcesses[], health}` | `L` | process_diagnostics.rs:27 | — |
| `server.signalProcess` | `diagnostics::signal_process(conn, pid, start_time_ms, signal)` | `pid` (> 0), `startTimeMs`, `signal` (`SIGINT`/`SIGKILL`/…) | `{pid, signal, signaled, message}` (`message` es `Option`) | `M·set` | process_diagnostics.rs:15 | El par `pid`+`startTimeMs` evita matar un pid reciclado: repetirla es inocua. |
| `server.checkOtlpEndpoint` | `diagnostics::check_otlp_endpoint(conn, signal, url)` | `signal` (`OtlpSignal`), `url` (`^https?://`) | unión por `_tag`: `Accepted{latencyMs}`, `Rejected{status}`, `Unreachable{timedOut}` | `L` | write.rs:345 | Sonda sin efecto, pero el servidor la protege con `settings:write` (clientRpcPermissions.ts:18). |
| `server.getUsageSummary` | `diagnostics::usage_summary(conn, &UsageQuery)` | `sinceDay`, `untilDay`, `timeZone`; opcionales: `resolution`, `sinceTime`, `untilTime`, `awaitRefresh` | `{contractVersion, readAt, timeZone, sinceDay, untilDay, buckets[], sources[], pricing, scanDurationMs}` | `L` | usage_summary.rs:100 | La agregación (`usage_summary::aggregate`) es pura y viaja en el lote 3 de la auditoría. |
| `server.refreshUsageRates` | `diagnostics::refresh_usage_rates(conn)` | `{}` | `UsagePricing{status, source, fetchedAt, knownModels}` | `L` | usage_summary.rs:97 | Refresca una tabla pública en el servidor; no toca estado del usuario. |
| `server.runStorageCleanup` | `diagnostics::run_storage_cleanup(conn)` | `{}` | `StorageCleanupReport{trigger, startedAt, finishedAt, entries[], counts{removed,kept,failed}, omittedCount, bytesFreed}` | `M!` | storage_cleanup.rs:35 | Borra worktrees y logs: no repetir a ciegas; releer con `diagnostics::storage_cleanup_report` (stream, §2). |

Referencias de contrato:

| Tag | `Rpc.make` | Clave del método | Payload (esquema) | Respuesta (esquema) | Scope exigido | Handler |
|---|---|---|---|---|---|---|
| `server.getHostResources` | `rpc.ts:863` | `rpc.ts:501` | `Schema.Struct({})` `inline` | `HostResourcesSnapshot` `resourceTelemetry.ts:9` | `orchestration:read` (RpcAuthorization.ts:103) | `ws.ts:2406` |
| `server.getProcessDiagnostics` | `rpc.ts:857` | `rpc.ts:500` | `Schema.Struct({})` `inline` | `ServerProcessDiagnosticsResult` `server.ts:427` | `diagnostics:read` (RpcAuthorization.ts:100) | `ws.ts:2405` |
| `server.getResourceTelemetryHistory` | `rpc.ts:876` | `rpc.ts:503` | `ResourceTelemetryHistoryInput` `resourceTelemetry.ts:455` | `ResourceTelemetryHistory` `resourceTelemetry.ts:495` | `diagnostics:read` (RpcAuthorization.ts:105) | `ws.ts:2409` |
| `server.signalProcess` | `rpc.ts:908` | `rpc.ts:506` | `ServerSignalProcessInput` `server.ts:505` | `ServerSignalProcessResult` `server.ts:512` | `environment:maintain` (RpcAuthorization.ts:109) | `ws.ts:2415` |
| `server.checkOtlpEndpoint` | `rpc.ts:890` | `rpc.ts:505` | `OtlpEndpointCheckInput` `server.ts:313` | `OtlpEndpointCheckResult` `server.ts:319` | `settings:write` (clientRpcPermissions.ts:18) | `ws.ts:2414` |
| `server.getUsageSummary` | `rpc.ts:896` | `rpc.ts:510` | `UsageSummaryInput` `usage.ts:132` | `UsageSummary` `usage.ts:147` | `diagnostics:read` (RpcAuthorization.ts:107) | `ws.ts:2411` |
| `server.refreshUsageRates` | `rpc.ts:902` | `rpc.ts:511` | `Schema.Struct({})` `inline` | `UsagePricing` `usage.ts:124` | `diagnostics:read` (RpcAuthorization.ts:108) | `ws.ts:2412` |
| `server.runStorageCleanup` | `rpc.ts:739` | `rpc.ts:482` | `Schema.Struct({})` `inline` | `StorageCleanupReport` `settings.ts:1064` | `settings:write` (clientRpcPermissions.ts:17) | `ws.ts:2380` |

### 1.3 `provider` — proveedores, instalación y autenticación

Todo lo de `providers:manage`. El flujo `auth.*` lleva credenciales del usuario en `respond`: la lib no debe registrarlas ni devolverlas.

| Tag | Función propuesta | Payload | Respuesta | Kind | Sitios ot3 | En la lib hoy / fusión |
|---|---|---|---|---|---|---|
| `server.refreshProviders` | `provider::refresh(conn, instance_id?, cwd?, fresh?, refresh_models?)` | todos opcionales: `instanceId`, `cwd`, `fresh`, `refreshModels` (solo petición explícita del usuario: fuerza redescubrir modelos) | `{providers: ServerProviders}` | `L` | quotas.rs:204 suggestions.rs:26 | `composer::catalog(provider, cwd, kind, query)` (composer.rs:16) consume `providers[].workspaceSnapshots`. |
| `server.updateProvider` | `provider::update(conn, provider, target_version?, instance_id?)` | `provider` (`ProviderDriverKind`), `targetVersion?`, `instanceId?` | `{providers}` | `M!` | provider_updates.rs:50,53 (`rpc_with_timeout` 300 s) | Usa `rpc_with_timeout` 300 s: el registro lleva el plazo (§3). |
| `provider.auth.start` | `provider::auth_start(conn, instance_id, method_id?, return_url?, callback_mode?)` | `instanceId`; opcionales: `methodId`, `returnUrl`, `callbackMode` (`server`/`client`) | `ProviderAuthState{instanceId, phase, flowId, authorizationUrl, expiresAt, message, methods?, interaction?, credentialOwner?}` | `M!` | provider_setup.rs:53 | — |
| `provider.auth.respond` | `provider::auth_respond(conn, instance_id, flow_id, interaction_id, &AuthResponse)` | `instanceId`, `flowId`, `interactionId`, `response`: `{type:"browser",action:"accept"/"decline"}`, `{type:"terminal",data (≤ 4096),size?}` o `{type:"credentials",values:{id: texto ≤ 16 384}}` | `ProviderAuthState` | `M!` | provider_setup.rs:57 | `provider_setup::response` (ot3, provider_setup.rs:29) valida la forma: moverla aquí. |
| `provider.auth.cancel` | `provider::auth_cancel(conn, instance_id, flow_id)` | `instanceId`, `flowId` | `ProviderAuthState` | `M·set` | provider_setup.rs:56 | — |
| `provider.auth.logout` | `provider::auth_logout(conn, instance_id)` | `instanceId` | `ProviderAuthState` | `M·set` | provider_setup.rs:55 | — |
| `provider.install.start` | `provider::install_start(conn, instance_id)` | `instanceId` | `ProviderInstallState{driver, operationId, phase, downloadedBytes, totalBytes, version, installedVersion, executablePath?, source?, canRemove, message}` | `M!` | provider_setup.rs:44 | — |
| `provider.install.cancel` | `provider::install_cancel(conn, instance_id, operation_id)` | `instanceId`, `operationId` | `ProviderInstallState` | `M·set` | provider_setup.rs:46 | — |
| `provider.install.remove` | `provider::install_remove(conn, instance_id)` | `instanceId` | `ProviderInstallState` | `M·set` | provider_setup.rs:45 | — |

Referencias de contrato:

| Tag | `Rpc.make` | Clave del método | Payload (esquema) | Respuesta (esquema) | Scope exigido | Handler |
|---|---|---|---|---|---|---|
| `server.refreshProviders` | `rpc.ts:608` | `rpc.ts:475` | `inline` `rpc.ts:608` | `ServerProviderUpdatedPayload` `server.ts:848` | `orchestration:read` (RpcAuthorization.ts:61) | `ws.ts:2179` |
| `server.updateProvider` | `rpc.ts:621` | `rpc.ts:476` | `ServerProviderUpdateInput` `server.ts:853` | `ServerProviderUpdatedPayload` `server.ts:848` | `providers:manage` (RpcAuthorization.ts:62) | `ws.ts:2309` |
| `provider.auth.start` | `rpc.ts:635` | `rpc.ts:411` | `ProviderAuthStartInput` `providerSetup.ts:102` | `ProviderAuthState` `providerSetup.ts:118` | `providers:manage` (RpcAuthorization.ts:63) | `ws.ts:2343` |
| `provider.auth.respond` | `rpc.ts:641` | `rpc.ts:418` | `ProviderAuthRespondInput` `providerSetup.ts:110` | `ProviderAuthState` `providerSetup.ts:118` | `providers:manage` (RpcAuthorization.ts:70) | `ws.ts:2344` |
| `provider.auth.cancel` | `rpc.ts:676` | `rpc.ts:419` | `ProviderAuthCancelInput` `providerSetup.ts:150` | `ProviderAuthState` `providerSetup.ts:118` | `providers:manage` (RpcAuthorization.ts:71) | `ws.ts:2355` |
| `provider.auth.logout` | `rpc.ts:682` | `rpc.ts:420` | `ProviderSetupInput` `providerSetup.ts:12` | `ProviderAuthState` `providerSetup.ts:118` | `providers:manage` (RpcAuthorization.ts:72) | `ws.ts:2356` |
| `provider.install.start` | `rpc.ts:695` | `rpc.ts:422` | `ProviderSetupInput` `providerSetup.ts:12` | `ProviderInstallState` `providerSetup.ts:158` | `providers:manage` (RpcAuthorization.ts:74) | `ws.ts:2359` |
| `provider.install.cancel` | `rpc.ts:701` | `rpc.ts:423` | `ProviderInstallCancelInput` `providerSetup.ts:181` | `ProviderInstallState` `providerSetup.ts:158` | `providers:manage` (RpcAuthorization.ts:75) | `ws.ts:2360` |
| `provider.install.remove` | `rpc.ts:714` | `rpc.ts:425` | `ProviderSetupInput` `providerSetup.ts:12` | `ProviderInstallState` `providerSetup.ts:158` | `providers:manage` (RpcAuthorization.ts:77) | `ws.ts:2362` |

### 1.4 `orchestration` — hilos, comandos y diffs

El núcleo del chat. `dispatchCommand` es un solo tag con ≈55 variantes por `type`; la lib debe tiparlas (ver la tabla de variantes) en lugar de dejar que Kotlin arme `{"type":…}` a mano.

| Tag | Función propuesta | Payload | Respuesta | Kind | Sitios ot3 | En la lib hoy / fusión |
|---|---|---|---|---|---|---|
| `orchestration.dispatchCommand` | `orchestration::dispatch(conn, &Command)` | unión por `type`, siempre con `commandId` (≈55 variantes; ≈ 34 las emite ot3 hoy, directa o indirectamente, tabla abajo) | `{sequence}` (secuencia del evento que dejó el comando) | `M·id` | checkpoint_operations.rs:271,302 archive_management.rs:342 branch_picker.rs:118,207 pull_request_actions.rs:110,178 write.rs:172,173,174,363,414 auto_compact.rs:273,290; más write.rs:203-243 dentro del despachador dinámico :258 (fork, merge_back, subagent.stop, metadata, archive…) | Ya existen constructores de payload dispersos: `queue::command` (queue.rs:31), `questions::command` (questions.rs:37), `approvals::command` (approvals.rs:61), `models::runtime_mode_command` (models.rs:7), `sidebar::command` y `sidebar::thread_action` (sidebar.rs:63,71; los usa `write.rs:238-243`). Fusionar: pasan a ser `Command::queue(..)`, `Command::answer(..)`, `Command::approve(..)`, `Command::runtime_mode(..)`, `Command::sidebar(..)` y `dispatch` los consume. `projects::command` NO va aquí: sus `project.update`/`project.delete` van a `projects.mutate` (write.rs:239). |
| `orchestration.launchThread` | `orchestration::launch_thread(conn, &ThreadLaunch)` | `commandId`, `projectId`, `title`, `modelSelection`, `runtimeMode`, `interactionMode`, `workspaceStrategy` (`{type:"root",branch?}` / `{type:"existing_worktree",worktreePath,branch?}` / `{type:"worktree",baseRef,branch?,startFromOrigin?}`); opcionales: `creationSource`, `threadId`, `reuseExistingThread`, `generateTitle`, `initialMessage{messageId?,text,context?,attachments[]}` | `{threadId, projection, resumed}` | `M·id` | write.rs:203 (→ :258) | `launch_command` (write.rs, ot3) se mueve a la lib. Deduplicado por `commandId`: ThreadMessageIntake.ts:15. |
| `orchestration.getThreadProjection` | `orchestration::thread_projection(conn, thread_id)` | `threadId` | `ThreadProjection{thread, runs, attempts, nodes, subagents, providerSessions, providerThreads, providerTurns, runtimeRequests, messages, plans, turnItems, checkpointScopes, checkpoints, contextHandoffs, contextTransfers, visibleTurnItems, updatedAt}` | `L` | checkpoint_operations.rs:35 | `projection::apply` (projection.rs) ya reduce estos eventos sobre este JSON. |
| `orchestration.getTurnItem` | `orchestration::turn_item(conn, thread_id, item_id, revision?)` | `threadId`, `itemId`, `revision?` | `{item: TurnItem o null}` | `L` | tool_inspector.rs:7,111,118 failure_context.rs:103,129 | `tool_inspector::request` (ot3) arma el payload: moverlo. |
| `orchestration.getTurnDiff` | `orchestration::turn_diff(conn, thread_id, from_turn, to_turn, ignore_whitespace?)` | `threadId`, `fromTurnCount` ≤ `toTurnCount`, `ignoreWhitespace?` | `{threadId, fromTurnCount, toTurnCount, diff}` | `L` | diff_review.rs:198,203 | — |
| `orchestration.getFullThreadDiff` | `orchestration::full_thread_diff(conn, thread_id, to_turn, ignore_whitespace?)` | `threadId`, `toTurnCount`, `ignoreWhitespace?` | `{threadId, fromTurnCount, toTurnCount, diff}` | `L` | diff_review.rs:198,203 | `diff_review.rs` elige entre este tag y el anterior según `from == 0`: la regla viaja con él. |
| `orchestration.getWorkflowScript` | `orchestration::workflow_script(conn, thread_id, script_path)` | `threadId`, `scriptPath` | `{scriptPath, contents, truncated}` | `L` | write.rs:346 | — |
| `orchestration.stopWorkflow` | `orchestration::stop_workflow(conn, thread_id, subagent_id)` | `threadId`, `subagentId` | vacío | `M·set` | write.rs:347 | Scope `orchestration:operate`. |
| `orchestration.searchThreads` | `orchestration::search_threads(conn, query, limit?)` | `query` (2..200), `limit?` (1..50) | `{matches[]}` | `L` | thread_search.rs:16 | — |
| `orchestration.getArchivedShellSnapshot` | `orchestration::archived_shell(conn)` | `{}` | `{schemaVersion, snapshotSequence, projects[], threads[]}` | `L` | archive_management.rs:163 | — |

Referencias de contrato:

| Tag | `Rpc.make` | Clave del método | Payload (esquema) | Respuesta (esquema) | Scope exigido | Handler |
|---|---|---|---|---|---|---|
| `orchestration.dispatchCommand` | `rpc.ts:1623` | `orchestrationV2.ts:3081` | `OrchestrationV2Command` `orchestrationV2.ts:2563` | `OrchestrationV2DispatchCommandResult` `orchestrationV2.ts:3191` | `orchestration:operate` (RpcAuthorization.ts:42) | `ws.ts:1807` |
| `orchestration.launchThread` | `rpc.ts:1705` | `orchestrationV2.ts:3092` | `OrchestrationV2ThreadLaunchInput` `orchestrationV2.ts:3161` | `OrchestrationV2ThreadLaunchResult` `orchestrationV2.ts:3184` | `orchestration:operate` (RpcAuthorization.ts:53) | `ws.ts:1938` |
| `orchestration.getThreadProjection` | `rpc.ts:1676` | `orchestrationV2.ts:3088` | `OrchestrationV2GetThreadProjectionInput` `orchestrationV2.ts:3196` | `OrchestrationV2ThreadProjection` `orchestrationV2.ts:1745` | `orchestration:read` (RpcAuthorization.ts:51) | `ws.ts:1909` |
| `orchestration.getTurnItem` | `rpc.ts:1699` | `orchestrationV2.ts:3091` | `OrchestrationV2GetTurnItemInput` `orchestrationV2.ts:3461` | `OrchestrationV2GetTurnItemResult` `orchestrationV2.ts:3468` | `orchestration:read` (RpcAuthorization.ts:52) | `ws.ts:1858` |
| `orchestration.getTurnDiff` | `rpc.ts:1629` | `orchestrationV2.ts:3082` | `OrchestrationGetTurnDiffInput` `checkpointDiff.ts:29` | `OrchestrationGetTurnDiffResult` `checkpointDiff.ts:38` | `orchestration:read` (RpcAuthorization.ts:45) | `ws.ts:1869` |
| `orchestration.getFullThreadDiff` | `rpc.ts:1636` | `orchestrationV2.ts:3083` | `OrchestrationGetFullThreadDiffInput` `checkpointDiff.ts:41` | `OrchestrationGetFullThreadDiffResult` `checkpointDiff.ts:48` | `orchestration:read` (RpcAuthorization.ts:46) | `ws.ts:1879` |
| `orchestration.getWorkflowScript` | `rpc.ts:1691` | `orchestrationV2.ts:3089` | `OrchestrationV2GetWorkflowScriptInput` `orchestrationV2.ts:3413` | `OrchestrationV2GetWorkflowScriptResult` `orchestrationV2.ts:3420` | `orchestration:read` (RpcAuthorization.ts:43) | `ws.ts:1856` |
| `orchestration.stopWorkflow` | `rpc.ts:1684` | `orchestrationV2.ts:3090` | `OrchestrationV2StopWorkflowInput` `orchestrationV2.ts:3375` | `Schema.Void` `void` | `orchestration:operate` (RpcAuthorization.ts:44) | `ws.ts:1855` |
| `orchestration.searchThreads` | `rpc.ts:1660` | `orchestrationV2.ts:3084` | `OrchestrationSearchThreadsInput` `threadSearch.ts:16` | `OrchestrationSearchThreadsResult` `threadSearch.ts:31` | `orchestration:read` (RpcAuthorization.ts:47) | `ws.ts:1897` |
| `orchestration.getArchivedShellSnapshot` | `rpc.ts:1667` | `orchestrationV2.ts:3087` | `Schema.Struct({})` `inline` | `OrchestrationV2ArchivedShellSnapshot` `orchestrationV2.ts:3134` | `orchestration:read` (RpcAuthorization.ts:50) | `ws.ts:1907` |

**Variantes de `dispatchCommand` que ot3 emite.** El contrato tiene 59 variantes (`orchestrationV2.ts:2563-3010`, unión `OrchestrationV2Command`); estas son las que ot3 emite hoy (directamente o por las funciones de `sidebar`, `queue`, `questions`, `approvals` y `models` de la lib), con su sitio en el contrato. El resto (`prepared-run.*`, `delegated_task.*`, `notification.delivery.accept`, `provider-session.detach`, `thread.created.record`, `provider.switch`, `thread.model-selection.set`, `thread.title.regeneration.complete`, `thread.pull-request-link.sync`, `thread.pull-request.sync`, `thread.active.reorder`, `queued-run.start`) las emiten el servidor o clientes que ot3 no es; entran al enum de la lib como `Command::Raw(Value)` hasta que alguien las necesite.

| `type` | Contrato | Campos (todas llevan `commandId` y `threadId` salvo las de creación) | Quién lo emite hoy |
|---|---|---|---|
| `thread.create` | `orchestrationV2.ts:2571` | commandId, threadId, projectId, title, modelSelection, runtimeMode, interactionMode, branch, worktreePath (+ campos de creación); importedNativeThread? | write.rs:203-243, pull_request_actions.rs:178 |
| `message.dispatch` | `orchestrationV2.ts:2786` | commandId, threadId, messageId, text, attachments[]; opcionales: context, titleSeed, modelSelection, sourcePlanRef, *ContinuationOfRunId…, delivery (`defer_start`/`steer_active`/`restart_active`/`queue_after_active`/`start_immediately`) | write.rs:172-174, auto_compact.rs:273, checkpoint_operations.rs:271 |
| `run.interrupt` | `orchestrationV2.ts:2863` | commandId, threadId, runId; reason?, holdQueue? | write.rs (despachador) |
| `subagent.stop` | `orchestrationV2.ts:2856` | commandId, threadId, runId, subagentId | write.rs:236 |
| `thread.metadata.update` | `orchestrationV2.ts:2685` | commandId, threadId; title?, regenerateTitle?, branch?, worktreePath?, expected*?, limitRecovery?, linkedPullRequest? | branch_picker.rs:118,207, write.rs:238 |
| `thread.runtime-mode.set / thread.interaction-mode.set` | `orchestrationV2.ts:2761,2767` | commandId, threadId, runtimeMode / interactionMode | write.rs:172-173; lib models.rs:7 |
| `thread.fork / thread.merge_back` | `orchestrationV2.ts:2936,2946` | commandId, sourceThreadId, targetThreadId, sourcePoint; title?, createdAt? | write.rs:224,230 |
| `thread.archive / unarchive / delete / settle / unsettle / snooze / unsnooze / auto-settle(.set) / pin / unpin / mark-unread / visit` | `orchestrationV2.ts:2594-2680` | commandId, threadId (+ settledAt?, snoozedUntil, enabled, orderKey?, visitedAt según la variante) | lib sidebar.rs:63,71 (← write.rs:238-243), write.rs:363, archive_management.rs:342 |
| `thread.pin.reorder` | `orchestrationV2.ts:2662` | commandId, threadId, orderKey | write.rs:414 |
| `queued-run.cancel / reorder / edit, queued-message.promote-to-steer, queue.resume` | `orchestrationV2.ts:2871-2915` | commandId, threadId, runId (+ beforeRunId, text, attachments?, queuedRunId, targetRunId) | lib queue.rs:31 |
| `runtime-request.respond, thread.user-input.dismiss` | `orchestrationV2.ts:2913,2922` | commandId, threadId, requestId; decision? o answers?; attachmentsByQuestionId? | lib approvals.rs:61, questions.rs:37 |
| `checkpoint.rollback` | `orchestrationV2.ts:2928` | commandId, threadId, scopeId, checkpointId; restoreFiles? | checkpoint_operations.rs:302 |
| `thread.pull-request.link / unlink / watch` | `orchestrationV2.ts:2701-2735` | commandId, threadId (+ clave y url del PR) | pull_request_actions.rs:110 |

`commandId` lo pone el host (ot3 usa `omarchy-<uuid>` y lo guarda en su outbox, `write.rs:248-262`): debe ser el mismo en cada reenvío para que la deduplicación del servidor funcione. La lib **valida** que exista y que cumpla `valid_id`, y nunca lo inventa. Hay una excepción razonable: un helper `ids::command_id(prefix)` que el host puede llamar, pero que la lib no invoca sola.

### 1.5 `vcs` — repositorio, ramas y acciones

Casi todo lo de escritura exige `source-control:write` (`clientRpcPermissions.ts:34-44`). `git.runStackedAction` es un stream en el contrato: su fila está en §2.

| Tag | Función propuesta | Payload | Respuesta | Kind | Sitios ot3 | En la lib hoy / fusión |
|---|---|---|---|---|---|---|
| `vcs.refreshStatus` | `vcs::refresh_status(conn, cwd)` | `cwd` | `VcsStatus{isRepo, hasPrimaryRemote, isDefaultRef, refName, hasWorkingTreeChanges, workingTree{rootPath?,files[],insertions,deletions}, branchChanges?, hasUpstream, aheadCount, behindCount, aheadOfDefaultCount?, pr, sourceControlProvider?}` | `L` | git_controls.rs:249 branch_picker.rs:181,194 | `git_controls::scope`/`build_*` (ot3) consumen este JSON. |
| `vcs.listRefs` | `vcs::list_refs(conn, cwd, &RefQuery)` | `cwd`; opcionales: `query` (≤ 256), `cursor`, `includeMatchingRemoteRefs`, `refKind` (`all`/`local`/`remote`), `refresh`, `limit` | `{refs[{name,isRemote?,remoteName?,upstreamRef?,current,isDefault,worktreePath}], isRepo, hasPrimaryRemote, nextCursor, totalCount, isAtRepositoryRoot?}` | `L` | working_review.rs:220 branch_picker.rs:78 | `branch_picker::dedupe/derive_local` (ot3) pos-procesan `refs`. |
| `vcs.createRef` | `vcs::create_ref(conn, cwd, ref_name, switch_ref?)` | `cwd`, `refName`, `switchRef?` | `{refName}` | `M!` | branch_picker.rs:99 | Falla si la rama existe: tras `rpc_outcome_unknown`, `list_refs` antes de reintentar. |
| `vcs.switchRef` | `vcs::switch_ref(conn, cwd, ref_name)` | `cwd`, `refName` | `{refName o null}` | `M·set` | branch_picker.rs:108,202 | — |
| `vcs.pull` | `vcs::pull(conn, cwd)` | `cwd` | `{status: "pulled"/"skipped_up_to_date", refName, upstreamRef}` | `M·set` | git_controls.rs:326 (paso del plan) → git_operations.rs:255 | ot3 valida `status` (git_operations.rs:256): la validación entra en la lib. |
| `vcs.init` | `vcs::init(conn, cwd, kind?)` | `cwd`, `kind?` (`VcsDriverKind`) | vacío | `M·set` | write.rs:348 | — |
| `git.preparePullRequestThread` | `vcs::prepare_pull_request_thread(conn, cwd, reference, mode, thread_id?)` | `cwd`, `reference` (texto), `mode` (`local`/`worktree`), `threadId?` | `{pullRequest, branch, worktreePath, isOnPullRequestHead}` | `M!` | pull_request_actions.rs:176 | Scope compuesto: `source-control:write` y, con `mode=worktree` + `threadId`, también `orchestration:operate` (`clientRpcPermissions.ts:62-66`). |

Referencias de contrato:

| Tag | `Rpc.make` | Clave del método | Payload (esquema) | Respuesta (esquema) | Scope exigido | Handler |
|---|---|---|---|---|---|---|
| `vcs.refreshStatus` | `rpc.ts:1360` | `rpc.ts:428` | `VcsStatusInput` `git.ts:109` | `VcsStatusResult` `git.ts:260` | `orchestration:read` (RpcAuthorization.ts:170) | `ws.ts:2802` |
| `vcs.listRefs` | `rpc.ts:1398` | `rpc.ts:429` | `VcsListRefsInput` `git.ts:138` | `VcsListRefsResult` `git.ts:280` | `orchestration:read` (RpcAuthorization.ts:172) | `ws.ts:2856` |
| `vcs.createRef` | `rpc.ts:1418` | `rpc.ts:432` | `VcsCreateRefInput` `git.ts:181` | `VcsCreateRefResult` `git.ts:188` | `source-control:write` (clientRpcPermissions.ts:42) | `ws.ts:2861` |
| `vcs.switchRef` | `rpc.ts:1424` | `rpc.ts:433` | `VcsSwitchRefInput` `git.ts:193` | `VcsSwitchRefResult` `git.ts:313` | `source-control:write` (clientRpcPermissions.ts:43) | `ws.ts:2863` |
| `vcs.pull` | `rpc.ts:1354` | `rpc.ts:427` | `VcsPullInput` `git.ts:121` | `VcsPullResult` `git.ts:347` | `source-control:write` (clientRpcPermissions.ts:37) | `ws.ts:2803` |
| `vcs.init` | `rpc.ts:1430` | `rpc.ts:434` | `VcsInitInput` `git.ts:199` | — | `source-control:write` (clientRpcPermissions.ts:44) | `ws.ts:2865` |
| `git.preparePullRequestThread` | `rpc.ts:1392` | `rpc.ts:438` | `GitPreparePullRequestThreadInput` `git.ts:166` | `GitPreparePullRequestThreadResult` `git.ts:300` | `source-control:write*` (clientRpcPermissions.ts:39) | `ws.ts:2852` |

### 1.6 `review` — diffs de trabajo

Lectura de diffs del árbol de trabajo o de un rango de ramas. Scope `filesystem:read`.

| Tag | Función propuesta | Payload | Respuesta | Kind | Sitios ot3 | En la lib hoy / fusión |
|---|---|---|---|---|---|---|
| `review.getDiffPreview` | `review::diff_preview(conn, cwd, base_ref?, ignore_whitespace?, file?)` | `cwd`; opcionales: `baseRef`, `ignoreWhitespace`, `file{path, previousPath (o null), sourceKind: "working-tree"/"branch-range"}` | `{cwd, generatedAt, sources[{id,kind,title,baseRef,headRef,diff,diffHash,truncated,files?}]}` | `L` | working_review.rs:237,253 review_document.rs:78 (dinámico),187 diff_review.rs:203 | `diff_review::present`/`scoped_result` (ot3) interpretan `sources[]`. |
| `review.getDiffFileContents` | `review::diff_file_contents(conn, &FileContentsQuery)` | `cwd`, `sourceKind`, `changeType` (`change`/`rename-pure`/`rename-changed`/`new`/`deleted`), `baseRef` (o null), `headRef` (o null), `oldPath`, `newPath` | `{oldContents, newContents}` | `L` | review_document.rs:78 (dinámico),182 | — |

Referencias de contrato:

| Tag | `Rpc.make` | Clave del método | Payload (esquema) | Respuesta (esquema) | Scope exigido | Handler |
|---|---|---|---|---|---|---|
| `review.getDiffPreview` | `rpc.ts:1435` | `rpc.ts:440` | `ReviewDiffPreviewInput` `review.ts:6` | `ReviewDiffPreviewResult` `review.ts:61` | `filesystem:read` (RpcAuthorization.ts:173) | `ws.ts:2867` |
| `review.getDiffFileContents` | `rpc.ts:1441` | `rpc.ts:441` | `ReviewDiffFileContentsInput` `review.ts:44` | `ReviewDiffFileContentsResult` `review.ts:55` | `filesystem:read` (RpcAuthorization.ts:174) | `ws.ts:2868` |

### 1.7 projects, project_clone y filesystem

El módulo `projects` ya existe en la lib (`projects.rs`, 32 líneas, solo payloads). Las funciones nuevas viven en `api::projects`; las viejas se reexportan.

| Tag | Función propuesta | Payload | Respuesta | Kind | Sitios ot3 | En la lib hoy / fusión |
|---|---|---|---|---|---|---|
| `projects.readFile` | `projects::read_file(conn, cwd, relative_path, allow_missing?)` | `cwd`, `relativePath`, `allowMissing?` | `{relativePath, contents, byteLength, truncated, exists?}` | `L` | documents.rs:18 journal_vault.rs:191 scoped_settings.rs:162 | — |
| `projects.searchEntries` | `projects::search_entries(conn, cwd, query, limit, kind?, image_only?)` | `cwd`, `query` (≤ 256), `limit` (> 0, ≤ máximo del contrato); opcionales: `kind`, `imageOnly` | `{entries[], truncated}` | `L` | suggestions.rs:22 | `composer::paths(result)` (composer.rs:12) post-procesa `entries`. |
| `projects.mutate` | `projects::mutate(conn, &ProjectMutation)` | unión por `type`: `project.create{commandId,projectId,title,workspaceRoot,createWorkspaceRootIfMissing?,defaultModelSelection?,scripts?}`, `project.update{commandId,projectId,title?,workspaceRoot?,defaultModelSelection?,autoPull?,projectIcon?,faviconPath?,defaultThreadEnvMode?,scripts?}`, `project.delete{commandId,projectId,force?}` | `Project{id,title,workspaceRoot,repositoryIdentity?,faviconPath?,projectIcon?,defaultModelSelection,defaultThreadEnvMode?,autoPull?,scripts[],createdAt,updatedAt,deletedAt}` | `M·id` | write.rs:216,239 agent_sessions.rs:20 | `projects::command` (projects.rs:13) ya arma `update`/`delete`: fusionar en `ProjectMutation::{create,update,delete}`. Con `scripts`, exige además `settings:write` (settings.ts:1575). Deduplicado: ProjectService.ts:423. |
| `projects.createNew` | `projects::create_new(conn, name)` | `name` (≤ 200) | `{projectId, workspaceRoot, commitError?}` | `M!` | write.rs:185 | ot3 ya lo trata como no repetible (write.rs:252). |
| `projectClone.start` | `projects::clone_start(conn, &CloneStart)` | `projectId`, `title`, `createdAt`, `destinationPath`; opcionales: `provider`, `repository`, `remoteUrl`, `protocol` | `{projectId, cwd, remoteUrl, repository}` | `M!` | write.rs:197 | no repetible (write.rs:252); scope `source-control:write`. |
| `filesystem.browse` | `filesystem::browse(conn, partial_path, cwd?)` | `partialPath` (≤ máx.), `cwd?` | `{parentPath, entries[{name, fullPath,…}]}` | `L` | write.rs:309 | `projects::folders(result)` (projects.rs:7) ya filtra y acota el resultado. |
| `filesystem.getMetadata` | `filesystem::metadata(conn, paths)` | `paths[]` (1..N, cada una ≤ máx.) | `{entries: (Metadata o null)[]}` | `L` | write.rs:208 journal_vault.rs:200 | — |

Referencias de contrato:

| Tag | `Rpc.make` | Clave del método | Payload (esquema) | Respuesta (esquema) | Scope exigido | Handler |
|---|---|---|---|---|---|---|
| `projects.readFile` | `rpc.ts:1231` | `rpc.ts:386` | `ProjectReadFileInput` `project.ts:435` | `ProjectReadFileResult` `project.ts:445` | `filesystem:read` (RpcAuthorization.ts:146) | `ws.ts:2660` |
| `projects.searchEntries` | `rpc.ts:1207` | `rpc.ts:388` | `ProjectSearchEntriesInput` `project.ts:240` | `ProjectSearchEntriesResult` `project.ts:256` | `filesystem:read` (RpcAuthorization.ts:148) | `ws.ts:2612` |
| `projects.mutate` | `rpc.ts:1243` | `rpc.ts:390` | `ProjectMutation` `project.ts:206` | `Project` `project.ts:151` | `orchestration:operate*` (RpcAuthorization.ts:58) | `ws.ts:2683` |
| `projects.createNew` | `rpc.ts:1255` | `rpc.ts:392` | `ProjectCreateNewInput` `project.ts:529` | `ProjectCreateNewResult` `project.ts:534` | `orchestration:operate` (RpcAuthorization.ts:151) | `ws.ts:2594` |
| `projectClone.start` | `rpc.ts:1172` | `rpc.ts:564` | `ProjectCloneStartInput` `projectClone.ts:49` | `ProjectCloneStartResult` `projectClone.ts:61` | `source-control:write` (clientRpcPermissions.ts:34) | `ws.ts:2551` |
| `filesystem.browse` | `rpc.ts:1272` | `rpc.ts:396` | `FilesystemBrowseInput` `filesystem.ts:31` | `FilesystemBrowseResult` `filesystem.ts:43` | `filesystem:read` (RpcAuthorization.ts:153) | `ws.ts:2699` |
| `filesystem.getMetadata` | `rpc.ts:1266` | `rpc.ts:397` | `FilesystemGetMetadataInput` `filesystem.ts:8` | `FilesystemGetMetadataResult` `filesystem.ts:26` | `filesystem:read` (RpcAuthorization.ts:154) | `ws.ts:2698` |

### 1.8 `pull_requests` — lectura y acciones de PR

Todas llevan `PullRequestRef{projectId, repository, number, host?, expectedAccountId?, allowStale?}`. La lib ya tiene `pull_requests::request(v)` (pull_requests.rs:3) que traduce `{action:"detail"/"checks"/"activity"/"list"}` a un tag y un payload; debe quedar como adaptador JSON de las funciones nuevas, no como segunda fuente de verdad. ot3 llega a `pullRequests.list` solo a través de esa función (no hay literal en `src/`).

| Tag | Función propuesta | Payload | Respuesta | Kind | Sitios ot3 | En la lib hoy / fusión |
|---|---|---|---|---|---|---|
| `pullRequests.detail` | `pull_requests::detail(conn, &PrRef)` | `PrRef` | `PullRequestDetail{provider, capabilities, viewerPermissions, projectId, projectTitle, workspaceRoot, repository, number, title, body, url, author, state, isDraft, mergeability, additions, deletions, changedFiles, headBranch,…}` | `L` | connection_github.rs:15,18,21,29,34 (dinámico) ← write.rs:301 | Cubierto por `pull_requests::request`; el enrutado entre entornos (`connection_github`) es del host. |
| `pullRequests.checks` | `pull_requests::checks(conn, &PrRef)` | `PrRef` | `{state, checks[]}` o `null` | `L` | connection_github.rs:15,18,21,29,34 (dinámico) | ídem |
| `pullRequests.activity` | `pull_requests::activity(conn, &PrRef)` | `PrRef` | `{author?, reviewers?, comments[], commentCount, commentsTruncated, reviewThreads[], commits[], reactions?}` | `L` | connection_github.rs:15,18,21,29,34 (dinámico) | ídem |
| `pullRequests.routing` | `pull_requests::routing(conn, &PrRef)` | `PrRef` | `{accountId, host, provider, viewer, projectTitle, workspaceRoot}` | `L` | connection_github.rs:20 | — |
| `pullRequests.routingIdentity` | `pull_requests::routing_identity(conn, host)` | `host` | `{accountId, host, provider:"github", viewer}` | `L` | connection_github.rs:26 | — |
| `pullRequests.reviewerCandidates` | `pull_requests::reviewer_candidates(conn, &PrRef)` | `PrRef` | `{candidates[], truncated}` | `L` | pull_request_actions.rs:53 (rpc en :81) | — |
| `pullRequests.labelCandidates` | `pull_requests::label_candidates(conn, &PrRef)` | `PrRef` | `{candidates[], truncated}` | `L` | pull_request_actions.rs:54 | — |
| `pullRequests.comment` | `pull_requests::comment(conn, &PrRef, body)` | `PrRef` + `body` | vacío | `M!` | pull_request_actions.rs:26 | Un reintento duplica el comentario. |
| `pullRequests.submitReview` | `pull_requests::submit_review(conn, &PrRef, verdict, body, comments)` | `PrRef` + `verdict` (`comment`/`approve`/`request-changes`), `body` (≤ 65 536), `comments[]` | vacío | `M!` | pull_request_actions.rs:37 | — |
| `pullRequests.runAction` | `pull_requests::run_action(conn, &PrRef, &PrAction)` | `PrRef` + `action` (`merge`, `ready`, `draft`, `close`, `reopen`, `delete-branch`, `update-branch`, `enable-auto-merge`, `disable-auto-merge`, `revert`, `approve-workflows`); opcionales: `mergeMethod`, `updateMethod`, `stackNumber`, `expectedStackHeads` | vacío | `M!` | pull_request_actions.rs:51 | `pull_request_actions::payload` (ot3) valida la forma. |
| `pullRequests.requestReviewers` | `pull_requests::request_reviewers(conn, &PrRef, reviewers, requested)` | `PrRef` + `reviewers[1..25]{id,kind}` + `requested` | vacío | `M·set` | pull_request_actions.rs:65 | — |
| `pullRequests.setLabels` | `pull_requests::set_labels(conn, &PrRef, labels, applied)` | `PrRef` + `labels[1..25]` + `applied` | vacío | `M·set` | pull_request_actions.rs:74 | — |

Referencias de contrato:

| Tag | `Rpc.make` | Clave del método | Payload (esquema) | Respuesta (esquema) | Scope exigido | Handler |
|---|---|---|---|---|---|---|
| `pullRequests.detail` | `rpc.ts:996` | `rpc.ts:534` | `PullRequestRef` `pullRequest.ts:406` | `PullRequestDetail` `pullRequest.ts:544` | `orchestration:read` (RpcAuthorization.ts:128) | `ws.ts:2477` |
| `pullRequests.checks` | `rpc.ts:1008` | `rpc.ts:536` | `PullRequestRef` `pullRequest.ts:406` | `NullOr<PullRequestChecks>` `pullRequest.ts:590` | `orchestration:read` (RpcAuthorization.ts:130) | `ws.ts:2481` |
| `pullRequests.activity` | `rpc.ts:1014` | `rpc.ts:537` | `PullRequestRef` `pullRequest.ts:406` | `PullRequestActivity` `pullRequest.ts:596` | `orchestration:read` (RpcAuthorization.ts:131) | `ws.ts:2483` |
| `pullRequests.routing` | `rpc.ts:966` | `rpc.ts:530` | `PullRequestRef` `pullRequest.ts:406` | `PullRequestRoutingResult` `pullRequest.ts:429` | `orchestration:read` (RpcAuthorization.ts:124) | `ws.ts:2462` |
| `pullRequests.routingIdentity` | `rpc.ts:972` | `rpc.ts:531` | `PullRequestRoutingIdentityInput` `pullRequest.ts:416` | `PullRequestRoutingIdentityResult` `pullRequest.ts:421` | `orchestration:read` (RpcAuthorization.ts:125) | `ws.ts:2461` |
| `pullRequests.reviewerCandidates` | `rpc.ts:1134` | `rpc.ts:556` | `PullRequestRef` `pullRequest.ts:406` | `PullRequestReviewerCandidateList` `pullRequest.ts:212` | `orchestration:read` (RpcAuthorization.ts:140) | `ws.ts:2539` |
| `pullRequests.labelCandidates` | `rpc.ts:1146` | `rpc.ts:558` | `PullRequestRef` `pullRequest.ts:406` | `PullRequestLabelCandidateList` `pullRequest.ts:225` | `orchestration:read` (RpcAuthorization.ts:141) | `ws.ts:2543` |
| `pullRequests.comment` | `rpc.ts:1074` | `rpc.ts:547` | `PullRequestCommentInput` `pullRequest.ts:744` | `Schema.Void` `void` | `source-control:write` (clientRpcPermissions.ts:21) | `ws.ts:2511` |
| `pullRequests.submitReview` | `rpc.ts:1086` | `rpc.ts:549` | `PullRequestSubmitReviewInput` `pullRequest.ts:800` | `Schema.Void` `void` | `source-control:write` (clientRpcPermissions.ts:23) | `ws.ts:2515` |
| `pullRequests.runAction` | `rpc.ts:1062` | `rpc.ts:545` | `PullRequestActionInput` `pullRequest.ts:729` | `Schema.Void` `void` | `source-control:write` (clientRpcPermissions.ts:19) | `ws.ts:2499` |
| `pullRequests.requestReviewers` | `rpc.ts:1140` | `rpc.ts:557` | `PullRequestReviewerRequestInput` `pullRequest.ts:843` | `Schema.Void` `void` | `source-control:write` (clientRpcPermissions.ts:28) | `ws.ts:2541` |
| `pullRequests.setLabels` | `rpc.ts:1152` | `rpc.ts:559` | `PullRequestLabelChangeInput` `pullRequest.ts:852` | `Schema.Void` `void` | `source-control:write` (clientRpcPermissions.ts:29) | `ws.ts:2545` |

### 1.9 `preview` — navegador del hilo

Sesiones del navegador de vista previa. `preview.list` es el único con scope de lectura; el resto exige `preview:operate`. Android no tendrá el navegador de ot3, pero sí puede mostrar el estado (`list`, `subscribePreviewEvents`).

| Tag | Función propuesta | Payload | Respuesta | Kind | Sitios ot3 | En la lib hoy / fusión |
|---|---|---|---|---|---|---|
| `preview.list` | `preview::list(conn, thread_id)` | `threadId` | `{sessions[PreviewSession], serverEpoch, revision}` | `L` | preview_transport.rs:209,256,316 preview_actions.rs:135 | `preview_transport::list` (ot3) valida `serverEpoch`/`revision`. |
| `preview.open` | `preview::open(conn, thread_id, url?, viewport?, profile_id?, runtime?, reveal?)` | `threadId`; opcionales: `url`, `viewport`, `profileId`, `runtime`, `reveal` | `PreviewSession{threadId, tabId, navStatus, canGoBack, canGoForward, viewport?, colorScheme?, zoomFactor?, profileId?, runtime?, automationOwner?, reveal?, revealRequest?, updatedAt}` | `M!` | preview_actions.rs:66 (rpc en :164) | Crea una pestaña: releer con `list` antes de reintentar. |
| `preview.navigate` | `preview::navigate(conn, thread_id, tab_id, url, resolved_title?)` | `threadId`, `tabId`, `url`, `resolvedTitle?` | `PreviewSession` | `M·set` | preview_actions.rs:81 | — |
| `preview.resize` | `preview::resize(conn, thread_id, tab_id, viewport)` | `threadId`, `tabId`, `viewport` | `PreviewSession` | `M·set` | preview_actions.rs:85 | — |
| `preview.adjust` | `preview::adjust(conn, thread_id, tab_id, color_scheme?, zoom_factor?, hard_reload?, clear?)` | `threadId`, `tabId`; opcionales: `colorScheme`, `zoomFactor`, `hardReload`, `clear` (`cookies`/`cache`) | `PreviewSession` | `M·set` | preview_actions.rs:108 | — |
| `preview.refresh` | `preview::refresh(conn, thread_id, tab_id)` | `threadId`, `tabId` | vacío | `M·set` | preview_actions.rs:110 | — |
| `preview.close` | `preview::close(conn, thread_id, tab_id?)` | `threadId`, `tabId?` | vacío | `M·set` | preview_actions.rs:111 | — |
| `preview.clearProfile` | `preview::clear_profile(conn, profile_id)` | `profileId` | vacío | `M·set` | browser_profiles.rs:220 (`rpc_with_timeout` 60 s) | — |
| `preview.reportStatus` | `preview::report_status(conn, thread_id, tab_id, nav_status, can_go_back, can_go_forward)` | `threadId`, `tabId`, `navStatus`, `canGoBack`, `canGoForward` | vacío | `M·set` | preview_transport.rs:290 | — |

Referencias de contrato:

| Tag | `Rpc.make` | Clave del método | Payload (esquema) | Respuesta (esquema) | Scope exigido | Handler |
|---|---|---|---|---|---|---|
| `preview.list` | `rpc.ts:1527` | `rpc.ts:458` | `PreviewListInput` `preview.ts:255` | `PreviewListResult` `preview.ts:260` | `orchestration:read` (RpcAuthorization.ts:191) | `ws.ts:2904` |
| `preview.open` | `rpc.ts:1493` | `rpc.ts:452` | `PreviewOpenInput` `preview.ts:185` | `PreviewSessionSnapshot` `preview.ts:162` | `preview:operate` (RpcAuthorization.ts:185) | `ws.ts:2898` |
| `preview.navigate` | `rpc.ts:1499` | `rpc.ts:453` | `PreviewNavigateInput` `preview.ts:195` | `PreviewSessionSnapshot` `preview.ts:162` | `preview:operate` (RpcAuthorization.ts:186) | `ws.ts:2899` |
| `preview.resize` | `rpc.ts:1505` | `rpc.ts:454` | `PreviewResizeInput` `preview.ts:218` | `PreviewSessionSnapshot` `preview.ts:162` | `preview:operate` (RpcAuthorization.ts:187) | `ws.ts:2900` |
| `preview.adjust` | `rpc.ts:1511` | `rpc.ts:455` | `PreviewAdjustInput` `preview.ts:228` | `PreviewSessionSnapshot` `preview.ts:162` | `preview:operate` (RpcAuthorization.ts:188) | `ws.ts:2901` |
| `preview.refresh` | `rpc.ts:1517` | `rpc.ts:456` | `PreviewRefreshInput` `preview.ts:212` | — | `preview:operate` (RpcAuthorization.ts:189) | `ws.ts:2902` |
| `preview.close` | `rpc.ts:1522` | `rpc.ts:457` | `PreviewCloseInput` `preview.ts:238` | — | `preview:operate` (RpcAuthorization.ts:190) | `ws.ts:2903` |
| `preview.clearProfile` | `rpc.ts:1533` | `rpc.ts:459` | `PreviewClearProfileInput` `preview.ts:250` | — | `preview:operate` (RpcAuthorization.ts:192) | `ws.ts:2905` |
| `preview.reportStatus` | `rpc.ts:1543` | `rpc.ts:461` | `PreviewReportStatusInput` `preview.ts:203` | — | `preview:operate` (RpcAuthorization.ts:194) | `ws.ts:2907` |

### 1.10 `device` — emuladores, simuladores y hosts

El panel de dispositivos. Lleva plazos largos propios (`rpc_with_timeout`): el registro los fija por función. La lib ya tiene `device_hub.rs` para el hub HTTP/WebSocket, que no se toca.

| Tag | Función propuesta | Payload | Respuesta | Kind | Sitios ot3 | En la lib hoy / fusión |
|---|---|---|---|---|---|---|
| `device.list` | `device::list(conn, &ListOpts)` | opcionales: `updateTool` (`hub`/`agent`), `inspectOnly`, `retryHostId` | `DeviceServiceState{hosts[], …}` | `L` | integration_devices.rs:415,417,429,445 (rpc en :346, 300 s) | Con `updateTool` o `retryHostId` instala o reinicia herramientas y exige `orchestration:operate` (RpcAuthorization.ts:292-294): el registro lo marca como «dinámico». |
| `device.configure` | `device::configure(conn, enabled?, agent_access_enabled?, onboarding_completed?)` | los tres opcionales | `DeviceServiceState` | `M·set` | integration_devices.rs:409 (300 s) | Scope `settings:write`. |
| `device.testHost` | `device::test_host(conn, &SshHost)` | `kind:"ssh"`, `id`, `label`, `target` (sin espacios ni `-` inicial), `identityFile?`, `port?` (1..65 535) | `DeviceHostSummary` | `L` | integration_devices.rs:451 (300 s) | Sonda; exige `settings:write`. |
| `device.open` | `device::open(conn, thread_id, device_id, platform, host_id?, boot?)` | `threadId`, `deviceId`, `platform`; `hostId?`, `boot?` | `DeviceSession{threadId, hostId, deviceId, platform, openedAt}` | `M!` | device_screen.rs:280 (180 s) | — |
| `device.close` | `device::close(conn, thread_id, host_id?, device_id?, shutdown?)` | `threadId`; `hostId?`, `deviceId?`, `shutdown?` | vacío | `M·set` | device_screen.rs:112 (60 s) | El servidor responde `rpc_rejected` si el apagado tarda más de 15 s; ot3 lo traduce a «apagándose» (device_screen.rs:120): regla del host. |
| `device.detail` | `device::detail(conn, device_id, host_id?)` | `deviceId`, `hostId?` | `DeviceDetail{hostId, deviceId, settings, foregroundApp, readAt}` | `L` | device_screen.rs:87,93 (20 s) | — |
| `device.action` | `device::action(conn, &DeviceAction)` | unión por `type` con `deviceId`, `hostId?`: `setAppearance`, `setTextSize`, `setToggle`, `setLiquidGlass`, `setColorFilter`, `setOrientation`, `setLocation`, `clearLocation`, `setPermission`, `openUrl`, `launchApp`, `terminateApp`, `shake`, `sendPush` | `DeviceDetail` | `M!` | device_screen.rs:92 (30 s) | Las variantes `set*` son idempotentes; `shake`, `sendPush`, `launchApp` no. La lib marca el conjunto como `M!`. |

Referencias de contrato:

| Tag | `Rpc.make` | Clave del método | Payload (esquema) | Respuesta (esquema) | Scope exigido | Handler |
|---|---|---|---|---|---|---|
| `device.list` | `rpc.ts:1576` | `rpc.ts:464` | `DeviceListInput` `device.ts:255` | `DeviceServiceState` `device.ts:230` | `orchestration:read*` (RpcAuthorization.ts:200) | `ws.ts:2912` |
| `device.configure` | `rpc.ts:1582` | `rpc.ts:463` | `DeviceConfigureInput` `device.ts:262` | `DeviceServiceState` `device.ts:230` | `settings:write` (RpcAuthorization.ts:197) | `ws.ts:2909` |
| `device.testHost` | `rpc.ts:1564` | `rpc.ts:465` | `SshDeviceHostConfig` `device.ts:21` | `DeviceHostSummary` `device.ts:164` | `settings:write` (RpcAuthorization.ts:198) | `ws.ts:2910` |
| `device.open` | `rpc.ts:1588` | `rpc.ts:467` | `DeviceOpenInput` `device.ts:269` | `DeviceSession` `device.ts:177` | `orchestration:operate` (RpcAuthorization.ts:201) | `ws.ts:2923` |
| `device.close` | `rpc.ts:1594` | `rpc.ts:468` | `DeviceCloseInput` `device.ts:279` | — | `orchestration:operate` (RpcAuthorization.ts:202) | `ws.ts:2927` |
| `device.detail` | `rpc.ts:1604` | `rpc.ts:470` | `DeviceDetailInput` `device.ts:443` | `DeviceDetail` `device.ts:343` | `orchestration:read` (RpcAuthorization.ts:204) | `ws.ts:2929` |
| `device.action` | `rpc.ts:1610` | `rpc.ts:471` | `DeviceActionInput` `device.ts:372` | `DeviceDetail` `device.ts:343` | `orchestration:operate` (RpcAuthorization.ts:205) | `ws.ts:2930` |

### 1.11 `scheduled_tasks` — tareas programadas

Cinco funciones, un solo archivo de ot3 (`scheduled_tasks.rs:58-66`).

| Tag | Función propuesta | Payload | Respuesta | Kind | Sitios ot3 | En la lib hoy / fusión |
|---|---|---|---|---|---|---|
| `scheduledTasks.list` | `scheduled_tasks::list(conn)` | `{}` | `{tasks[]}` | `L` | scheduled_tasks.rs:58,66 | — |
| `scheduledTasks.upsert` | `scheduled_tasks::upsert(conn, &TaskUpsert)` | `title`, `prompt`, `enabled`, `schedule` (`{type:"interval",everyMs ≥ 60 000}`, `fixed_time` o `webhook`), `projectId`, `workspaceStrategy`, `modelSelection`, `runtimeMode`, `interactionMode`; opcionales: `id`, `requireExisting`, `commandId`, `threadId`, `createdBy`, `creationSource` | `{task}` | `M·id†` | scheduled_tasks.rs:59 | † Lleva `commandId` opcional; no verifiqué que el servidor lo deduplique. Sin `id` ni `commandId`, un reintento crea otra tarea. |
| `scheduledTasks.setEnabled` | `scheduled_tasks::set_enabled(conn, id, enabled)` | `id`, `enabled` | `{task}` | `M·set` | scheduled_tasks.rs:60 | — |
| `scheduledTasks.delete` | `scheduled_tasks::delete(conn, id)` | `id` | `{id}` | `M·set` | scheduled_tasks.rs:61 | — |
| `scheduledTasks.runNow` | `scheduled_tasks::run_now(conn, id)` | `id` | `{task}` | `M!` | scheduled_tasks.rs:62 | Lanza un hilo real: no repetir. |

Referencias de contrato:

| Tag | `Rpc.make` | Clave del método | Payload (esquema) | Respuesta (esquema) | Scope exigido | Handler |
|---|---|---|---|---|---|---|
| `scheduledTasks.list` | `rpc.ts:1787` | `rpc.ts:513` | `ScheduledTaskListInput` `scheduledTask.ts:259` | `ScheduledTaskListResult` `scheduledTask.ts:262` | `orchestration:read` (RpcAuthorization.ts:113) | `ws.ts:2045` |
| `scheduledTasks.upsert` | `rpc.ts:1800` | `rpc.ts:515` | `ScheduledTaskUpsertInput` `scheduledTask.ts:269` | `ScheduledTaskMutationResult` `scheduledTask.ts:386` | `orchestration:operate` (clientRpcPermissions.ts:46) | `ws.ts:2049` |
| `scheduledTasks.setEnabled` | `rpc.ts:1806` | `rpc.ts:516` | `ScheduledTaskSetEnabledInput` `scheduledTask.ts:290` | `ScheduledTaskMutationResult` `scheduledTask.ts:386` | `orchestration:operate` (clientRpcPermissions.ts:47) | `ws.ts:2050` |
| `scheduledTasks.delete` | `rpc.ts:1812` | `rpc.ts:517` | `ScheduledTaskDeleteInput` `scheduledTask.ts:296` | `ScheduledTaskDeleteResult` `scheduledTask.ts:391` | `orchestration:operate` (clientRpcPermissions.ts:48) | `ws.ts:2054` |
| `scheduledTasks.runNow` | `rpc.ts:1818` | `rpc.ts:518` | `ScheduledTaskRunNowInput` `scheduledTask.ts:301` | `ScheduledTaskRunNowResult` `scheduledTask.ts:396` | `orchestration:operate` (clientRpcPermissions.ts:49) | `ws.ts:2058` |

### 1.12 agent_sessions, mcp_apps, assets, shell, secrets, worktree_setup y terminal

Los módulos pequeños: 14 tags en total.

| Tag | Función propuesta | Payload | Respuesta | Kind | Sitios ot3 | En la lib hoy / fusión |
|---|---|---|---|---|---|---|
| `agentSessions.scan` | `agent_sessions::scan(conn)` | `{}` | `{candidates[], scannedAt, truncated?}` | `L` | agent_sessions.rs:9 | — |
| `agentSessions.import` | `agent_sessions::import(conn, project_id, expected_workspace_root?)` | `projectId`, `expectedWorkspaceRoot?` | `{importedCount, skippedCount}` | `M·set` | agent_sessions.rs:24 | `skippedCount` indica que repetirla no duplica. |
| `mcpApps.callTool` | `mcp_apps::call_tool(conn, thread_id, item_id, name, arguments)` | `threadId`, `itemId`, `name`, `arguments` (objeto) | `{content[], structuredContent?, isError?, _meta?}` | `M!` | mcp_apps.rs:17 (rpc en :42) | `documents::mcp_app(item)` (documents.rs:35) construye la referencia de la app. |
| `mcpApps.readResource` | `mcp_apps::read_resource(conn, thread_id, item_id, uri)` | `threadId`, `itemId`, `uri` | `{contents[]}` | `L` | mcp_apps.rs:21 | — |
| `mcpApps.updateModelContext` | `mcp_apps::update_model_context(conn, thread_id, item_id, conversation_thread_id, content?, structured_content?)` | `threadId`, `itemId`, `conversationThreadId`; `content?`, `structuredContent?` | vacío (ot3 devuelve `{}`, mcp_apps.rs:43) | `M·set` | mcp_apps.rs:32 | — |
| `assets.createUrl` | `assets::create_url(conn, &AssetResource)` | `resource`: `{_tag:"workspace-file"/"media-file", threadId, path}`, `{_tag:"draft-workspace-file", cwd, path}`, `{_tag:"attachment", attachmentId, fileName?, mimeType?, disposition?}`, `{_tag:"project-favicon", cwd, path}`… | `{relativeUrl (≤ 4096), expiresAt, sourcePath?, imageDimensions?}` | `L` | notification_icon.rs:121 checkpoint_operations.rs:182 lib.rs:74 | En la lib lo llaman `Connection::document` (documents.rs:13) e `image_resource` (lib.rs:~725): ambos pasan a `assets::create_url`. El scope cambia con el recurso (`filesystem:read` para archivos, `orchestration:read` para adjuntos; RpcAuthorization.ts:254-262). |
| `attachments.delete` | `assets::delete_attachment(conn, attachment_id)` | `attachmentId` (≤ 256) | vacío | `M·set` | write.rs:365 | — |
| `shell.openInEditor` | `shell::open_in_editor(conn, cwd, editor, reveal?)` | `cwd`, `editor` (`EditorId`), `reveal?` | vacío | `M!` | keybinding_settings.rs:291 thread_details.rs:53 | Abre una ventana en el equipo del servidor. |
| `secrets.answerRequest` | `secrets::answer(conn, thread_id, turn_item_id, &SecretAnswer)` | `threadId`, `turnItemId`, `answer`: `{type:"save", secret}` o `{type:"decline"}` | vacío | `M!` | write.rs:403 | El secreto no debe quedar en logs ni en la caché de la lib. |
| `worktreeSetup.cancel` | `worktree_setup::cancel(conn, thread_id)` | `threadId` | `{cancelled}` | `M·set` | write.rs:344 | — |
| `terminal.open` | `terminal::open(conn, thread_id, terminal_id, cwd, &OpenOpts)` | `threadId`, `terminalId`, `cwd`; opcionales: `worktreePath`, `cols`, `rows`, `env`, `providerInstanceId` | `TerminalSessionSnapshot{threadId, terminalId, cwd, worktreePath, status, pid, history, exitCode, exitSignal, label, updatedAt, sequence?}` | `M·set` | git_controls.rs:312 (paso del plan) → git_operations.rs:266 | `terminal.rs` de la lib ya tiene `session`, `dimensions`, `attach`, `write`, `resize` (payloads) y `Connection::terminal_write/terminal_resize` (terminal.rs:44-45): `api::terminal` los reexporta y suma `open`. |
| `terminal.write` | `terminal::write(conn, thread_id, terminal_id, data)` | `threadId`, `terminalId`, `data` (1..65 536 bytes) | vacío | `M!` | git_controls.rs:312 (paso del plan) → git_operations.rs:270 | Ya existe como `Connection::terminal_write`. Un reintento escribe dos veces. |

Referencias de contrato:

| Tag | `Rpc.make` | Clave del método | Payload (esquema) | Respuesta (esquema) | Scope exigido | Handler |
|---|---|---|---|---|---|---|
| `agentSessions.scan` | `rpc.ts:1278` | `rpc.ts:398` | `AgentSessionScanInput` `agentSessions.ts:23` | `AgentSessionScanResult` `agentSessions.ts:44` | `orchestration:read` (RpcAuthorization.ts:155) | `ws.ts:2712` |
| `agentSessions.import` | `rpc.ts:1284` | `rpc.ts:399` | `AgentSessionImportInput` `agentSessions.ts:51` | `AgentSessionImportResult` `agentSessions.ts:75` | `orchestration:operate` (RpcAuthorization.ts:156) | `ws.ts:2713` |
| `mcpApps.callTool` | `rpc.ts:1318` | `rpc.ts:405` | `McpAppCallToolInput` `mcpApps.ts:14` | `McpAppCallToolResult` `mcpApps.ts:21` | `orchestration:operate` (RpcAuthorization.ts:162) | `ws.ts:2251` |
| `mcpApps.readResource` | `rpc.ts:1335` | `rpc.ts:407` | `McpAppReadResourceInput` `mcpApps.ts:43` | `McpAppReadResourceResult` `mcpApps.ts:49` | `orchestration:read` (RpcAuthorization.ts:164) | `ws.ts:2254` |
| `mcpApps.updateModelContext` | `rpc.ts:1330` | `rpc.ts:408` | `McpAppUpdateModelContextInput` `mcpApps.ts:57` | — | `orchestration:operate` (RpcAuthorization.ts:165) | `ws.ts:2253` |
| `assets.createUrl` | `rpc.ts:1295` | `rpc.ts:400` | `AssetCreateUrlInput` `assets.ts:60` | `AssetCreateUrlResult` `assets.ts:71` | `orchestration:read*` (RpcAuthorization.ts:157) | `ws.ts:2715` |
| `attachments.delete` | `rpc.ts:1313` | `rpc.ts:403` | `AttachmentDeleteInput` `assets.ts:116` | — | `orchestration:operate` (RpcAuthorization.ts:160) | `ws.ts:2711` |
| `shell.openInEditor` | `rpc.ts:1261` | `rpc.ts:394` | `LaunchEditorInput` `editor.ts:158` | — | `orchestration:operate` (RpcAuthorization.ts:152) | `ws.ts:2697` |
| `secrets.answerRequest` | `rpc.ts:1833` | `rpc.ts:520` | `SecretRequestAnswerInput` `secretRequest.ts:9` | — | `orchestration:operate` (RpcAuthorization.ts:115) | `ws.ts:2066` |
| `worktreeSetup.cancel` | `rpc.ts:1373` | `rpc.ts:571` | `WorktreeSetupCancelInput` `worktreeSetup.ts:74` | `WorktreeSetupCancelResult` `worktreeSetup.ts:79` | `orchestration:operate` (RpcAuthorization.ts:168) | `ws.ts:2798` |
| `terminal.open` | `rpc.ts:1447` | `rpc.ts:443` | `TerminalOpenInput` `terminal.ts:36` | `TerminalSessionSnapshot` `terminal.ts:101` | `terminal:operate` (RpcAuthorization.ts:175) | `ws.ts:2869` |
| `terminal.write` | `rpc.ts:1467` | `rpc.ts:446` | `TerminalWriteInput` `terminal.ts:64` | — | `terminal:operate` (RpcAuthorization.ts:178) | `ws.ts:2877` |

### 1.13 Fusiones con lo que la lib ya tiene

Regla: la lib no gana segundas fuentes de verdad. Cada módulo existente se queda como **validador/constructor puro**, y la función nueva de `api` lo llama.

| Módulo existente | Qué contiene | Decisión |
|---|---|---|
| `pull_requests.rs` (245 líneas) | `request(v)` → `(tag, payload)` para `list`/`detail`/`checks`/`activity`/`diff`; `sidebar_badge` | `request` queda como adaptador JSON y llama a `api::pull_requests::*`. `list` y `diff` (que es HTTP, `/api/pull-requests/diff`) no tienen fila porque ot3 no los llama con literal; `pullRequests.list` está en el contrato (`rpc.ts:949`) y la lib ya lo arma, así que se agrega `pull_requests::list` en la tanda 2. |
| `projects.rs` (32) | `command(input)` → `project.update`/`project.delete`; `folders(result)` | `command` pasa a construir `ProjectMutation`; `folders` es el post-proceso de `filesystem.browse`. |
| `queue.rs`, `questions.rs`, `approvals.rs`, `sidebar.rs` (`command`, `thread_action`) | `command(..)` → variantes de `dispatchCommand` | Quedan como constructores; `orchestration::dispatch` acepta su salida. Se les añade `commandId` obligatorio. `sidebar.rs` también tiene derivaciones puras (`sidebar_status`, `metadata`) que no se tocan. |
| `models.rs` (56) | `runtime_mode_command`, `catalog(config)`, `favorite_payload`, `Connection::model_catalog`, `Connection::set_model_favorite` | `model_catalog`/`set_model_favorite` pasan a llamar a `server::config`/`server::settings`/`server::update_settings`. Menos 2 sitios con tag literal en la lib. |
| `settings.rs` (134) | `snapshot(raw)`, `patch({key,value})` | Sin cambio; `patch` alimenta `server::update_settings`. |
| `terminal.rs` (50) | `session`, `dimensions`, `attach`, `write`, `resize`, `Output`, `Connection::terminal_write/resize` | `api::terminal` los reexporta y suma `open`. `terminal.resize` (`rpc.ts:1472`) no está en los 110 pero ya está envuelto aquí. |
| `documents.rs` (81), `lib.rs` `image_resource` | `Connection::document`, `attachment_image`, `media_image` llaman a `assets.createUrl` con un literal | Pasan a `assets::create_url`. El resto (GET firmado, límites 4/10 MiB) es el Lote 2 de la auditoría. |
| `composer.rs` (62) | `paths(result)`, `catalog(provider,…)` | Pos-procesan `projects.searchEntries` y `server.refreshProviders`. Sin cambio. |
| `history.rs` (86), `tasks.rs`, `changes.rs`, `activity.rs`, `lifecycle.rs`, `projection.rs` | derivaciones puras o rutas HTTP | Fuera de alcance: no son RPC. |

---

## 2. Streams (13)

Todos los streams son Effect-RPC sobre WebSocket: una conexión por suscripción (`Connection::subscribe_probed`, `stream.rs:11-22`). Cada `subscribe_*` propuesto es en realidad dos cosas:

```rust
// 1. el contrato de la suscripción, validado, sin red:
pub fn thread_spec(a: &ThreadSub) -> Result<StreamSpec>;          // StreamSpec { tag, payload, max_frame_bytes, first_item_timeout }
// 2. el bucle para hosts Rust (ot3), con el mismo callback que hoy:
pub fn subscribe_thread(conn: &Connection, a: &ThreadSub, cancel: impl Fn() -> bool,
    probe: impl FnMut() -> bool, receive: impl FnMut(Option<Value>) -> Result<()>) -> Result<()>;
```

El FFI usa solo (1) y entrega el `StreamSpec` a `Subscription::start` (`ffi_stream.rs:50-80`). Así la validación y el límite de trama (hoy cableado por tag en `stream.rs:16`) viven en un único sitio, y ot3 y Kotlin no pueden divergir.

| Tag | Función | Payload | Forma de los ítems | Cursor | Sitios ot3 |
|---|---|---|---|---|---|
| `orchestration.subscribeShell` | `orchestration::subscribe_shell(conn, after_sequence?, completion_marker?)` | `{afterSequence?: u64, requestCompletionMarker?: bool}` | `kind`: `synchronized` · `snapshot{snapshot, resolvedRepositoryIdentityRoots?}` · `project.updated{sequence,project}` · `project.removed{sequence,projectId}` · `thread.updated{sequence,location,thread}` · `thread.removed{sequence,location,threadId}` | `afterSequence` = mayor `sequence` visto (o `snapshot.snapshotSequence`). Si el servidor rechaza el cursor, empezar sin él (shell_watch.rs:130). | shell_watch.rs:86 |
| `orchestration.subscribeThread` | `orchestration::subscribe_thread(conn, &ThreadSub)` | `{threadId, afterSequence?, requestCompletionMarker?, acceptBoundedSnapshot?, acceptCompactTurnItems?}` | `kind`: `synchronized` · `snapshot{snapshotSequence, projection, historyCursor?, hasMoreHistory?, latestLocalTurnOrdinal?, payloadBudgetExceeded?, turnItemsOmitLocalVisible?}` · `event{sequence,event}` · evento desconocido (ot3 lo lee como `unknown-event`, live.rs:47; también mueve el cursor) | `afterSequence` = `max(snapshotSequence, sequence de cada event/unknown-event)`. El `snapshot` inicial viene de `GET …/bounded` (Lote 2). Con `acceptBoundedSnapshot` un ítem puede pesar hasta 64 MiB (stream.rs:16). | live.rs:36 |
| `orchestration.subscribeCommandOutput` | `orchestration::subscribe_command_output(conn, thread_id, item_id)` | `{threadId, itemId}` | `{kind:"replace"/"append", text, truncated, running}` | sin cursor: el primer ítem es `replace` con la cola acotada | command_output.rs:80 |
| `terminal.attach` | `terminal::subscribe_attach(conn, &AttachOpts)` | `TerminalAttachInput`: `threadId`, `terminalId`; opcionales: `cwd`, `worktreePath`, `cols`, `rows`, `env`, `providerInstanceId`, `restartIfNotRunning`, `resume` | unión por `type`: `snapshot`/`started`/`restarted`, `output`, `exited`, `closed`, `error`, `cleared`, `activity` | `sequence` del snapshot; la lib ya tiene `terminal::Output::apply` (terminal.rs:30) que descarta reproducciones anteriores al cursor | terminal.rs:67 |
| `subscribeDeviceState` | `device::subscribe_state(conn)` | `{}` | `DeviceServiceState` completo en cada ítem | sin cursor (cada ítem reemplaza al anterior) | integration_devices.rs:188 |
| `subscribeDiscoveredLocalServers` | `preview::subscribe_local_servers(conn, configured_urls?)` | `{configuredUrls?: Url[]}` | `{servers[], scannedAt, configuredUrlProbing?}` | sin cursor; el primer ítem puede preceder al escaneo (local_servers.rs:12) | local_servers.rs:13 |
| `subscribePreviewEvents` | `preview::subscribe_events(conn)` | `{}` | `PreviewEvent` por `type`: `opened`, `navigated`, `resized`, `failed`, `closed` | sin cursor; releer con `preview::list` (`serverEpoch`,`revision`) para detectar huecos | preview_transport.rs:309 |
| `subscribeServerConfig` | `server::subscribe_config(conn, &ConfigSub)` | `{environmentThemes?, usageLimitSources?, usageLimitsCommand?}` (cada flag evita que un cliente viejo reciba miembros que no decodifica) | `{version:1, type, …}`: `snapshot`, `keybindingsUpdated`, `providerStatuses`, `settingsUpdated`, `environmentThemesUpdated`, `usageLimitSourcesUpdated` | sin cursor: el primer ítem es `snapshot` | keybinding_settings.rs:52 quota_sources.rs:170 (← quotas.rs:164,261) |
| `subscribeWorktreeSetup` | `worktree_setup::subscribe(conn, thread_id)` | `{threadId}` | `WorktreeSetupSnapshot{threadId, phase, startedAt, endedAt, branch, baseRef, worktreePath, setupScript, stages[], error, sequence}` o `null` | `sequence` crece en cada ítem | worktree_setup.rs:22 |
| `server.getStorageCleanupReport` | `diagnostics::subscribe_storage_cleanup_report(conn) / diagnostics::storage_cleanup_report(conn, timeout)` | `{}` | `StorageCleanupReport` o `null` | sin cursor; ot3 solo lee el primer ítem (storage_cleanup.rs:19-31) | storage_cleanup.rs:19 |
| `provider.install.subscribe` | `provider::subscribe_install(conn, instance_id) / provider::install_state(conn, instance_id, timeout)` | `{instanceId}` | `ProviderInstallState` | sin cursor; ot3 lee el primer ítem con plazo de 10 s (provider_setup.rs:7-20) | provider_setup.rs:12 |
| `provider.auth.subscribe` | `provider::subscribe_auth(conn, instance_id) / provider::auth_state(conn, instance_id, timeout)` | `{instanceId}` | `ProviderAuthState` | ídem | provider_setup.rs:12 |
| `git.runStackedAction` | `vcs::subscribe_stacked_action(conn, &StackedAction)` | `{actionId, cwd, action (commit/push/create_pr/commit_push/commit_push_pr), commitMessage? (≤ 10 000), featureBranch?, filePaths?, threadId?, projectId?}` | `GitActionProgressEvent` por `kind`: `action_started`, `phase_started`, `hook_started`, `hook_output`, `hook_finished`, `action_finished{result}`, `action_failed{message}` | sin cursor; termina con `action_finished` o `action_failed`. **No** repetir tras corte: `M!`; releer `vcs::refresh_status` y `list_refs`. | git_controls.rs:364 → git_operations.rs:212 |

Referencias de contrato de los streams (`packages/contracts/src/`; ítem = esquema de la respuesta):

| Tag | `Rpc.make` | Clave del método | Payload (esquema) | Ítem (esquema) | Scope exigido | Handler |
|---|---|---|---|---|---|---|
| `orchestration.subscribeShell` | `rpc.ts:1721` | `orchestrationV2.ts:3094` | `OrchestrationV2SubscribeShellInput` `orchestrationV2.ts:3202` | `OrchestrationV2ShellStreamItem` `orchestrationV2.ts:1897` | `orchestration:read` (RpcAuthorization.ts:55) | `ws.ts:2031` |
| `orchestration.subscribeThread` | `rpc.ts:1728` | `orchestrationV2.ts:3095` | `OrchestrationV2SubscribeThreadInput` `orchestrationV2.ts:3208` | `OrchestrationV2ThreadStreamItem` `orchestrationV2.ts:3313` | `orchestration:read` (RpcAuthorization.ts:56) | `ws.ts:2033` |
| `orchestration.subscribeCommandOutput` | `rpc.ts:1736` | `orchestrationV2.ts:3096` | `OrchestrationV2SubscribeCommandOutputInput` `orchestrationV2.ts:3099` | `OrchestrationV2CommandOutputFrame` `orchestrationV2.ts:3116` | `orchestration:read` (RpcAuthorization.ts:57) | `ws.ts:2039` |
| `terminal.attach` | `rpc.ts:1453` | `rpc.ts:444` | `TerminalAttachInput` `terminal.ts:47` | `TerminalAttachStreamEvent` `terminal.ts:226` | `terminal:operate` (RpcAuthorization.ts:176) | `ws.ts:2870` |
| `subscribeDeviceState` | `rpc.ts:1616` | `rpc.ts:576` | `Schema.Struct({})` `inline` | `DeviceServiceState` `device.ts:230` | `orchestration:read` (RpcAuthorization.ts:206) | `ws.ts:2931` |
| `subscribeDiscoveredLocalServers` | `rpc.ts:1555` | `rpc.ts:575` | `inline` `rpc.ts:1555` | `DiscoveredLocalServerList` `preview.ts:340` | `orchestration:read` (RpcAuthorization.ts:196) | `ws.ts:2932` |
| `subscribePreviewEvents` | `rpc.ts:1548` | `rpc.ts:574` | `Schema.Struct({})` `inline` | `PreviewEvent` `preview.ts:316` | `orchestration:read` (RpcAuthorization.ts:195) | `ws.ts:2908` |
| `subscribeServerConfig` | `rpc.ts:1759` | `rpc.ts:577` | `inline` `rpc.ts:1759` | `ServerConfigStreamEvent` `server.ts:771` | `orchestration:read` (RpcAuthorization.ts:207) | `ws.ts:2958` |
| `subscribeWorktreeSetup` | `rpc.ts:1366` | `rpc.ts:570` | `WorktreeSetupSubscribeInput` `worktreeSetup.ts:66` | `WorktreeSetupStreamEvent` `worktreeSetup.ts:71` | `orchestration:read` (RpcAuthorization.ts:167) | `ws.ts:2797` |
| `server.getStorageCleanupReport` | `rpc.ts:744` | `rpc.ts:483` | `Schema.Struct({})` `inline` | `NullOr<StorageCleanupReport>` `settings.ts:1064` | `orchestration:read` (RpcAuthorization.ts:83) | `ws.ts:2381` |
| `provider.install.subscribe` | `rpc.ts:707` | `rpc.ts:424` | `ProviderSetupInput` `providerSetup.ts:12` | `ProviderInstallState` `providerSetup.ts:158` | `orchestration:read` (RpcAuthorization.ts:76) | `ws.ts:2361` |
| `provider.auth.subscribe` | `rpc.ts:688` | `rpc.ts:421` | `ProviderSetupInput` `providerSetup.ts:12` | `ProviderAuthState` `providerSetup.ts:118` | `providers:manage` (RpcAuthorization.ts:73) | `ws.ts:2357` |
| `git.runStackedAction` | `rpc.ts:1379` | `rpc.ts:436` | `GitRunStackedActionInput` `git.ts:126` | `GitActionProgressEvent` `git.ts:486` | `source-control:write*` (clientRpcPermissions.ts:38) | `ws.ts:2811` |

Notas de la tabla: `orchestration.subscribeCommandOutput` y `subscribeThread` no devuelven ítems del mismo tipo que sus lecturas unarias; el primer ítem de `subscribeThread` es un `snapshot` completo. El scope de `provider.install.subscribe` es `orchestration:read`, el de `provider.auth.subscribe` es `providers:manage` (`RpcAuthorization.ts:73,76`).

### 2.1 Cómo encaja con `stream.rs`

- `subscribe_probed` ya hace lo que el contrato pide: `Request` con `id:"1"`, `Ack` tras cada `Chunk`, `Pong` a cada `Ping`, ping propio cada 5 s, socket muerto a los 20 s (`STREAM_IDLE_TIMEOUT`), `Interrupt` al cancelar (`stream.rs:6-8,18-43`). Los 13 `subscribe_*` no tocan ese bucle.
- El cursor no vive en la lib. El host pasa `afterSequence` en cada (re)suscripción y lo avanza con cada ítem. Para que Kotlin no reimplemente la regla (`live.rs:41-48`, `shell_watch.rs:98-102`), la lib ofrece una función pura `stream::cursor_of(tag, &item) -> Option<u64>`: `sequence` de `event`/`project.updated`/`thread.updated`/…, `snapshotSequence` de `snapshot`, y la `sequence` de los eventos desconocidos (que también mueven el cursor, igual que en ot3). Se expone como `call` `stream.cursor`.
- Streams de «primer ítem» (`provider.*.subscribe`, `server.getStorageCleanupReport`, `subscribeDiscoveredLocalServers`): ot3 los usa como lecturas con plazo de 4–10 s (`provider_setup.rs:7-20`, `storage_cleanup.rs:14-31`, `local_servers.rs:8-30`). La lib los ofrece también como funciones unarias `*_state(conn, …, timeout)` basadas en un `stream::first_item(conn, spec, timeout, accept)`, y esas son las que Kotlin llama con `call`, sin cola ni `poll`.
- Un stream que el servidor cierra con `Exit` termina con `stream_closed` (`stream.rs:27`); la lib distingue `stream_closed`, `stream_timeout`, `disconnected`, `malformed_stream`. El backoff y la decisión de resembrar (`held = None`, `reconnect::transient`) siguen siendo del host.

### 2.2 Cómo encaja con `ffi_stream.rs`

Hoy: cola de 64 ítems y 8 MiB de JSON serializado por suscripción; desbordar la cola termina el stream con `stream_backpressure` y conserva lo ya encolado; un solo consumidor hace `poll` (espera ≤ 1 s); máximo 4 suscripciones por handle (`ffi.rs:85-86`, `ffi_stream.rs:12,26-41`). No hay pérdida silenciosa, y eso se mantiene. Lo que hay que cambiar para que los 13 streams funcionen en Android:

| # | Problema | Evidencia | Cambio propuesto |
|---|---|---|---|
| a | Un `snapshot` de hilo de más de 8 MiB (hasta 64 MiB con `acceptBoundedSnapshot`) nunca llega: `push` lo rechaza con `stream_backpressure` aunque `websocket_with_limit` lo aceptó | `stream.rs:16` (64 MiB) frente a `ffi_stream.rs:34-35` (`size > MAX_BYTES - bytes`) | `StreamSpec.max_frame_bytes` fija el tope por ítem y el de la cola es `max(8 MiB, max_frame_bytes + 8 MiB)`. Riesgo abierto: pasar 64 MiB por un `byte[]` de JNI es caro; si la medición en Android lo confirma, el snapshot se entrega por descriptor/archivo temporal (como ya hace `upload` con `fd`). |
| b | Se pierden los límites de chunk: `receive(None)` (fin de chunk o tick ocioso) no llega al host, y ot3 los usa para unir renders (`live.rs:41,52`) y para decidir «ya está al día» sin marcador (`shell_watch.rs:102`) | `ffi_stream.rs:63-72` solo encola `Some` | `poll` añade `boundaries: [i,…]` (posiciones de `items` tras las que terminó un chunk) e `idle: bool`. Es aditivo: `items`, `closed`, `error`, `errorMessage` no cambian. |
| c | Tras suspender el teléfono o cambiar de red no hay modo de comprobar los sockets: `subscribe_probed` tiene `probe`, el FFI usa `subscribe` (con `probe` fijo en `false`) | `stream.rs:11,14`, `ffi_stream.rs:63` | Nuevo op `{"op":"wake"}`: levanta un `AtomicBool` compartido que el `probe` de cada suscripción consume (ping inmediato y corte a los 3 s sin trama, `STREAM_PROBE_TIMEOUT`). Lo llama Android desde `ConnectivityManager`. |
| d | 4 suscripciones es poco: shell, hilo, salida de comando, config, preview y terminal son 6, y cada una abre su WebSocket | `ffi.rs:85` | `maxSubscriptions` = 8 en `capabilities`. Sin multiplexar: el servidor expone una petición por socket (`id:"1"`). |
| e | Las suscripciones por nombre de función (`fn`) necesitan validar antes de abrir red | `ffi.rs:83-100` (`tag` libre) | `subscribe` acepta `fn` además de `tag`; con `fn` pasa por `*_spec`. `tag` crudo sigue funcionando (sin validación) para no romper hosts existentes. |

`afterSequence` y `requestCompletionMarker` se pasan en `args`; la lib no los guarda. Para `subscribeThread` el flujo de Android queda: `call` `orchestration.boundedThread` (Lote 2) → `subscribe` con `afterSequence = snapshotSequence` → por cada `poll`, `stream.cursor` sobre los ítems → al reconectar, volver a suscribir con el último cursor.

---

## 3. Exposición por la ABI JSON en C

Hoy hay 9 ops en la lista `operations` (`ffi.rs:33`): `connectBearer`, `connectRelay`, `request`, `rpc`, `rpcWithTimeout` (solo en la lib; falta en el vendor), `disconnect`, `subscribe`, `pollSubscription`, `cancelSubscription`, más `capabilities`. Más las tres funciones C binarias (`t3_client_upload`, `t3_client_image`, `t3_client_call`). Nada de esto cambia.

| | A. Un op genérico `call` | B. Un op por función |
|---|---|---|
| Forma | `{"op":"call","fn":"vcs.refreshStatus","args":{"cwd":"/r"}}` | `{"op":"vcsRefreshStatus","cwd":"/r"}` (110 ops) |
| Validación | tabla estática: una sola ruta de código | 110 brazos en `ffi.rs:call` |
| Compatibilidad | una función nueva es una fila nueva; `capabilities`/`describe` la anuncia | cada tag nuevo cambia la lista `operations` y el header |
| Kotlin | `call(fn, args)` + wrappers generados | 110 métodos a mano o generados desde la misma tabla |
| Coste JNI | ninguno (un solo `byte[] → byte[]`) | ninguno |

**Recomendación: A (`call` + wrappers Kotlin generados), con `subscribe` aceptando `fn`.** Por qué, pensando en Kotlin:

1. El JNI ya es una sola función `byte[] → byte[]` (`T3Native.call`): un op por función solo multiplicaría los brazos del `match` y los cambios de ABI, sin ganar nada del lado Kotlin.
2. Los métodos tipados de Kotlin se **generan** desde `describe` (nombre, argumentos con tipo y obligatoriedad, kind, scope, plazo): hay seguridad de tipos en compilación sin mantener 110 métodos a mano, y un diff en CI avisa si la lib cambió y el generado no.
3. La política vive en un sitio: validación de ids antes de abrir red, plazo por función, «una mutación nunca se reintenta» y la obligación de `commandId` en las `M·id`. Con un op por función esa política se copiaría 110 veces.
4. Un servidor de T3 nuevo no obliga a una app vieja a actualizar: `describe` permite detectar funciones disponibles, y una función desconocida da `unknown_function` (distinto de `unknown_operation`).
5. A mano solo quedan los casos donde Kotlin necesita algo que no es JSON: subida por `fd` (`upload`), bytes de imagen (`image`) y el consumidor de `poll` (`T3Subscription`); esos ya son ops propios.

### 3.1 Contrato propuesto (ABI 3, solo aditivo)

```jsonc
// unario: lectura, mutación o «primer ítem» de un stream
{"op":"call","fn":"vcs.refreshStatus","args":{"cwd":"/repo"}}
{"op":"call","fn":"orchestration.dispatchCommand","timeoutMs":20000,
 "args":{"type":"thread.archive","commandId":"omarchy-1","threadId":"t-1"}}
// → {"ok":true,"data":{…respuesta del servidor…}}
// → {"ok":false,"error":"invalid_cwd"}                      (validación local, sin red)
// → {"ok":false,"error":"rpc_rejected","message":"…"}       (rechazo del servidor)
// → {"ok":false,"error":"rpc_outcome_unknown"}              (tras enviar)

// streams: el op existente, con «fn» en lugar de «tag»
{"op":"subscribe","fn":"orchestration.subscribeThread","args":{"threadId":"t-1","afterSequence":41,
 "requestCompletionMarker":true,"acceptBoundedSnapshot":true,"acceptCompactTurnItems":true}}
{"op":"pollSubscription","subscriptionId":"…","timeoutMs":1000}
// → {"items":[…],"boundaries":[3],"idle":false,"closed":false,"error":null,"errorMessage":null}

{"op":"wake"}                                   // tras reanudar o cambiar de red
{"op":"describe"}                               // sin conexión
// → {"apiVersion":1,"contract":"t3code@680152e27","functions":[
//     {"fn":"vcs.refreshStatus","module":"vcs","tag":"vcs.refreshStatus","kind":"read","replay":"free",
//      "scope":"orchestration:read","timeoutMs":20000,"stream":false,
//      "args":[{"name":"cwd","type":"path","required":true}]}, …]}
```

- `kind` = `read` / `mutation`; `replay` = `free` (L), `id` (M·id), `set` (M·set) o `never` (M!, M·id†); `scope` = el string de T3 o `"dynamic"`; `tag` = el tag de la red o `null` en funciones compuestas (p. ej. `provider.installState`, `assets.document`, `stream.cursor`).
- `capabilities` suma `"call"`, `"describe"`, `"wake"` a `operations`, `functions: 110+N`, `maxSubscriptions: 8` y `abiVersion: 3` (`T3_PROTOCOL_ABI_VERSION`, `t3_protocol.h:3`). Un host que no conoce `call` sigue funcionando con `rpc`.
- `rpc` y `rpcWithTimeout` quedan como puerta de escape sin validación; se documentan como «para tags que la lib aún no envuelve».
- Errores: mismo sobre `{ok,error,message?}`. Validación local: `invalid_<campo>` (`invalid_cwd`, `invalid_thread_id`, `invalid_command_id`) o `invalid_input` si el JSON no es un objeto; función desconocida: `unknown_function`.
- Kotlin: `T3Client.call(fn, args, timeoutMs?)`, un `object T3Api` generado por módulo (`T3Api.vcs.refreshStatus(cwd)`), `T3Client` pasa a leer `message` y `T3Subscription` pasa a exponer `boundaries`, `idle` y `errorMessage`. El generador es una tarea de Gradle que lee el JSON de `describe` guardado en el repo.

### 3.2 Qué módulo sale por dónde

| Módulo | Por `call` | Por `subscribe` con `fn` | Otro |
|---|---|---|---|
| `server` | 9 | 1 | — |
| `diagnostics` | 8 | 1 | — |
| `provider` | 9 | 2 | — |
| `orchestration` | 10 | 3 | — |
| `vcs` | 7 | 1 | — |
| `review` | 2 | 0 | — |
| `projects` | 7 | 0 | — |
| `pull_requests` | 12 | 0 | — |
| `preview` | 9 | 2 | — |
| `device` | 7 | 1 | — |
| `scheduled_tasks` | 5 | 0 | — |
| `agent_sessions` | 12 | 2 | `assets.createUrl` también alimenta `t3_client_image` / `t3_client_upload` (ya existen) |

Funciones compuestas (sin tag propio, solo vía `call`): `server.models` (= `models::model_catalog`), `server.setModelFavorite`, `assets.document` (= `Connection::document`), `provider.installState`, `provider.authState`, `diagnostics.storageCleanupReport`, `preview.discoveredServers`, `stream.cursor`, `terminal.resize`. Son las 9 que añaden filas al registro por encima de los 110 tags.

---

## 4. Convenciones

### 4.1 Tipos

- **Entradas tipadas, salidas `serde_json::Value`.** Cada función toma sus argumentos como parámetros Rust (o una struct pequeña con `Serialize`, `deny_unknown_fields` y `camelCase` cuando son más de cuatro). En el FFI, `args` se deserializa en esa misma struct: un campo desconocido o mal escrito es `invalid_input`, no se reenvía al servidor.
- **Las respuestas no se tipan.** El contrato las declara tolerantes a miembros nuevos (`ForwardCompatibleArray`, `ForwardCompatibleUnion`, `ForwardCompatibleNullable`), y ot3 ya consume `Value` en los 153 sitios; un struct estricto rompería con un servidor más nuevo y obligaría a reescribir las 318 pruebas de integración de ot3, que fijan JSON literal. Donde un host necesita una vista, la lib ya la ofrece como derivación pura (`models::catalog`, `settings::snapshot`, `queue::derive`, `projection::apply`…).
- `Void` → `Value::Null`; `Option` de Effect → se deja tal cual (`{_tag:"Some",value}`); las vistas puras lo normalizan.
- Los datos con secretos (`provider.auth.respond.response.values`, `secrets.answerRequest.answer.secret`) no implementan `Debug` ni se serializan en errores. Lo que ya hace la lib con `Credential`.

### 4.2 Validación de ids y texto (`valid_id`)

`valid_id` hoy solo existe en ot3 (`lib.rs:120-126`: no vacío, ≤ 2048, primer byte alfanumérico, resto `[A-Za-z0-9_:%.-]`; 85 usos en `src/`). La lib tiene validadores sueltos y distintos: `terminal.rs:4` (≤ 2048, sin control), `queue.rs` (`valid`), `models.rs:9` y `questions.rs`/`approvals.rs` (ids opacos sin tope de 256). El contrato solo exige `TrimmedNonEmptyString`, así que `valid_id` es **más estricto que el servidor**; por eso hay seis validadores pequeños en un módulo `ids` y cada argumento usa el que le toca. Aplicar `valid_id` a todo (`deviceId`, `profileId`…) rechazaría ids que el servidor sí acepta:

| Validador | Regla | Se aplica a |
|---|---|---|
| `ids::id` (= `valid_id` de ot3) | no vacío, ≤ 2048, `[A-Za-z0-9][A-Za-z0-9_:%.-]*` | los ids que el servidor mintea y ot3 ya valida así: `threadId`, `projectId`, `commandId`, `runId`, `messageId`, `itemId`/`turnItemId`, `subagentId`/`nodeId`, `scheduledTaskId`, `checkpointId`, `scopeId` (el contrato: `TrimmedNonEmptyString` sin más, `baseSchemas.ts:258-338`) |
| `ids::slug` | `^[a-zA-Z][a-zA-Z0-9_-]*$`, ≤ 64 (la regla exacta del contrato, `providerInstance.ts:5-6`) | `instanceId`, `provider` (`ProviderDriverKind`) |
| `ids::token(max)` | no vacío, sin caracteres de control, ≤ `max`, **sin restringir el alfabeto** | los ids con tope pero sin alfabeto en el contrato: `profileId` 64 (`browserProfile.ts:12`), `tabId` 128 (`preview.ts:15`), `flowId`/`operationId`/`interactionId`/`methodId` 128 (`providerSetup.ts:33`), `terminalId` 128 (`terminal.ts:15`), `hostId` 128 (`device.ts:10`), `deviceId` 256 (`device.ts:113`), `attachmentId` 256 (`assets.ts:116`), `actionId` |
| `ids::opaque` | no vacío, sin control, **sin recortar espacios y sin tope de 256**: los ids de proveedor llevan `%253A` y superan los 256 (`questions.rs` y `approvals.rs` ya lo prueban) | `requestId` de `runtime-request.respond` y `thread.user-input.dismiss` |
| `ids::path` | no vacío, ≤ 4096, sin control; no se comprueba contra el disco del host | `cwd`, `relativePath`, `worktreePath`, `destinationPath`, `workspaceRoot`, `scriptPath`, `partialPath`, `oldPath`/`newPath` |
| `ids::text(max)` | no vacío tras recortar, ≤ `max` caracteres | `title`, `body`, `query`, `refName`, `name`, `prompt` |

Cada rechazo ocurre **antes** de abrir el socket (patrón de `tests/stream.rs`, `pre_cancelled_subscription_does_not_open_a_socket`) y devuelve `invalid_<campo>`.

### 4.3 Errores

- Forma: `Result<T> = Result<T, String>` (`lib.rs:43`), con `code` o `code: reason`; `error_code(&str)` separa (`lib.rs:487`). El FFI ya lo parte en `error` + `message` (`ffi.rs:174-184`).
- Códigos que la lib ya emite y la tabla de funciones respeta: `not_connected`, `unauthorized`, `session_expired`, `unreachable`, `rpc_connection_failed: <causa>`, `rpc_rejected: <mensaje del servidor>` (primera línea, ≤ 300 caracteres), `write_not_authorized: <mensaje>`, `rpc_outcome_unknown`, `invalid_rpc_timeout`, `stream_closed`, `stream_timeout`, `stream_backpressure`, `malformed_stream`, `disconnected`, `subscription_limit`, `unknown_subscription`, `unknown_operation`. Se añaden `unknown_function`, `invalid_input` e `invalid_<campo>`.
- **`write_not_authorized` se decide por `_tag == "EnvironmentAuthorizationError"`**, no por subcadenas (hallazgo 5), y se agrega el scope que faltó: `write_not_authorized: <mensaje>` más `requiredScope` en el sobre del FFI (`{"ok":false,"error":"write_not_authorized","message":"…","requiredScope":"settings:write"}`). Es información, no una decisión: el host decide si pedir otro token.
- `rpc_outcome_unknown` solo se emite cuando la petición ya salió (`lib.rs:451-484`, `long_rpc.rs`). Lo que el host puede hacer depende del kind (tabla de la cabecera); para las `M!` y `M·id†` el estado se reconcilia leyendo:

| `M!` / `M·id†` | Reconciliar con |
|---|---|
| `server.runStorageCleanup`, `git.runStackedAction` | `diagnostics.storageCleanupReport`; `vcs.refreshStatus` + `vcs.listRefs` |
| `server.updateProvider`, `provider.auth.start/respond`, `provider.install.start` | `provider.installState` / `provider.authState`; `server.getConfig` (`providers[].version`) |
| `vcs.createRef`, `git.preparePullRequestThread` | `vcs.listRefs` |
| `projects.createNew`, `projectClone.start` | `orchestration` shell (`GET /api/orchestration/shell`) por nombre/ruta. ot3 ya lo trata como no repetible y guarda un recibo (`write.rs:252-259`) |
| `pullRequests.comment/submitReview/runAction` | `pullRequests.activity` / `pullRequests.detail` |
| `preview.open`, `device.open`, `device.action` | `preview.list`; `device.detail` |
| `scheduledTasks.upsert†/runNow` | `scheduledTasks.list` |
| `mcpApps.callTool`, `shell.openInEditor`, `secrets.answerRequest`, `terminal.write` | sin lectura que reconcilie (efecto externo): no repetir |

### 4.4 Plazos

Por defecto 20 s (`rpc`). Excepciones que el registro lleva por función, tomadas de ot3: `server.updateProvider` 300 s, `device.list/configure/testHost` 300 s, `device.open` 180 s, `device.close` y `preview.clearProfile` 60 s, `device.action` 30 s, `device.detail` 20 s. La petición `call` puede traer `timeoutMs` junto a `args` (1..300 000, el mismo rango de `rpc_with_timeout`) para sobreescribirlo.

### 4.5 Qué NO entra en la lib

| Queda en el host | Por qué / dónde está hoy en ot3 |
|---|---|
| Elegir el token de lectura o de escritura (y qué hacer con un `requiredScope` que falta) | La lib recibe una `Connection` ya construida. Los 27 constructores del Apéndice C de la auditoría siguen siendo del host. La lib solo **publica** el scope de cada función. |
| Persistencia: outbox de comandos, recibos, journal de rewind, caché de capacidades, estado de UI | `write.rs:248-262`, `checkpoint_journal.rs`, `server_capabilities.rs`. La lib acepta el `commandId` y no lo guarda. |
| Backoff, reconexión y decisión de reintentar | `reconnect.rs` (`Backoff`, `transient`, `Waiter`). **Esto contradice el Lote 3 de la auditoría**, que proponía mover `reconnect::Backoff` a la lib; con este diseño no se mueve y ninguna función de `api` hace `sleep`. |
| Enrutado entre entornos y balanceo | `connection_github.rs`, `connection_route.rs`, `connection_preferences.rs`. |
| Generar `commandId`, `actionId` y recibos | El host los crea y los conserva para poder reenviar el mismo payload. |
| Login, claves y almacenamiento seguro (Clerk, keyring, DPoP persistido) | `desktop_cloud.rs`; la lib lo dice en su README («Hosts own account login and key storage»). |
| Texto de interfaz y traducción de errores | `device_screen.rs:120`, `integration_devices.rs:350`: los mapas `rpc_rejected → mensaje` son del host. |
| Renderizado (Markdown, LaTeX, diffs en HTML), sistema de archivos, procesos, SSH, portapapeles | §2 y §4.D de la auditoría. |

---

## 5. Orden de implementación (3 tandas por uso)

Criterio: sitios de ot3 por tag, contando una vez el helper que repite el mismo tag en varias líneas (p. ej. `connection_github.rs` con 5 llamadas a `pullRequests.*`). Abren la tanda 1 los cinco tags que pide el encargo (`server.getConfig` 25, `server.getSettings` 13, `orchestration.dispatchCommand` 20, `server.updateSettings` 7, `preview.list` 4) y los demás tags que ot3 llama desde 3 archivos distintos o más (`review.getDiffPreview`, `projects.readFile`, `assets.createUrl`), más `vcs.refreshStatus` (3 sitios; es la lectura de estado de repo que Android necesita) y `launchThread` (comparte módulo y tipos con `dispatchCommand`). Los sitios salen de contar `archivo.rs:línea` en las tablas de §1.

| Tanda | Contenido | Tags | Sitios ot3 que deja tipados |
|---|---|---|---|
| 1 | Infraestructura (registro, `ids`, `call`/`describe`, errores) + 10 funciones de mayor uso | 10 | ≈ 83 |
| 2 | Los demás tags unarios | 87 | ≈ 122 |
| 3 | Los 13 streams, `first_item`, `cursor_of` y los cambios de `ffi_stream.rs` | 13 | ≈ 17 |

### Tanda 1 — núcleo y andamiaje

Tags: `server.getConfig` (25), `server.getSettings` (13), `server.updateSettings` (7), `orchestration.dispatchCommand` (20), `orchestration.launchThread` (1), `preview.list` (4), `assets.createUrl` (3), `vcs.refreshStatus` (3), `review.getDiffPreview` (4), `projects.readFile` (3).

| Pieza | Líneas (aprox.) |
|---|---|
| `api/mod.rs`: `Function`, `Kind`, `Replay`, `Scope`, macro `api_fn!`, parseo de `args`; `ids.rs` (los 4 validadores) | 260 |
| Arreglo de `rejection()` (por `_tag`, con `requiredScope`) y taxonomía de errores | 40 |
| `ffi.rs`: ops `call` y `describe`, `capabilities` v3, header y README | 110 |
| 10 funciones × ≈ 10 líneas | 100 |
| `Command` tipado (≈ 30 variantes que ot3 emite) + `launch_thread`; los constructores de `queue`/`questions`/`approvals`/`models` se envuelven, no se reescriben | 185 |
| **Producción Rust** | **≈ 700** |
| Tests: servidor falso compartido `tests/support/fake_t3.rs` (170); tabla por función (10 × 14 = 140); variantes de `Command` (30 × 5 = 150); `rejection` por `_tag` (40); deriva del contrato (60); ABI `call`/`describe` (90) | ≈ 650 |
| Kotlin a mano: `T3Client.call`, lectura de `message`; generador de wrappers | ≈ 190 (+ ≈ 300 generadas) |
| **Total tanda 1** | **≈ 1 350 Rust + 190 Kotlin** |

Salida comprobable: `grep -rnE '"(server.getConfig|server.getSettings|server.updateSettings|orchestration.dispatchCommand|preview.list)"' src` no devuelve nada fuera de `tests/`. `cargo test --locked` de ot3 (318 de integración, 93 unitarios) y de la lib en verde, y `tools/test-c-abi` con `call`.

### Tanda 2 — el resto de los unarios

| Pieza | Líneas (aprox.) |
|---|---|
| 87 funciones × ≈ 9 líneas | 783 |
| Casos grandes: `device.action` (14 variantes) 70, `scheduledTasks.upsert` 40, `pull_requests` (fusión con `request`) 60, `provider.auth_respond` 40, `projects.mutate` 50, teclas 20 | 280 |
| Menos lo que se borra por fusión (`models`, `documents`, `terminal`) | −60 |
| **Producción Rust** | **≈ 1 003** |
| Tests: una fila de tabla por función (≈ 8 líneas × 87)  4 pruebas de `rpc_outcome_unknown` (una por kind  25 c/u) y actualización de la deriva | ≈ 816 |
| Kotlin: solo wrappers generados | ≈ 700 generadas |
| **Total tanda 2** | **≈ 1 819 Rust** |

Salida: `grep -rn 'rpc(' src` y `grep -rn '\.rpc(' src` solo en `tests/` y en el despachador `rpc` crudo. Cada archivo de ot3 migrado deja sus tests de servidor falso verdes: fijan `req["tag"]` y `payload`, así que un tag o un campo mal escrito falla al instante.

### Tanda 3 — streams

| Pieza | Líneas (aprox.) |
|---|---|
| 13 `*_spec` + 13 `subscribe_*` × ≈ 14 | 180 |
| `stream::first_item` + 4 funciones `*_state` | 60 |
| `stream::cursor_of` | 40 |
| `ffi_stream.rs`/`ffi.rs`: límite por ítem, `boundaries`/`idle`, `wake`, `subscribe` con `fn`, 8 suscripciones | 150 |
| **Producción Rust** | **≈ 430** |
| Tests: uno por stream (13 × 22), desbordamiento y límite por ítem (80), límites de chunk (60), `wake`/`probe` (60), cursor (40) | ≈ 530 |
| Kotlin a mano: `T3Subscription` con `boundaries`/`idle`/`errorMessage` y una fachada `ThreadStream` | ≈ 120 |
| **Total tanda 3** | **≈ 960 Rust + 120 Kotlin** |

**Total ≈ 4 100 líneas de Rust** (la crate tiene hoy ≈ 3 800 en `src/`) **+ ≈ 310 de Kotlin a mano**. Son estimaciones por tag típico, con ±30 %.

**Desvío recomendado para Android.** El flujo de chat de Android necesita `subscribeShell` y `subscribeThread` antes que `preview.*`. Sugiero adelantar de la tanda 3 solo esos dos `subscribe_*` y los cambios a, b y c de §2.2 (≈ 300 líneas con sus pruebas). No altera el resto del orden.

### Fixtures de prueba

Todas con servidor WebSocket falso, sin servidor T3 real ni agentes de pago, como `tests/stream.rs`, `tests/rpc_errors.rs` y `tests/long_rpc.rs` de la lib, y `tests/writes.rs`/`tests/thread_launch.rs` de ot3 (`tungstenite::accept`, se lee la primera trama `Request` y se afirma `tag` y `payload`).

1. **`tests/support/fake_t3.rs` (nuevo, compartido).** `FakeT3::spawn(Vec<Step>)` con `Step::Rpc{tag, payload: Value, reply}` y `reply ∈ {Ok(Value), Fail(cause), DropAfterRequest, Chunks(Vec<Vec<Value>>), Hang}`. Registra cuántas conexiones y cuántas peticiones recibió para afirmar «una petición, cero reintentos».
2. **Tabla por módulo** (`tests/api_<módulo>.rs`): por función, `args` → `tag` y `payload` exactos, respuesta enlatada → salida. Cubre los 97 unarios. Ejemplo de fila: `("vcs.refreshStatus", json!({"cwd":"/r"}), "vcs.refreshStatus", json!({"cwd":"/r"}))`.
3. **Validación sin red:** con un `TcpListener` no bloqueante, cada `invalid_<campo>` deja `accept()` en `WouldBlock`.
4. **Taxonomía de errores:** causa con `EnvironmentAuthorizationError` → `write_not_authorized` con `requiredScope`; causa con «Author identity unknown» o «Authentication failed» → `rpc_rejected` (regresión del hallazgo 5); corte tras enviar → `rpc_outcome_unknown` para un representante de cada kind, con exactamente una petición recibida.
5. **Deriva del contrato:** `tests/fixtures/t3-contract.json`, generado por un script desde `rpc.ts`/`RpcAuthorization.ts` en el commit fijado, con tag, stream/no, scope y plazo. El test afirma que el registro coincide tag a tag (nombres únicos, `stream`, `scope`). Con `T3_CONTRACTS_DIR` definido, además relee `WS_METHODS` del checkout.
6. **ABI** (`tools/test-c-abi`, `tests/ffi_events.rs`): `capabilities.abiVersion == 3`; las operaciones antiguas siguen en la lista; `call` feliz; `unknown_function`; `invalid_input`; `describe` incluye las 110 funciones; `subscribe` con `fn` y con `tag` crudo.
7. **Streams:** 70 ítems en un chunk → `stream_backpressure` con lo encolado intacto (hoy, `ffi_stream.rs:121-137`); ítem de 20 MiB con `acceptBoundedSnapshot` pasa y sin él falla; `boundaries` en las posiciones esperadas; `Interrupt` al cancelar; `wake` corta el socket mudo en ≤ 3 s; `afterSequence` llega intacto en el `Request`; `cursor_of` sobre `event`, `snapshot` y evento desconocido.
8. **Kotlin** (`bindings/android/tests`, `KotlinFixture.kt`): los wrappers generados compilan contra el `describe` guardado y una llamada de ejemplo por módulo pasa por el JNI de prueba.

---

## 6. Qué verifiqué y qué no

- Verificado contra el código: existencia de los 110 tags y de su handler; scope de cada uno; `stream: true` en 13; esquemas de payload y respuesta de cada tag (leídos del `.ts`); deduplicación por `commandId` en `dispatchCommand` (`EventSink.ts:602-625`), `launchThread` (`ThreadMessageIntake.ts:15`) y `projects.mutate` (`ProjectService.ts:423`); el comportamiento de `rejection()` y de `ffi_stream.rs`.
- Verificado solo por muestra: que el payload que arma ot3 coincida campo a campo con el contrato. Lo miré en `projects.searchEntries` (`limit` obligatorio, ot3 lo envía), `getFullThreadDiff`/`getTurnDiff` (`diff_review.rs:195-198`), `server.getUsageSummary`, `server.getProcessDiagnostics`, `server.signalProcess`. No encontré diferencias.
- No verificado: deduplicación de `scheduledTasks.upsert` (por eso `M·id†`); que `vcs.init`, `agentSessions.import`, `server.prepareAcpRegistryAgent` y `mcpApps.updateModelContext` sean realmente idempotentes (los marqué `M·set` por su forma, no por haber leído el handler); el impacto en rendimiento de pasar 64 MiB por JNI; el alcance real de los scopes con los tokens que ot3 guarda hoy.
- Los conteos de líneas por tanda son estimaciones, no medidas.

---

## 7. Resumen (10 líneas)

1. Los 110 tags existen en T3 `680152e27`; ninguno es obsoleto. Son 97 unarios y 13 streams (no 98/12): `git.runStackedAction` es `stream: true`.
2. Propuesta: `t3_protocol::api::<módulo>` con 19 módulos (`server`, `diagnostics`, `provider`, `orchestration`, `vcs`, `review`, `projects`, `filesystem`, `pull_requests`, `preview`, `device`, `scheduled_tasks` y siete pequeños), funciones `fn(conn: &Connection, …) -> Result<Value>`, entradas tipadas y salidas `Value`.
3. Cada función lleva su kind: 41 lecturas, 4 mutaciones con `commandId`, 32 idempotentes por valor y 20 no repetibles. La lib no reintenta ninguna; el host reconcilia según la tabla de §4.3.
4. T3 autoriza con 10 scopes distintos en estos tags (6 dinámicos) y ot3 solo tiene 2 tokens: la lib publica el scope de cada función y el host elige token.
5. Hallazgo a corregir primero: `rejection()` marca `write_not_authorized` por subcadena (`"Auth"` casa con «Author identity unknown» y «Authentication failed»). Debe decidir por `EnvironmentAuthorizationError` y devolver `requiredScope`.
6. ABI: un op genérico `{"op":"call","fn":…,"args":…}` + `describe` + `subscribe` con `fn`, aditivo (ABI 3). Kotlin recibe un `call` y wrappers generados desde `describe`; los ops antiguos y `rpc` no cambian.
7. Fusiones: `pull_requests`, `projects`, `queue`/`questions`/`approvals`, `models`, `settings`, `terminal`, `documents` ya construyen payloads; pasan a ser validadores que `api` llama. No hay segundas fuentes de verdad.
8. Streams: `*_spec` validado + `subscribe_*` con el callback actual. `ffi_stream.rs` necesita cuatro cambios: tope por ítem (hoy un snapshot de más de 8 MiB nunca llega), `boundaries`/`idle`, op `wake` y 8 suscripciones.
9. Orden: tanda 1 (10 funciones de mayor uso + andamiaje, ≈ 1 350 líneas Rust), tanda 2 (87 unarios, ≈ 1 800), tanda 3 (13 streams, ≈ 960). Total ≈ 4 100 líneas Rust + ≈ 310 Kotlin a mano. Para Android conviene adelantar `subscribeShell`/`subscribeThread` y los cambios a-c de `ffi_stream`.
10. Quedan fuera de la lib: elección de token, persistencia (outbox, recibos), backoff y reconexión (contradice el Lote 3 de la auditoría, que movía `Backoff`), enrutado entre entornos, login y almacenamiento de claves, generación de ids de comando, textos de interfaz.
