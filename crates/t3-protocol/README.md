# t3-protocol

Shared Rust library for Omarchy T3 Code and Omi. The desktop plugin now uses this
crate for T3 HTTP reads, Effect RPC writes, signed attachment streaming and
T3 Connect authorization. The library has no Quickshell, Electron, filesystem,
SSH, keyring, Python or JavaScript runtime dependency.

## Rust API

```rust,no_run
use std::sync::Arc;
use t3_protocol::{connect_relay, Connection, Credential, DpopKey};
use serde_json::json;

# fn example() -> t3_protocol::Result<()> {
let direct = Connection::new(
    "https://environment.example", None, Credential::bearer("host-provided-token")?
)?;
let shell = direct.request_json("GET", "/api/orchestration/shell", None, true)?;
let relay = connect_relay(
    "https://relay.example", "host-provided-cloud-jwt", "t3-mobile",
    "00000000-0000-4000-8000-000000000001", Arc::new(DpopKey::generate())
)?;
let config = relay.rpc("server.getConfig", json!({}))?;
# Ok(()) }
```

The host supplies the current Clerk JWT issued for T3's relay template. A native
OAuth CLI token and a browser's cached DPoP access token are not interchangeable
with that JWT. A `RelayClient` can reuse relay authorization across environments.
`Connection::upload_attachment(name, mime, size, reader)` streams host-owned file
or Android content data through the same RPC grant and signed POST protocol.

The core signs ES256 DPoP proofs with fresh jti, htm, normalized htu, iat and ath;
checks the environment descriptor before exchanging its bootstrap credential;
and obtains a one-use WebSocket ticket for DPoP RPC. Credentials never go into
ordinary URLs. The ticket is used only for the WebSocket upgrade, as T3 requires.
Mutation requests are never automatically retried. A post-send failure returns
`rpc_outcome_unknown`; the host owns reconciliation and stable command IDs.

Keep keys and tokens in the platform's secure store if persisting them. Types
containing secrets do not implement Debug/Serialize. Expired DPoP credentials
fail with `session_expired`; reauthorize using the host's current account session.
Run blocking calls on a worker thread. Native TLS certificate validation stays on.
Direct bearer endpoints may use HTTP when inherited from T3; relay URLs require
HTTPS (loopback HTTP is allowed for isolated tests). Redirects are disabled.

## Omi / Kotlin

The crate builds as `rlib` and `staticlib`. `include/t3_protocol.h` exposes an opaque
handle and UTF-8 JSON C ABI for a thin Kotlin/JNI adapter. The adapter marshals
strings and ownership; HTTP/RPC/authentication stay in this library.

1. Build `t3-protocol` for each Android ABI with the Android NDK/toolchain.
2. Link `libt3_protocol.a` into Omi's JNI library and include `t3_protocol.h`.
3. Create a handle with `t3_client_new`, call from a worker thread, release each
   response with `t3_string_free`, then release the handle with `t3_client_free`.
4. Supply Omi's current signed-in cloud session to `connectRelay`; do not read
   Omarchy's files or duplicate the protocol in Kotlin. The Rust streaming API
   accepts a reader supplied by the Android content adapter.

C ABI commands:

```json
{"op":"connectBearer","httpBaseUrl":"https://environment.example","token":"host-provided-token"}
{"op":"connectRelay","relayUrl":"https://relay.example","cloudJwt":"host-provided-jwt","clientId":"t3-mobile","environmentId":"00000000-0000-4000-8000-000000000001"}
{"op":"request","method":"GET","path":"/api/orchestration/shell","protocol":true}
{"op":"rpc","tag":"server.getConfig","payload":{}}
{"op":"disconnect"}
```

Responses are `{ "ok": true, "data": ... }` or
`{ "ok": false, "error": "stable_code" }`. Inputs remain owned by the caller;
the caller clears its credential buffers. Never free a handle during a call.
Connection changes are atomic: a failed connect leaves the prior connection
intact, so the host must treat the returned error as a failed switch.

This repository supplies the shared library and ABI; it does not modify or build
the Omi Android app. Android linking and device validation are still required.
The checked host build is aarch64 Linux, not an Android binary.

## Validation

```sh
cargo test --workspace --locked
cargo build -p t3-protocol --release --locked
```

Fixtures verify real ES256 signatures and request/token binding, fresh proof IDs,
relay/environment exchanges, identity mismatch before credential release,
HTTP protocol headers, WebSocket tickets, RPC envelopes, C ABI ownership/errors
and attachment streaming via the desktop integration test. No paid agent runs
or real conversations are created by these tests.

`activity::describe` clasifica herramientas en resúmenes compactos para el timeline
y estados de trabajo. Acepta items planos/anidados y no exporta entradas, salidas
ni razonamiento crudo. No depende de Linux; reutilizable desde Omi.

`composer` comparte serialización de links de archivos de T3 y sugerencias de
skills/comandos por workspace/proveedor. Filtra skills deshabilitadas o reservadas
al agente. No realiza escaneos de disco ni depende de QML/Linux.

`queue::derive` comparte orden/estado/capacidades de la cola y IDs ocultos del
timeline; `queue::command` genera comandos nativos de editar, quitar, reordenar,
promover a Steer y reanudar. Adaptadores del host gestionan outbox y credenciales.

`tasks::derive` comparte el resumen/lista de tareas del run actual para composer,
con progreso y duraciones del servidor, sin confundir planes de ejecuciones viejas.

`i18n::{normalize,translate}` resuelve catálogos planos por idioma/región y listas
de preferencia, con fallback inglés y alias noruegos nb/nn. El host proporciona
el catálogo y el locale; el core no lee preferencias ni depende de Qt/Linux.

`composer::attachment_reference` conserva la sintaxis inline canónica de T3 y
sanitiza etiquetas a200 caracteres. `legacy_attachment_text` proyecta las
referencias conocidas a etiquetas para transportes anteriores, manteniendo los
descriptores en attachments. Compartido por send y edición de cola; sin I/O.
El adaptador Wayland clipboard está fuera del core y no introduce dependencia
Linux en Omi.

`changes::summaries` deriva archivos/totales y rango de diff por checkpoint del
run de app; excluye capturas no ready y scopes sin appRunOrdinal. No lee Git ni
archivos del host. Actividad conserva sólo ejecutable seguro/título publicado y
metadatos para agrupar/desplegar; no expone argumentos/salidas/razonamiento.
Omi puede usar summaries y pedir orchestration.getTurnDiff con su Connection.

`Connection::subscribe` comparte el transporte Effect RPC/DPoP de T3 con Omi,
con Chunk/Ack, Ping/Pong, cancelación e interrupción. El host gestiona cursor de
snapshot/evento, deduplicación, backoff y UI. El callback recibe None al cerrar un
chunk o durante un tick de lectura para agrupar la presentación sin perder eventos.
`projection::apply` actualiza un espejo acotado por ID; descarta outputs/diffs/
razonamiento de herramientas que no se usan en la presentación compacta.
`attachment_image` pide assets.createUrl y lee bytes por el grant firmado del
mismo origen, sin auth en el GET, verifica firma de imagen y límite10MiB. El host
pone el caché/decodificación; el core no depende del filesystem ni de Qt/Linux.

`compaction::candidate(projection, settings, now_ms)` derives preventive deadlines
from the current root provider's native token report, with an explicitly estimated
fallback and provider/environment overrides. Hosts own the durable one-attempt
claim, native command dispatch, cancellation checks and lifecycle. A native
`/compact` never creates another human activity episode. See the desktop README
for idle/queue/request guards and limits; Omi must implement the same durable
claim before dispatch, rather than merely running the pure policy periodically.
